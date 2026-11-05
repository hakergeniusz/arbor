//! Directory scanner (docs/03).
//!
//! Runs on a worker thread and reports progress over a GLib main-context
//! channel so the GUI stays responsive. Semantics per the specification:
//!
//! - snapshot semantics: what was read is what is shown; changes need a rescan
//! - hidden (dot-prefixed) entries are included
//! - symlinks are never followed, never listed, never counted (cycle-safe)
//! - unreadable directories become error rows (own contribution still counts)
//! - other filesystems are excluded unless `recursive_mounts` is set
//! - ignored locations (and everything below them) are excluded
//! - hardlinks are counted per path (decision D14)

// Some helpers are wired up in later phases or used only by unit tests.
#![allow(dead_code)]

use std::fs::{DirEntry, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;


use crate::models::{node_contribution, DisplayNode, EntryMeta};

/// Incremental scan output consumed by the UI.
#[derive(Debug, Clone)]
pub enum ScanUpdate {
    /// A directory's direct entries have been read and stat'ed.
    DirListed { entries: Vec<EntryMeta> },
    /// A folder's subtree aggregate is final.
    DirMeasured {
        path: PathBuf,
        size: u64,
        items: u64,
        error: bool,
        mtime: i64,
    },
    /// The whole scan finished successfully.
    Finished { root: PathBuf },
    /// The scan target is not a readable directory.
    Failed { path: PathBuf },
    /// The scan was cancelled; partial data is discarded by the UI.
    Cancelled,
}

pub struct ScanHandle {
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ScanHandle {
    /// Ask the worker to stop. Cheap, idempotent.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

impl Drop for ScanHandle {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Spawn a scan of `path`; updates arrive on the returned std channel, which
/// the UI drains on the main loop. The scan target itself is never subject to
/// the ignore list — the user picked it explicitly (decision D15 note).
pub fn spawn(
    path: &Path,
    recursive_mounts: bool,
    ignored: Vec<String>,
) -> (ScanHandle, std::sync::mpsc::Receiver<ScanUpdate>) {
    spawn_ext(path, recursive_mounts, ignored, false)
}

/// Like [`spawn`], with the on-disk scan cache enabled (the worker streams
/// one record per entry to the cache file; the main loop never pays).
pub fn spawn_ext(
    path: &Path,
    recursive_mounts: bool,
    ignored: Vec<String>,
    cache: bool,
) -> (ScanHandle, std::sync::mpsc::Receiver<ScanUpdate>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    let root = path.to_path_buf();
    let (sender, receiver) = std::sync::mpsc::channel::<ScanUpdate>();

    let thread = std::thread::Builder::new()
        .name("arbor-scanner".into())
        .spawn(move || {
            run_scan(&root, recursive_mounts, ignored, cancel_worker, &sender, cache);
        })
        .expect("scanner thread");

    (
        ScanHandle {
            cancel,
            thread: Some(thread),
        },
        receiver,
    )
}

/// True when a child directory on another device may be entered.
pub fn should_descend(child_dev: u64, parent_dev: u64, recursive_mounts: bool) -> bool {
    recursive_mounts || child_dev == parent_dev
}

/// Coalescing sender: `DirListed` batches are flushed by size or time so a
/// large tree produces a stream of chunky messages instead of one per
/// directory (a real home folder easily exceeds 10⁵ directories). Every
/// non-listed update first flushes pending listings — the UI merges a
/// directory's entries before its measurement may refine them, so
/// `DirListed(dir)` must always precede `DirMeasured(dir)` on the channel.
struct Emitter {
    sender: std::sync::mpsc::Sender<ScanUpdate>,
    pending: Vec<EntryMeta>,
    last_flush: std::time::Instant,
}

impl Emitter {
    fn new(sender: std::sync::mpsc::Sender<ScanUpdate>) -> Self {
        Emitter {
            sender,
            pending: Vec::new(),
            last_flush: std::time::Instant::now(),
        }
    }

    const MAX_BATCH: usize = 512;
    const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

    fn list(&mut self, entries: Vec<EntryMeta>) {
        self.pending.extend(entries);
        if self.pending.len() >= Self::MAX_BATCH
            || self.last_flush.elapsed() >= Self::FLUSH_INTERVAL
        {
            self.flush();
        }
    }

    /// Send the dir listing immediately (used for the scan root, whose
    /// message must exist even when the folder is empty).
    fn list_now(&mut self, entries: Vec<EntryMeta>) {
        self.flush();
        if !entries.is_empty() {
            let _ = self
                .sender
                .send(ScanUpdate::DirListed { entries });
        }
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            let entries = std::mem::take(&mut self.pending);
            let _ = self.sender.send(ScanUpdate::DirListed { entries });
        }
        self.last_flush = std::time::Instant::now();
    }

    /// Send a non-listing update, flushing pending listings first.
    fn send(&mut self, update: ScanUpdate) {
        self.flush();
        let _ = self.sender.send(update);
    }
}

fn run_scan(
    root: &Path,
    recursive_mounts: bool,
    ignored: Vec<String>,
    cancel: Arc<AtomicBool>,
    sender: &std::sync::mpsc::Sender<ScanUpdate>,
    cache: bool,
) {
    let mut emit = Emitter::new(sender.clone());
    // Streaming cache writer (one flat record per lstat'ed entry, walk
    // order) — the main loop never pays for encoding the tree.
    let mut writer = if cache {
        match std::fs::metadata(root) {
            Ok(m) => crate::cache::Writer::create(root, m.mtime()),
            Err(_) => None,
        }
    } else {
        None
    };
    let meta = match std::fs::metadata(root) {
        Ok(m) if m.is_dir() => m,
        _ => {
            emit.send(ScanUpdate::Failed { path: root.to_path_buf() });
            return;
        }
    };

    // Iterative post-order walk; an explicit stack avoids stack overflow on
    // deeply nested trees.
    struct Frame {
        path: PathBuf,
        dev: u64,
        own_size: u64,
        mtime: i64,
        #[allow(dead_code)]
        children: Vec<DisplayNode>,
        child_contrib: u64,
        child_items: u64,
        /// This directory's record index in the cache stream (parent index
        /// for everything listed inside it).
        record_index: Option<u32>,
        pending: std::vec::IntoIter<DirEntry>,
    }

    let entries = match list_dir(root, &ignored) {
        Ok(entries) => entries,
        Err(_) => {
            emit.send(ScanUpdate::Failed { path: root.to_path_buf() });
            return;
        }
    };
    // The root listing goes out on its own even when empty, so the UI can
    // create the root node (and show an empty result) immediately.
    emit.list_now(entry_metas(&entries));

    let root_meta_size = node_contribution(meta.size(), meta.blocks());
    let root_record = writer.as_mut().and_then(|w| {
        w.record(
            None,
            &root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| root.display().to_string()),
            true,
            false,
            root_meta_size,
            Some(meta.mtime()),
        )
    });
    let mut stack = vec![Frame {
        path: root.to_path_buf(),
        dev: meta.dev(),
        own_size: root_meta_size,
        mtime: meta.mtime(),
        children: Vec::new(),
        child_contrib: 0,
        child_items: 0,
        record_index: root_record,
        pending: entries
            .into_iter()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>()
            .into_iter(),
    }];

    while let Some(frame) = stack.last_mut() {
        if cancel.load(Ordering::SeqCst) {
            emit.send(ScanUpdate::Cancelled);
            return;
        }
        let Some(entry) = frame.pending.next() else {
            // Frame complete: finalize and pop. Folder size = the node's own
            // contribution plus every descendant's (docs/04 §4: "a folder
            // row sums its own and all descendants' contributions").
            let frame = stack.pop().unwrap();
            let size = frame.own_size + frame.child_contrib;
            let items = 1 + frame.child_items;
            emit.send(ScanUpdate::DirMeasured {
                path: frame.path.clone(),
                size,
                items,
                error: false,
                mtime: frame.mtime,
            });
            if let Some(parent) = stack.last_mut() {
                parent.child_contrib += size;
                parent.child_items += items;
                parent.children.push(DisplayNode {
                    name: frame
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| frame.path.display().to_string()),
                    path: frame.path,
                    is_dir: true,
                    error: false,
                    own_size: frame.own_size,
                    size,
                    items,
                    mtime: Some(frame.mtime),
                    measured: true,
                    children: Vec::new(),
                });
            } else {
                if let Some(w) = writer.take() {
                    w.finish();
                }
                emit.send(ScanUpdate::Finished {
                    root: frame.path,
                });
            }
            continue;
        };

        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue; // excluded everywhere (docs/03 §3)
        }
        let Ok(meta) = entry.metadata() else { continue }; // vanished mid-scan
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        if file_type.is_dir() {
            if !should_descend(meta.dev(), frame.dev, recursive_mounts) {
                continue; // other filesystem excluded (decision D1)
            }
            if is_ignored(&path, &ignored) {
                continue;
            }
            match list_dir(&path, &ignored) {
                Err(_) => {
                    // Unreadable directory: error row, own contribution only.
                    let own = node_contribution(meta.size(), meta.blocks());
                    if let (Some(w), Some(parent_idx)) = (&mut writer, frame.record_index) {
                        w.record(Some(parent_idx), &name, true, true, own, Some(meta.mtime()));
                    }
                    emit.list(vec![EntryMeta {
                        name: name.clone(),
                        path: path.clone(),
                        is_dir: true,
                        error: true,
                        own_size: own,
                        mtime: Some(meta.mtime()),
                    }]);
                    emit.send(ScanUpdate::DirMeasured {
                        path: path.clone(),
                        size: own,
                        items: 1,
                        error: true,
                        mtime: meta.mtime(),
                    });
                    frame.child_contrib += own;
                    frame.child_items += 1;
                }
                Ok(entries) => {
                    emit.list(entry_metas(&entries));
                    let dir_idx = match (&mut writer, frame.record_index) {
                        (Some(w), Some(parent_idx)) => w.record(
                            Some(parent_idx),
                            &name,
                            true,
                            false,
                            node_contribution(meta.size(), meta.blocks()),
                            Some(meta.mtime()),
                        ),
                        _ => None,
                    };
                    // The dir node's own contribution and item are accounted
                    // when its frame completes (below) — do not add them
                    // here, or they would be counted twice.
                    stack.push(Frame {
                        path,
                        dev: meta.dev(),
                        own_size: node_contribution(meta.size(), meta.blocks()),
                        mtime: meta.mtime(),
                        children: Vec::new(),
                        child_contrib: 0,
                        child_items: 0,
                        record_index: dir_idx,
                        pending: entries
                            .into_iter()
                            .map(|(_, e)| e)
                            .collect::<Vec<_>>()
                            .into_iter(),
                    });
                }
            }
        } else {
            let own = node_contribution(meta.size(), meta.blocks());
            if let (Some(w), Some(parent_idx)) = (&mut writer, frame.record_index) {
                w.record(Some(parent_idx), &name, false, false, own, Some(meta.mtime()));
            }
            frame.child_contrib += own;
            frame.child_items += 1;
        }
    }
}

/// Read a directory and lstat every non-symlink entry.
fn list_dir(path: &Path, ignored: &[String]) -> std::io::Result<Vec<(Metadata, DirEntry)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        let entry_path = entry.path();
        if file_type.is_dir() && is_ignored(&entry_path, ignored) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        out.push((meta, entry));
    }
    Ok(out)
}

/// Map a directory listing to the wire format (name/path/size metadata).
fn entry_metas(entries: &[(Metadata, DirEntry)]) -> Vec<EntryMeta> {
    entries
        .iter()
        .map(|(meta, entry)| EntryMeta {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path(),
            is_dir: meta.is_dir(),
            error: false,
            own_size: node_contribution(meta.size(), meta.blocks()),
            mtime: Some(meta.mtime()),
        })
        .collect()
}

fn is_ignored(path: &Path, ignored: &[String]) -> bool {
    ignored.iter().any(|ig| path.starts_with(ig))
}
