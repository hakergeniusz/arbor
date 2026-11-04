//! Trash backend (docs/06 R5–R6).
//!
//! Deletion goes through GIO's trash implementation — the platform
//! mechanism — which writes XDG `.trashinfo` metadata and never falls back
//! to permanent deletion. Restore (Undo) is conservative: a plain rename
//! back to the original path, refusing to overwrite anything.

// The undo scan helpers are exercised by unit tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gio::prelude::*;
use gtk::gio;
use gtk::glib;

/// Everything needed to undo one trash operation.
#[derive(Debug, Clone)]
pub struct TrashRecord {
    /// The item's original absolute path.
    pub original: PathBuf,
    /// Candidate trash directories to consult (topdir trash first, then the
    /// home trash).
    pub search_dirs: Vec<PathBuf>,
}

/// Build the undo record for a path about to be trashed: the trash
/// directories GIO would have used for it (same-device topdir trash and the
/// home trash), ordered newest-relevant first.
pub fn undo_record(original: &Path) -> TrashRecord {
    TrashRecord {
        original: original.to_path_buf(),
        search_dirs: candidate_trash_dirs(original),
    }
}

/// Trash directories that may hold an entry for `original`:
/// the mount-root trash of the file's device (`.Trash-$uid`, then
/// `.Trash/$uid`), then the home trash.
fn candidate_trash_dirs(original: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let uid = unsafe { libc::geteuid() };
    // Walk up to the highest directory on the same device (the mount root).
    let mut top: &Path = original;
    if let Ok(meta) = std::fs::metadata(original) {
        let dev = dev_of(&meta);
        let mut cur = original.parent();
        while let Some(p) = cur {
            match std::fs::metadata(p) {
                Ok(m) if dev_of(&m) == dev => {
                    top = p;
                    cur = p.parent();
                }
                _ => break,
            }
        }
    }
    dirs.push(top.join(format!(".Trash-{uid}")));
    dirs.push(top.join(".Trash").join(uid.to_string()));
    dirs.push(glib::user_data_dir().join("Trash"));
    dirs
}

#[cfg(target_os = "linux")]
fn dev_of(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.dev()
}

/// A found trash entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TrashEntry {
    /// Directory holding the trashed item (…/files).
    pub files_dir: PathBuf,
    /// Item name inside `files_dir`.
    pub item_name: String,
    /// The `.trashinfo` file.
    pub info_path: PathBuf,
}

/// Find the newest trash entry whose recorded `Path=` equals `original`.
pub fn find_entry(record: &TrashRecord) -> Option<TrashEntry> {
    let mut best: Option<(std::time::SystemTime, TrashEntry)> = None;
    for dir in &record.search_dirs {
        let info_dir = dir.join("info");
        let files_dir = dir.join("files");
        let Ok(read) = std::fs::read_dir(&info_dir) else { continue };
        for info_entry in read.flatten() {
            let path = info_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("trashinfo") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            if record.original.display().to_string() != recorded_path(&content).unwrap_or_default() {
                continue;
            }
            let name = path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !files_dir.join(&name).exists() {
                continue;
            }
            let mtime = info_entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best
                .as_ref()
                .map(|(t, _)| mtime > *t)
                .unwrap_or(true)
            {
                best = Some((
                    mtime,
                    TrashEntry {
                        files_dir: files_dir.clone(),
                        item_name: name,
                        info_path: path,
                    },
                ));
            }
        }
    }
    best.map(|(_, e)| e)
}

/// Extract the (URL-decoded) `Path=` value from `.trashinfo` content.
fn recorded_path(content: &str) -> Option<String> {
    let line = content.lines().find(|l| l.starts_with("Path="))?;
    let raw = &line["Path=".len()..];
    if !raw.contains('%') {
        return Some(raw.to_string());
    }
    // Percent-decode as raw bytes (paths on Linux are bytes; XDG spec says
    // gio escapes with percent-encoding).
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// Restore a trashed item to its original path. Conservative: refuses to
/// overwrite, requires the parent directory to exist, and on any failure
/// leaves the trash entry untouched.
pub fn restore(entry: &TrashEntry, original: &Path) -> Result<(), String> {
    let from = entry.files_dir.join(&entry.item_name);
    if !from.exists() {
        return Err("trashed item is gone".into());
    }
    if original.exists() {
        return Err("original path already exists".into());
    }
    let Some(parent) = original.parent() else {
        return Err("no parent directory".into());
    };
    if !parent.exists() {
        return Err("original parent directory is gone".into());
    }
    std::fs::rename(&from, original).map_err(|e| e.to_string())?;
    // Metadata cleanup is cosmetic; ignore failure.
    let _ = std::fs::remove_file(&entry.info_path);
    Ok(())
}

/// Move an item to the trash via GIO (writes XDG metadata, no deletion
/// fallback — docs/06 R5).
pub fn trash(path: &Path) -> Result<(), String> {
    gio::File::for_path(path)
        .trash(None::<&gio::Cancellable>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arbor-trash-test-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_entry_by_recorded_path() {
        let base = tempdir("find");
        let trash_dir = base.join(".Trash-1000");
        std::fs::create_dir_all(trash_dir.join("files")).unwrap();
        std::fs::create_dir_all(trash_dir.join("info")).unwrap();
        let original = base.join("victim.txt");
        std::fs::write(&original, b"x").unwrap();
        std::fs::rename(&original, trash_dir.join("files").join("victim.txt")).unwrap();
        std::fs::write(
            trash_dir.join("info").join("victim.txt.trashinfo"),
            "[Trash Info]\nPath=/does/not/match\nDeletionDate=2026-01-01T10:00:00\n",
        )
        .unwrap();
        let record = TrashRecord {
            original: base.join("other.txt"),
            search_dirs: vec![trash_dir.clone()],
        };
        assert!(find_entry(&record).is_none());

        std::fs::write(
            trash_dir.join("info").join("victim.txt.trashinfo"),
            format!(
                "[Trash Info]\nPath={}\nDeletionDate=2026-01-01T10:00:00\n",
                base.join("victim.txt").display()
            ),
        )
        .unwrap();
        let record = TrashRecord {
            original: base.join("victim.txt"),
            search_dirs: vec![trash_dir.clone()],
        };
        let entry = find_entry(&record).expect("entry found");
        assert_eq!(entry.item_name, "victim.txt");

        // Restore puts the file back and clears the metadata.
        restore(&entry, &base.join("victim.txt")).expect("restore");
        assert!(base.join("victim.txt").exists());
        assert!(!trash_dir.join("files").join("victim.txt").exists());
        assert!(!trash_dir.join("info").join("victim.txt.trashinfo").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn restore_refuses_to_overwrite_or_escape() {
        let base = tempdir("overwrite");
        let trash_dir = base.join("trash");
        std::fs::create_dir_all(trash_dir.join("files")).unwrap();
        std::fs::create_dir_all(trash_dir.join("info")).unwrap();
        std::fs::write(trash_dir.join("files").join("f"), b"trashed").unwrap();
        std::fs::write(
            trash_dir.join("info").join("f.trashinfo"),
            format!("[Trash Info]\nPath={}\n", base.join("existing").display()),
        )
        .unwrap();
        std::fs::write(base.join("existing"), b"precious").unwrap();
        let entry = find_entry(&TrashRecord {
            original: base.join("existing"),
            search_dirs: vec![trash_dir],
        })
        .expect("entry");
        assert_eq!(
            restore(&entry, &base.join("existing")),
            Err("original path already exists".into())
        );
        assert_eq!(std::fs::read(base.join("existing")).unwrap(), b"precious");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn restore_fails_when_parent_gone() {
        let base = tempdir("gone-parent");
        let trash_dir = base.join("trash");
        std::fs::create_dir_all(trash_dir.join("files")).unwrap();
        std::fs::create_dir_all(trash_dir.join("info")).unwrap();
        std::fs::write(trash_dir.join("files").join("f"), b"x").unwrap();
        std::fs::write(
            trash_dir.join("info").join("f.trashinfo"),
            format!("[Trash Info]\nPath={}\n", base.join("sub").join("f").display()),
        )
        .unwrap();
        let entry = find_entry(&TrashRecord {
            original: base.join("sub").join("f"),
            search_dirs: vec![trash_dir],
        })
        .expect("entry");
        assert!(restore(&entry, &base.join("sub").join("f")).is_err());
        // The entry must still be intact for a later attempt.
        assert!(entry.files_dir.join("f").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
