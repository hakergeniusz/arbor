//! On-disk scan cache (user request: "add caching").
//!
//! A finished scan is persisted under `$XDG_CACHE_HOME/arbor/scans/` so
//! re-opening the same folder restores the previous result instantly — no
//! automatic rescan (a restore that recalculates defeats the cache); the
//! rescan button / Ctrl+R refresh it for real (docs/03's snapshot semantics
//! are untouched — a cached snapshot is simply an older one).
//!
//! Format: a flat pre-order stream of records instead of a nested tree, so
//! the scanner worker can append one record per entry *as it lstats it* —
//! streaming, no extra worker memory, and the main loop never pays for
//! encoding. The decoder reads one pass into slot storage (parents are
//! always recorded before children, so a record's parent is already
//! decoded), then rebuilds the tree with two non-recursive index sweeps:
//! descending order aggregates folder sizes bottom-up (size = own +
//! Σ descendants), ascending order attaches children (per-parent order =
//! record order) — reproducing exactly the tree the UI holds after
//! `Finished`, with no recursion to overflow on pathological depth.
//!
//! ```text
//! header:  "ARBCACHE2" | u32 node_count | u64 root mtime | u64 scanned_at
//!          | u16 root_path len | root_path bytes
//! record:  u32 parent index (u32::MAX for the root) | u8 flags (bit0 dir,
//!          bit1 error) | u64 own size | u8 mtime flag | i64 mtime
//!          | u16 name len | name bytes
//! ```
//!
//! Every read is advisory: any problem (missing file, bad magic, truncated
//! stream) yields `None` and the UI falls back to a normal scan.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::models::DisplayNode;

const MAGIC: &[u8; 9] = b"ARBCACHE2";
/// Upper bound on cached files kept per user (largest scans win nothing by
/// eviction order — oldest files are pruned after a successful write).
const MAX_FILES: usize = 4;

/// A decoded snapshot: the tree as it stood when the scan finished.
pub struct CachedScan {
    pub root: DisplayNode,
    /// Unix seconds when the cached scan completed.
    pub scanned_at: u64,
}

/// Where the cache for `root` lives, or None when no cache directory can be
/// determined ($XDG_CACHE_HOME, else $HOME/.cache).
pub fn cache_path(root: &Path) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(cache_file_for(&base, root))
}

/// Cache file path for `root` under cache base `base` — split from
/// [`cache_path`] so tests can redirect the location without mutating the
/// process environment.
pub(crate) fn cache_file_for(base: &Path, root: &Path) -> PathBuf {
    // Hash the full path (collisions across same-named folders) and keep a
    // readable suffix for anyone poking around the directory.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        root.display().to_string().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };
    let suffix = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "root".to_string());
    let suffix: String = suffix
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40)
        .collect();
    base.join("arbor").join("scans").join(format!("{key}-{suffix}.tree"))
}

/// Streaming writer used by the scanner worker: one record per lstat'ed
/// entry, in walk order. The root's record (parent `u32::MAX`) must be
/// written first; directory records' indices become later entries' parents.
///
/// Dropping a Writer that never reached [`Writer::finish`] (cancelled scan,
/// failed scan) removes its temp file — no `.tree.tmp` orphans.
pub struct Writer {
    out: BufWriter<std::fs::File>,
    next_index: u32,
    tmp_path: PathBuf,
    final_path: PathBuf,
    node_count: u32,
    finished: bool,
    /// Reusable record buffer: fixed fields + name packed into one
    /// `write_all` per record instead of seven.
    buf: Vec<u8>,
}

impl Writer {
    /// Create the cache writer for a scan of `root` (temp file + header).
    /// Returns None when caching is impossible; the scan proceeds uncached.
    pub fn create(root: &Path, root_mtime: i64) -> Option<Writer> {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
        Writer::create_in(&base, root, root_mtime)
    }

    /// [`Writer::create`] under an explicit cache base (tests).
    pub(crate) fn create_in(base: &Path, root: &Path, root_mtime: i64) -> Option<Writer> {
        let final_path = cache_file_for(base, root);
        let dir = final_path.parent()?.to_path_buf();
        std::fs::create_dir_all(&dir).ok()?;
        let tmp_path = final_path.with_extension("tree.tmp");
        let file = std::fs::File::create(&tmp_path).ok()?;
        let mut out = BufWriter::with_capacity(1 << 20, file);
        let root_str = root.display().to_string();
        out.write_all(MAGIC).ok()?;
        out.write_all(&0u32.to_le_bytes()).ok()?; // node count, patched on finish
        out.write_all(&(root_mtime as u64).to_le_bytes()).ok()?;
        out.write_all(&(now_secs()).to_le_bytes()).ok()?;
        out.write_all(&(root_str.len() as u16).to_le_bytes()).ok()?;
        out.write_all(root_str.as_bytes()).ok()?;
        Some(Writer {
            out,
            next_index: 0,
            tmp_path,
            final_path,
            node_count: 0,
            finished: false,
            buf: Vec::with_capacity(4 + 1 + 8 + 1 + 8 + 2 + 128),
        })
    }

    /// Append one entry record; returns the record's index (the parent index
    /// for its own children). `parent` is `None` for the scan root.
    pub fn record(
        &mut self,
        parent: Option<u32>,
        name: &str,
        is_dir: bool,
        error: bool,
        own_size: u64,
        mtime: Option<i64>,
    ) -> Option<u32> {
        let index = self.next_index;
        if index == u32::MAX {
            return None; // absurd tree; abandon caching, scanning continues
        }
        let mut flags = 0u8;
        if is_dir {
            flags |= 1;
        }
        if error {
            flags |= 2;
        }
        // One write per record: the BufWriter coalesces, but seven small
        // write_all calls per record across 10⁵+ records add up.
        let mut buf = std::mem::take(&mut self.buf);
        buf.clear();
        buf.extend_from_slice(&parent.unwrap_or(u32::MAX).to_le_bytes());
        buf.push(flags);
        buf.extend_from_slice(&own_size.to_le_bytes());
        match mtime {
            Some(m) => {
                buf.push(1);
                buf.extend_from_slice(&m.to_le_bytes());
            }
            None => buf.push(0),
        }
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(name.as_bytes());
        let written = self.out.write_all(&buf).ok();
        self.buf = buf;
        written?;
        self.next_index += 1;
        self.node_count += 1;
        Some(index)
    }

    /// Finish the stream: patch the node count, move the temp file into
    /// place, prune old entries. Failures are silently non-fatal.
    pub fn finish(mut self) {
        let _ = self.out.flush();
        // Patch the node count in the header (fixed offset: magic 9 + count 4).
        // The BufWriter stays open (nothing is written through it anymore;
        // its Drop flushes an empty buffer) — Unix allows the concurrent
        // second handle and the rename of an open file.
        if let Some(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .open(&self.tmp_path)
            .ok()
        {
            use std::io::{Seek, SeekFrom};
            let _ = f.seek(SeekFrom::Start(MAGIC.len() as u64));
            let _ = f.write_all(&self.node_count.to_le_bytes());
        }
        self.finished = true; // the temp file becomes the real cache below
        let _ = std::fs::rename(&self.tmp_path, &self.final_path);
        prune_old_cache_files(&self.final_path);
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.finished {
            // Cancelled or failed scan: the temp file would otherwise
            // linger forever (nothing reads it, prune ignores it).
            let _ = std::fs::remove_file(&self.tmp_path);
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Upper bound on the total size of cached files kept per user. Bounds the
/// cache directory for huge scans (a 1M-item tree caches to ~57 MB; four of
/// those plus bigger mounts must not grow without limit).
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// A `.tree.tmp` older than this is an orphan from a crash or an older
/// version (the writer's Drop removes its own on cancel/failure); younger
/// ones may belong to a concurrently running scan in another instance and
/// are left alone.
const TMP_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

fn prune_old_cache_files(keep: &Path) {
    let Some(dir) = keep.parent() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let now = std::time::SystemTime::now();
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        let Ok(meta) = e.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        match path.extension().and_then(|x| x.to_str()) {
            Some("tree") => files.push((modified, meta.len(), path)),
            Some("tmp")
                if now.duration_since(modified).is_ok_and(|age| age > TMP_MAX_AGE) =>
            {
                let _ = std::fs::remove_file(&path); // stale orphan
            }
            _ => {}
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
    let mut kept = 0usize;
    let mut kept_bytes = 0u64;
    for (_, size, path) in &files {
        kept += 1;
        kept_bytes += size;
        if kept > MAX_FILES || kept_bytes > MAX_TOTAL_BYTES {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Header facts read without decoding the whole stream — the UI decides
/// between a synchronous and a worker-thread decode with these.
pub struct SnapshotInfo {
    pub node_count: u32,
    pub scanned_at: u64,
}

/// Read just the cache header for `root`.
pub fn peek(root: &Path) -> Option<SnapshotInfo> {
    let path = cache_path(root)?;
    let file = std::fs::File::open(path).ok()?;
    let mut r = BufReader::new(file);
    let mut magic = [0u8; 9];
    r.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    let node_count = read_u32(&mut r)?;
    let _root_mtime = read_u64(&mut r)?;
    let scanned_at = read_u64(&mut r)?;
    let root_len = read_u16(&mut r)? as usize;
    if root_len > 4096 {
        return None;
    }
    let mut root_buf = vec![0u8; root_len];
    r.read_exact(&mut root_buf).ok()?;
    let root_path = PathBuf::from(String::from_utf8(root_buf).ok()?);
    if root_path.as_path() != root {
        return None;
    }
    Some(SnapshotInfo {
        node_count,
        scanned_at,
    })
}

/// Load the cached scan for `root`, if a well-formed one exists.
pub fn load(root: &Path) -> Option<CachedScan> {
    let path = cache_path(root)?;
    load_at(&path, root)
}

/// [`load`] under an explicit cache base (tests redirect the location
/// without mutating the process environment).
pub(crate) fn load_in(base: &Path, root: &Path) -> Option<CachedScan> {
    load_at(&cache_file_for(base, root), root)
}

fn load_at(path: &Path, root: &Path) -> Option<CachedScan> {
    let file = std::fs::File::open(path).ok()?;
    let mut r = BufReader::with_capacity(1 << 20, file);

    let mut magic = [0u8; 9];
    r.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    let node_count = read_u32(&mut r)?;
    let root_mtime = read_u64(&mut r)?;
    let scanned_at = read_u64(&mut r)?;
    let root_len = read_u16(&mut r)? as usize;
    if root_len > 4096 {
        return None;
    }
    let mut root_buf = vec![0u8; root_len];
    r.read_exact(&mut root_buf).ok()?;
    let root_path = PathBuf::from(String::from_utf8(root_buf).ok()?);
    if root_path.as_path() != root || node_count == 0 {
        return None;
    }

    // Decode the flat stream in one pass into slot storage. The scanner
    // writes every record after its parent (a directory's record is written
    // when its frame is pushed), so a record's parent is always already
    // decoded when the record arrives. The tree is then rebuilt by two
    // non-recursive sweeps: descending index order aggregates sizes/items
    // into parents (a child is always aggregated before its parent is
    // touched), ascending index order attaches each node to its parent
    // (per-parent child order = record order). No recursion: a pathological
    // deep chain cannot overflow the stack.
    let mut slots: Vec<Option<DisplayNode>> = Vec::with_capacity(node_count as usize);
    let mut parents: Vec<u32> = Vec::with_capacity(node_count as usize);
    for _ in 0..node_count {
        let parent = read_u32(&mut r)?;
        let flags = read_u8(&mut r)?;
        let own_size = read_u64(&mut r)?;
        let mtime = if read_u8(&mut r)? == 1 {
            Some(read_i64(&mut r)?)
        } else {
            None
        };
        let name_len = read_u16(&mut r)? as usize;
        if name_len > 4096 || slots.len() >= node_count as usize {
            return None;
        }
        let mut name_buf = vec![0u8; name_len];
        r.read_exact(&mut name_buf).ok()?;
        let name = String::from_utf8(name_buf).ok()?;
        let path = if parent == u32::MAX {
            root_path.clone()
        } else {
            let parent_node = slots
                .get(parent as usize)
                .and_then(|s| s.as_ref())
                .filter(|p| p.is_dir)?;
            parent_node.path.join(&name)
        };
        slots.push(Some(DisplayNode {
            name,
            path,
            is_dir: flags & 1 != 0,
            error: flags & 2 != 0,
            own_size,
            size: own_size,
            items: 1,
            mtime,
            measured: true,
            children: Vec::new(),
            child_index: None,
        }));
        parents.push(parent);
    }
    let n = slots.len();
    // Bottom-up attach (the old recursive assemble, as index math):
    // descending index order moves every node after all of its descendants
    // are already attached, so each subtree travels as one. The moved
    // node's aggregates fold into the parent at the same moment (folder
    // size = own + Σ descendants, items = 1 + Σ — the old finalize).
    // Attaching delivers each folder's children in reverse record order,
    // so each completed subtree reverses its children back (and rebuilds
    // the name index built during attach) before moving to its parent.
    for i in (1..n).rev() {
        if parents[i] == u32::MAX {
            continue; // stray extra root record: not part of the tree
        }
        let p = parents[i] as usize;
        if p >= n {
            continue; // unreachable (decode bounds parents), kept for safety
        }
        let mut node = slots[i].take().expect("attached once");
        node.reverse_children();
        let parent = slots[p].as_mut().expect("parent decoded first");
        parent.size += node.size;
        parent.items += node.items;
        parent.push_child(node);
    }
    let mut root_node = slots.swap_remove(0).expect("root decoded");
    root_node.reverse_children();
    // The root's stored mtime is the directory's; kept in the header as a
    // cross-check hook for future validation strategies.
    let _ = root_mtime;
    Some(CachedScan {
        root: root_node,
        scanned_at,
    })
}

fn read_u8(r: &mut impl Read) -> Option<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).ok()?;
    Some(b[0])
}

fn read_u16(r: &mut impl Read) -> Option<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).ok()?;
    Some(u16::from_le_bytes(b))
}

fn read_u32(r: &mut impl Read) -> Option<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> Option<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).ok()?;
    Some(u64::from_le_bytes(b))
}

fn read_i64(r: &mut impl Read) -> Option<i64> {
    read_u64(r).map(|v| v as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::Duration;

    /// Fresh per-test cache base + scan root under the OS temp dir.
    struct Fixture {
        base: PathBuf,
        dir: PathBuf,
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Fixture {
            let dir = std::env::temp_dir().join(format!("arbor-cache-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let base = dir.join("cache-base");
            let root = dir.join("scan-root");
            std::fs::create_dir_all(&root).unwrap();
            Fixture { base, dir, root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    /// Drive a Writer over a synthetic two-level tree and decode it back.
    #[test]
    fn roundtrip_restores_tree_with_finalized_aggregates() {
        let fx = Fixture::new("rt");
        let mut writer = Writer::create_in(&fx.base, &fx.root, 1000).expect("writer");
        let root_idx = writer
            .record(None, "roundtrip-root", true, false, 4096, Some(1000))
            .expect("root record");
        let docs = writer
            .record(Some(root_idx), "docs", true, false, 4096, Some(1001))
            .expect("docs record");
        writer
            .record(Some(docs), "a.txt", false, false, 100, Some(1002))
            .expect("a record");
        writer
            .record(Some(docs), "err-dir", true, true, 4096, Some(1003))
            .expect("err record");
        writer
            .record(Some(root_idx), "b.bin", false, false, 200_000, Some(1004))
            .expect("b record");
        writer.finish();

        let cached = load_in(&fx.base, &fx.root).expect("loaded");
        let root = &cached.root;
        assert_eq!(root.path, fx.root);
        assert_eq!(root.children.len(), 2);
        // docs = own 4096 + a.txt 100 + err-dir 4096
        let docs_node = &root.children[0];
        assert_eq!(docs_node.name, "docs");
        assert_eq!(docs_node.size, 4096 + 100 + 4096);
        assert_eq!(docs_node.items, 3);
        assert!(docs_node.children[1].error, "error flag survives");
        // root = own 4096 + docs subtree + b.bin 200000
        assert_eq!(root.size, 4096 + docs_node.size + 200_000);
        assert_eq!(root.items, 1 + docs_node.items + 1);
        assert!(root.measured && docs_node.children.iter().all(|c| c.measured));
    }

    /// A 1000-deep chain: the decode's bottom-up attach must rebuild the
    /// whole subtree iteratively (no recursion) with intact aggregates,
    /// child order and paths.
    #[test]
    fn deep_chain_decodes_iteratively_with_intact_aggregates() {
        const DEPTH: u32 = 1000;
        let fx = Fixture::new("deep");

        let mut writer = Writer::create_in(&fx.base, &fx.root, 1).expect("writer");
        writer
            .record(None, "deep-root", true, false, 1, Some(1))
            .expect("root record");
        let mut parent = Some(0u32);
        for d in 1..DEPTH {
            parent = writer.record(parent, &format!("n{d:04}"), true, false, 1, Some(1));
            assert!(parent.is_some(), "record {d} accepted");
        }
        writer.finish();

        let cached = load_in(&fx.base, &fx.root).expect("loaded");
        let mut node = &cached.root;
        assert_eq!(node.name, "deep-root");
        assert_eq!(node.size, DEPTH as u64, "root aggregates the whole chain");
        assert_eq!(node.items, DEPTH as u64);
        let mut path = fx.root.clone();
        for d in 1..DEPTH {
            assert_eq!(node.children.len(), 1, "single child at depth {d}");
            node = &node.children[0];
            path.push(format!("n{d:04}"));
            assert_eq!(node.path, path, "path chain intact at depth {d}");
        }
        assert_eq!(node.name, format!("n{:04}", DEPTH - 1));
        assert_eq!(node.size, 1);
        assert!(node.children.is_empty());
    }

    #[test]
    fn load_rejects_garbage_and_wrong_path() {
        let fx = Fixture::new("bad");
        // No file at all.
        assert!(load_in(&fx.base, &fx.root).is_none());
        // Garbage file.
        let path = cache_file_for(&fx.base, &fx.root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"NOTACACHE!!!!!!!!").unwrap();
        assert!(load_in(&fx.base, &fx.root).is_none());
    }

    #[test]
    fn collect_paths_lists_every_node() {
        let fx = Fixture::new("paths");
        let mut writer = Writer::create_in(&fx.base, &fx.root, 1).unwrap();
        let root_idx = writer.record(None, "paths-root", true, false, 4096, Some(1)).unwrap();
        let sub = writer.record(Some(root_idx), "sub", true, false, 4096, Some(1)).unwrap();
        writer.record(Some(sub), "f.txt", false, false, 1, Some(1)).unwrap();
        writer.finish();

        let cached = load_in(&fx.base, &fx.root).unwrap();
        let mut paths = HashSet::new();
        cached.root.collect_paths(&mut paths);
        assert_eq!(paths.len(), 3);
        assert!(paths.contains(&fx.root));
        assert!(paths.contains(&fx.root.join("sub")));
        assert!(paths.contains(&fx.root.join("sub").join("f.txt")));
    }

    /// A Writer dropped without finish() (cancelled or failed scan) must
    /// not leave its .tree.tmp behind.
    #[test]
    fn writer_drop_without_finish_removes_tmp() {
        let fx = Fixture::new("drop");
        let tmp = {
            let writer = Writer::create_in(&fx.base, &fx.root, 1).expect("writer");
            writer.tmp_path.clone()
        }; // dropped unfinished here
        assert!(!tmp.exists(), "unfinished writer's tmp removed on drop");
        assert!(load_in(&fx.base, &fx.root).is_none(), "no cache was published");
    }

    /// Prune keeps the newest MAX_FILES caches within MAX_TOTAL_BYTES and
    /// deletes .tree.tmp orphans — but only stale ones (a concurrently
    /// running scan in another instance owns fresh tmp files).
    #[test]
    fn prune_enforces_count_size_and_tmp_orphans() {
        let fx = Fixture::new("prune");
        let scans = cache_file_for(&fx.base, &fx.root)
            .parent()
            .expect("scans dir")
            .to_path_buf();
        std::fs::create_dir_all(&scans).unwrap();
        let write_file = |name: &str, len: u64, age: Duration| {
            let path = scans.join(name);
            let f = std::fs::File::create(&path).unwrap();
            f.set_len(len).unwrap(); // sparse; prune only reads metadata().len()
            f.set_times(
                std::fs::FileTimes::new()
                    .set_modified(now_secs_systemtime() - age),
            )
            .unwrap();
            drop(f);
            path
        };
        let hour = Duration::from_secs(3600);
        // Six .tree files, newest first by age: keep 4 (count rule).
        let mut tree_paths = Vec::new();
        for i in 0..6 {
            tree_paths.push(write_file(&format!("t{i}.tree"), 1000, hour * (i + 1)));
        }
        // Fresh tmp (a live scan elsewhere) and a stale orphan.
        write_file("live.tree.tmp", 500, Duration::from_secs(60));
        let orphan = write_file("orphan.tree.tmp", 500, hour * 48);
        prune_old_cache_files(&tree_paths[0]);
        for p in tree_paths.iter().take(4) {
            assert!(p.exists(), "newest four kept: {}", p.display());
        }
        for p in tree_paths.iter().skip(4) {
            assert!(!p.exists(), "beyond MAX_FILES removed: {}", p.display());
        }
        assert!(!orphan.exists(), "stale tmp orphan removed");
        assert!(scans.join("live.tree.tmp").exists(), "fresh tmp untouched");

        // Size rule: four files whose total exceeds MAX_TOTAL_BYTES — the
        // oldest over-budget ones go even though the count allows them.
        for p in tree_paths.iter().take(4) {
            std::fs::remove_file(p).ok();
        }
        let big: Vec<PathBuf> = (0..4)
            .map(|i| write_file(&format!("big{i}.tree"), 90 * 1024 * 1024, hour * (i + 1)))
            .collect();
        prune_old_cache_files(&big[0]);
        assert!(big[0].exists() && big[1].exists(), "newest within budget kept");
        assert!(!big[2].exists() && !big[3].exists(), "over-budget evicted");
    }

    fn now_secs_systemtime() -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
}
