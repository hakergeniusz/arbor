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
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    let root = path.to_path_buf();
    let (sender, receiver) = std::sync::mpsc::channel::<ScanUpdate>();

    let thread = std::thread::Builder::new()
        .name("arbor-scanner".into())
        .spawn(move || {
            run_scan(&root, recursive_mounts, ignored, cancel_worker, &sender);
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

fn run_scan(
    root: &Path,
    recursive_mounts: bool,
    ignored: Vec<String>,
    cancel: Arc<AtomicBool>,
    sender: &std::sync::mpsc::Sender<ScanUpdate>,
) {
    let meta = match std::fs::metadata(root) {
        Ok(m) if m.is_dir() => m,
        _ => {
            let _ = sender.send(ScanUpdate::Failed { path: root.to_path_buf() });
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
        children: Vec<DisplayNode>,
        child_contrib: u64,
        child_items: u64,
        pending: std::vec::IntoIter<DirEntry>,
    }

    let entries = match list_dir(root, &ignored) {
        Ok(entries) => entries,
        Err(_) => {
            let _ = sender.send(ScanUpdate::Failed { path: root.to_path_buf() });
            return;
        }
    };
    send_listed(&entries, sender);

    let root_meta_size = node_contribution(meta.size(), meta.blocks());
    let mut stack = vec![Frame {
        path: root.to_path_buf(),
        dev: meta.dev(),
        own_size: root_meta_size,
        mtime: meta.mtime(),
        children: Vec::new(),
        child_contrib: 0,
        child_items: 0,
        pending: entries
            .into_iter()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>()
            .into_iter(),
    }];

    while let Some(frame) = stack.last_mut() {
        if cancel.load(Ordering::SeqCst) {
            let _ = sender.send(ScanUpdate::Cancelled);
            return;
        }
        let Some(entry) = frame.pending.next() else {
            // Frame complete: finalize and pop. Folder size = the node's own
            // contribution plus every descendant's (docs/04 §4: "a folder
            // row sums its own and all descendants' contributions").
            let frame = stack.pop().unwrap();
            let size = frame.own_size + frame.child_contrib;
            let items = 1 + frame.child_items;
            let _ = sender.send(ScanUpdate::DirMeasured {
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
                let _ = sender.send(ScanUpdate::Finished {
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
                    let _ = sender.send(ScanUpdate::DirListed {
                        entries: vec![EntryMeta {
                            name: name.clone(),
                            path: path.clone(),
                            is_dir: true,
                            error: true,
                            own_size: own,
                            mtime: Some(meta.mtime()),
                        }],
                    });
                    let _ = sender.send(ScanUpdate::DirMeasured {
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
                    send_listed(&entries, sender);
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

fn send_listed(entries: &[(Metadata, DirEntry)], sender: &std::sync::mpsc::Sender<ScanUpdate>) {
    let metas = entries
        .iter()
        .map(|(meta, entry)| EntryMeta {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path(),
            is_dir: meta.is_dir(),
            error: false,
            own_size: node_contribution(meta.size(), meta.blocks()),
            mtime: Some(meta.mtime()),
        })
        .collect();
    let _ = sender.send(ScanUpdate::DirListed { entries: metas });
}

fn is_ignored(path: &Path, ignored: &[String]) -> bool {
    ignored.iter().any(|ig| path.starts_with(ig))
}
