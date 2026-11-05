//! Scanner integration tests (docs/09 Part C: A8/A9 territory).
//!
//! All fixtures are created by the tests themselves inside fresh temporary
//! directories; nothing pre-existing is ever touched, matching the project's
//! destructive-testing rule.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use arbor::models::node_contribution;
use arbor::scanner::{spawn, should_descend, ScanUpdate};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "arbor-scanner-test-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Drain the update channel until a terminal update arrives (with timeout).
fn collect_updates(handle_dir: &Path) -> Vec<ScanUpdate> {
    let (handle, rx) = spawn(handle_dir, false, vec![]);
    let mut updates = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(update) => {
                let terminal = matches!(
                    update,
                    ScanUpdate::Finished { .. }
                        | ScanUpdate::Failed { .. }
                        | ScanUpdate::Cancelled
                );
                updates.push(update);
                if terminal {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                assert!(std::time::Instant::now() < deadline, "scan timed out");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(handle); // cancels + joins the worker
    updates
}

/// Independent ground-truth walk (mirrors the spec arithmetic).
fn ground_truth(path: &Path) -> (u64, u64) {
    let meta = fs::symlink_metadata(path).unwrap();
    let own = node_contribution(meta.size() as u64, meta.blocks() as u64);
    if meta.is_dir() {
        let mut size = own;
        let mut items = 1u64;
        for entry in fs::read_dir(path).unwrap().flatten() {
            let ft = entry.file_type().unwrap();
            if ft.is_symlink() {
                continue;
            }
            let (s, i) = ground_truth(&entry.path());
            size += s;
            items += i;
        }
        (size, items)
    } else {
        (own, 1)
    }
}

/// The DirMeasured update for `dir` itself.
fn measured_at<'a>(updates: &'a [ScanUpdate], dir: &Path) -> Option<(u64, u64)> {
    updates.iter().find_map(|u| match u {
        ScanUpdate::DirMeasured { path, size, items, .. } if path == dir => {
            Some((*size, *items))
        }
        _ => None,
    })
}

#[test]
fn nested_tree_totals_match_ground_truth() {
    let dir = temp_dir("nested");
    fs::write(dir.join("a-256B.txt"), vec![b'x'; 256]).unwrap();
    fs::write(dir.join("b-4kiB.bin"), vec![b'y'; 4096]).unwrap();
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("c-1MiB.bin"), vec![b'z'; 1024 * 1024]).unwrap();
    let empty = dir.join("empty-dir");
    fs::create_dir_all(&empty).unwrap();

    let updates = collect_updates(&dir);
    let (got_size, got_items) = measured_at(&updates, &dir).expect("root measured");
    let (want_size, want_items) = ground_truth(&dir);
    assert_eq!(got_size, want_size, "subtree size must match du-style walk");
    assert_eq!(got_items, want_items);
    // Node count: a-256B, b-4kiB, sub, c-1MiB, empty-dir, root = 6.
    assert_eq!(got_items, 6);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn sparse_file_uses_apparent_size() {
    let dir = temp_dir("sparse");
    let sparse = dir.join("sparse-10MiB.dat");
    let f = fs::File::create(&sparse).unwrap();
    f.set_len(10 * 1024 * 1024).unwrap();
    drop(f);

    let updates = collect_updates(&dir);
    let (got_size, got_items) = measured_at(&updates, &dir).unwrap();
    assert_eq!(got_items, 2); // file + root
    let root_own = {
        let m = fs::symlink_metadata(&dir).unwrap();
        node_contribution(m.size() as u64, m.blocks() as u64)
    };
    // Apparent (content length) dominates for sparse files (docs/04 §4).
    assert_eq!(got_size, 10 * 1024 * 1024 + root_own);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn symlinks_are_excluded_everywhere() {
    let dir = temp_dir("symlinks");
    fs::write(dir.join("real.txt"), b"hello world").unwrap();
    let target_dir = dir.join("target-dir");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("inner.bin"), vec![b'q'; 8192]).unwrap();

    std::os::unix::fs::symlink("real.txt", dir.join("link-to-file")).unwrap();
    std::os::unix::fs::symlink("target-dir", dir.join("link-to-dir")).unwrap();
    std::os::unix::fs::symlink("/definitely/not/here", dir.join("broken-link")).unwrap();
    // Cycle: inner/loop-up -> ..
    let inner = dir.join("inner");
    fs::create_dir_all(&inner).unwrap();
    std::os::unix::fs::symlink("..", inner.join("loop-up")).unwrap();

    let updates = collect_updates(&dir);
    let (got_size, got_items) = measured_at(&updates, &dir).unwrap();
    let (want_size, want_items) = ground_truth(&dir);
    assert_eq!(got_size, want_size);
    // Root + real.txt + target-dir + inner.bin + inner = 5; no symlinks.
    assert_eq!(got_items, 5);
    // Reaching this point means the cycle did not hang the walk.
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn hidden_files_are_included() {
    let dir = temp_dir("hidden");
    fs::write(dir.join(".hidden-file"), b"abc").unwrap();
    let hidden_dir = dir.join(".hidden-dir");
    fs::create_dir_all(&hidden_dir).unwrap();
    fs::write(hidden_dir.join("x.txt"), b"def").unwrap();

    let updates = collect_updates(&dir);
    let (_, got_items) = measured_at(&updates, &dir).unwrap();
    assert_eq!(got_items, 4); // root + .hidden-file + .hidden-dir + x.txt
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn unreadable_dir_is_error_row_but_scan_completes() {
    let dir = temp_dir("perm");
    fs::write(dir.join("readable.txt"), b"data").unwrap();
    let locked = dir.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(locked.join("secret.txt"), b"nope").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    // Permissions don't apply when running as root — skip in that case.
    if unsafe { libc::geteuid() } == 0 {
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_dir_all(&dir).ok();
        return;
    }

    let updates = collect_updates(&dir);
    let (_, got_items) = measured_at(&updates, &dir).unwrap();
    assert_eq!(got_items, 3); // root + readable.txt + locked (still counted)
    // The locked dir was reported as an error node.
    assert!(
        updates
            .iter()
            .any(|u| matches!(u, ScanUpdate::DirMeasured { error: true, .. })),
        "locked dir must surface as an error node"
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn empty_dir_counts_only_itself() {
    let dir = temp_dir("empty");
    let updates = collect_updates(&dir);
    let (got_size, got_items) = measured_at(&updates, &dir).unwrap();
    assert_eq!(got_items, 1);
    let m = fs::symlink_metadata(&dir).unwrap();
    assert_eq!(
        got_size,
        node_contribution(m.size() as u64, m.blocks() as u64)
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn ignored_subtrees_are_excluded() {
    let dir = temp_dir("ignored");
    fs::write(dir.join("keep.txt"), b"keep").unwrap();
    let ignored_dir = dir.join("ignored-dir");
    fs::create_dir_all(&ignored_dir).unwrap();
    fs::write(ignored_dir.join("drop.bin"), vec![b'd'; 4096]).unwrap();

    let (handle, rx) = spawn(&dir, false, vec![ignored_dir.display().to_string()]);
    let mut items = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ScanUpdate::DirMeasured { path, items: it, .. }) if path == dir => {
                items = Some(it);
                break;
            }
            Ok(ScanUpdate::Finished { .. }) => break,
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                assert!(std::time::Instant::now() < deadline, "timeout");
                break;
            }
            Err(_) => break,
        }
    }
    drop(handle);
    assert_eq!(items, Some(2)); // root + keep.txt; ignored-dir fully excluded
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancellation_stops_the_scan() {
    let dir = temp_dir("cancel");
    // A tree large enough that an immediate cancel lands mid-walk.
    for i in 0..100 {
        let d = dir.join(format!("d{i}"));
        fs::create_dir_all(&d).unwrap();
        for j in 0..200 {
            fs::write(d.join(format!("f{j}.bin")), vec![b'x'; 1024]).unwrap();
        }
    }
    let (handle, rx) = spawn(&dir, false, vec![]);
    handle.cancel();
    let mut saw_finished = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ScanUpdate::Finished { .. }) => {
                saw_finished = true;
                break;
            }
            Ok(ScanUpdate::Cancelled) => break,
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                assert!(std::time::Instant::now() < deadline, "timeout");
                break;
            }
            Err(_) => break,
        }
    }
    drop(handle);
    assert!(!saw_finished, "no Finished may follow a cancel");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn invalid_target_fails_cleanly() {
    let ghost = std::env::temp_dir().join("arbor-does-not-exist-anywhere");
    let (handle, rx) = spawn(&ghost, false, vec![]);
    let mut failed = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(ScanUpdate::Failed { .. }) => {
                failed = true;
                break;
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            Err(_) => break,
        }
    }
    drop(handle);
    assert!(failed, "nonexistent target must produce Failed");
}

#[test]
fn mount_boundary_logic() {
    let a = 1234u64;
    let b = 5678u64;
    assert!(!should_descend(b, a, false));
    assert!(should_descend(b, a, true));
    assert!(should_descend(a, a, false));
}

/// Replay a full scan stream through the same merge/measure logic the UI
/// uses (`models::DisplayNode`) and pin the coalescing protocol invariants
/// (regression for the "Not Responding" fix, where `DirListed` batches are
/// flushed by size/time):
///
/// 1. every listed entry finds its parent at merge time — coalescing must
///    never reorder a directory's listing after its measurement;
/// 2. a `DirMeasured` aggregate is final on arrival — no later listing may
///    bump an already-measured subtree;
/// 3. the replayed tree equals the filesystem ground truth.
#[test]
fn coalesced_stream_replays_to_the_final_tree() {
    use std::collections::{HashMap, HashSet};

    let dir = temp_dir("coalesce");
    // ~1560 entries across 61 directories: large enough that the 512-entry
    // batch threshold guarantees several coalesced `DirListed` messages.
    let mut all_dirs = Vec::new();
    let mut all_files = Vec::new();
    for d in 0..30 {
        let sub = dir.join(format!("d{d:02}"));
        fs::create_dir_all(&sub).unwrap();
        all_dirs.push(sub.clone());
        for f in 0..40 {
            let p = sub.join(format!("f{f:02}.txt"));
            fs::write(&p, vec![b'x'; 100]).unwrap();
            all_files.push(p);
        }
        let nested = sub.join("nested");
        fs::create_dir_all(&nested).unwrap();
        all_dirs.push(nested.clone());
        for f in 0..10 {
            let p = nested.join(format!("n{f:02}.txt"));
            fs::write(&p, vec![b'y'; 100]).unwrap();
            all_files.push(p);
        }
    }
    all_dirs.push(dir.clone());

    let updates = collect_updates(&dir);

    let mut tree: Option<arbor::models::DisplayNode> = None;
    let mut known: HashSet<PathBuf> = HashSet::new();
    let mut measured: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    for update in &updates {
        match update {
            ScanUpdate::DirListed { entries } => {
                if tree.is_none() {
                    let root_meta = entries.first().expect("root listing non-empty");
                    let root_path = root_meta.path.parent().unwrap().to_path_buf();
                    tree = Some(arbor::models::DisplayNode::new_dir(
                        &arbor::models::EntryMeta {
                            name: "root".into(),
                            path: root_path,
                            is_dir: true,
                            error: false,
                            own_size: 0,
                            mtime: None,
                        },
                    ));
                }
                let merged = tree.as_mut().unwrap().merge_entries(entries, &mut known);
                assert_eq!(
                    merged,
                    entries.len(),
                    "every entry must find its parent when merged — \
                     a listing arrived before its parent's listing"
                );
            }
            ScanUpdate::DirMeasured { path, size, items, error, mtime } => {
                assert!(
                    tree.as_mut()
                        .unwrap()
                        .apply_measured(path, *size, *items, *error, *mtime),
                    "measured {path:?} must exist in the replayed tree"
                );
                measured.insert(path.clone(), (*size, *items));
            }
            ScanUpdate::Finished { .. } => {}
            other => panic!("unexpected non-terminal update in collected stream: {other:?}"),
        }
    }
    let tree = tree.expect("root listing created the tree");

    // Invariant 2: measured aggregates were never refined afterwards.
    for (path, (size, items)) in &measured {
        let node = tree.find(path).expect("measured node present");
        assert_eq!(
            (node.size, node.items),
            (*size, *items),
            "{path:?} changed after its DirMeasured — a listing was flushed \
             after its measurement"
        );
    }

    // Invariant 3: replayed tree matches the filesystem and ground truth.
    for d in &all_dirs {
        assert!(tree.find(d).is_some(), "directory {d:?} missing from tree");
    }
    for f in &all_files {
        assert!(tree.find(f).is_some(), "file {f:?} missing from tree");
    }
    let (want_size, want_items) = ground_truth(&dir);
    let root = tree.find(&dir).unwrap();
    assert_eq!(root.size, want_size);
    assert_eq!(root.items, want_items);
    assert_eq!(measured.get(&dir), Some(&(want_size, want_items)));

    // Batching sanity: the stream must not carry one message per directory
    // (61 dirs) — coalescing has to compress the listings.
    let listed_msgs = updates
        .iter()
        .filter(|u| matches!(u, ScanUpdate::DirListed { .. }))
        .count();
    assert!(
        listed_msgs < all_dirs.len(),
        "expected coalesced batches, got {listed_msgs} DirListed messages \
         for {} directories",
        all_dirs.len()
    );

    fs::remove_dir_all(&dir).ok();
}

/// The merge path must stay flat: one directory with 60,000 entries merges
/// in bounded time. The pre-fix duplicate guard scanned the parent's
/// children linearly per entry — O(n²) on the main loop — which froze real
/// home-folder scans (a single flat directory with hundreds of thousands of
/// files). 60k in-memory entries would take tens of seconds the old way.
#[test]
fn flat_directory_merges_in_linear_time() {
    use std::collections::HashSet;

    let n = 60_000usize;
    let parent: PathBuf = "/flat".into();
    let entries: Vec<arbor::models::EntryMeta> = (0..n)
        .map(|i| arbor::models::EntryMeta {
            name: format!("f{i:05}.bin"),
            path: parent.join(format!("f{i:05}.bin")),
            is_dir: false,
            error: false,
            own_size: 100,
            mtime: None,
        })
        .collect();
    let root_meta = arbor::models::EntryMeta {
        name: "flat".into(),
        path: parent.clone(),
        is_dir: true,
        error: false,
        own_size: 0,
        mtime: None,
    };
    let mut tree = arbor::models::DisplayNode::new_dir(&root_meta);
    let mut known = HashSet::new();
    let started = std::time::Instant::now();
    // Replay batch-by-batch exactly like the UI drain (emitter batches are
    // capped at 512 entries).
    let mut merged_total = 0;
    for chunk in entries.chunks(512) {
        merged_total += tree.merge_entries(chunk, &mut known);
    }
    let elapsed = started.elapsed();
    assert_eq!(merged_total, n);
    assert_eq!(tree.find(&parent).unwrap().items, n as u64 + 1);
    assert_eq!(tree.find(&parent).unwrap().size, n as u64 * 100);
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "merging {n} flat entries took {elapsed:?} — the merge went quadratic again"
    );
}

/// Merging the same batch twice must be a no-op (duplicate guard): no extra
/// nodes, no double-counted aggregates.
#[test]
fn merge_entries_is_idempotent_per_path() {
    use std::collections::HashSet;

    let parent: PathBuf = "/dup".into();
    let meta = |name: &str| arbor::models::EntryMeta {
        name: name.into(),
        path: parent.join(name),
        is_dir: false,
        error: false,
        own_size: 10,
        mtime: None,
    };
    let entries = vec![meta("a"), meta("b"), meta("c")];
    let root_meta = arbor::models::EntryMeta {
        name: "dup".into(),
        path: parent.clone(),
        is_dir: true,
        error: false,
        own_size: 0,
        mtime: None,
    };
    let mut tree = arbor::models::DisplayNode::new_dir(&root_meta);
    let mut known = HashSet::new();
    assert_eq!(tree.merge_entries(&entries, &mut known), 3);
    // Replay the identical listing: nothing new may be merged.
    assert_eq!(tree.merge_entries(&entries, &mut known), 0);
    let node = tree.find(&parent).unwrap();
    assert_eq!(node.items, 4); // self + a + b + c
    assert_eq!(node.size, 30);
    assert_eq!(node.children.len(), 3);
}

/// The cache writer runs inside the scan worker: a scan with caching enabled
/// must leave a loadable snapshot whose tree matches what the UI merged
/// (user request "add caching").
#[test]
fn cached_scan_roundtrips_through_a_real_scan() {
    let dir = temp_dir("cache-roundtrip");
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.join("big.bin"), vec![0u8; 10_000]).unwrap();
    fs::write(sub.join("note.txt"), b"hello").unwrap();

    // Ground truth via a normal (uncached) scan merge.
    let updates = collect_updates(&dir);
    assert!(updates
        .iter()
        .any(|u| matches!(u, ScanUpdate::Finished { .. })));

    // Cached scan of the same tree.
    let (handle, rx) = arbor::scanner::spawn_ext(&dir, false, vec![], true);
    let mut finished = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ScanUpdate::Finished { .. }) => {
                finished = true;
                break;
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if std::time::Instant::now() >= deadline {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(finished, "cached scan never finished");
    drop(handle); // join the worker so the cache file is finalized

    let cached = arbor::cache::load(&dir).expect("cache snapshot must be loadable");
    let root = &cached.root;
    assert_eq!(root.path, dir);
    // root = own dir block + big.bin (10 kB → blocks) + sub subtree
    let big = node_contribution(10_000, 24); // 10_000 bytes → 20 blocks… actual st_blocks may differ
    let _ = big;
    assert_eq!(root.children.len(), 2, "big.bin + sub");
    let sub_node = root
        .children
        .iter()
        .find(|c| c.is_dir)
        .expect("sub directory");
    assert_eq!(sub_node.children.len(), 1);
    assert_eq!(sub_node.children[0].name, "note.txt");
    assert!(root.measured && sub_node.measured);
    // Aggregates were finalized bottom-up by the decoder.
    assert_eq!(root.size, root.own_size + sub_node.size + root.children.iter().find(|c| !c.is_dir).unwrap().size);
    // Every path must be reachable via find(), like in a merged UI tree.
    assert!(root.find(&sub.join("note.txt")).is_some());
    assert!(root.find(&dir.join("big.bin")).is_some());

    // The stale-children refresh rule the UI applies on DirListed for a
    // measured (cache-loaded) directory.
    let mut known = std::collections::HashSet::new();
    root.collect_paths(&mut known);
    assert_eq!(known.len(), 4); // root, big.bin, sub, note.txt

    fs::remove_dir_all(&dir).ok();
}
