//! Mounted-filesystem enumeration for the start page (docs/01 §1).
//!
//! Reads `/proc/self/mounts` directly (no daemon required — the hermetic GUI
//! session has no udisks) and keeps real block-device filesystems that are
//! not internal pseudo locations. Live available/total values are queried
//! through GIO `filesystem::` attributes. Names prefer the filesystem label
//! (via `/dev/disk/by-label`), falling back to the device name.

// Some helpers are wired up in later implementation phases or used only
// by unit tests.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::gio;

#[derive(Debug, Clone)]
pub struct MountInfo {
    /// Mount point, e.g. `/`.
    pub root: PathBuf,
    /// Display label: filesystem label or device name.
    pub name: String,
    /// Total bytes of the filesystem.
    pub total: u64,
    /// Free bytes of the filesystem.
    pub available: u64,
}

/// List user-visible mounted filesystems, root filesystem first.
pub fn list_mounts() -> Vec<MountInfo> {
    let labels = device_labels();
    let mut out: Vec<MountInfo> = Vec::new();
    let mut seen_roots: Vec<PathBuf> = Vec::new();
    let mut seen_devices: Vec<String> = Vec::new();

    let Ok(data) = std::fs::read_to_string("/proc/self/mounts") else {
        return out;
    };
    let mut entries: Vec<MountInfo> = Vec::new();
    for line in data.lines() {
        let mut fields = line.split_whitespace();
        let (Some(dev), Some(mnt), Some(_fstype)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !dev.starts_with("/dev/") || dev.starts_with("/dev/loop") {
            continue; // network/pseudo mounts are not start-page targets
        }
        let root = unescape_mount_path(mnt);
        if is_internal_location(&root) {
            continue;
        }
        let dev_short = dev.rsplit('/').next().unwrap_or(dev).to_string();
        // Multiple bind mounts of the same device: keep the first occurrence.
        if seen_devices.contains(&dev_short) {
            continue;
        }
        seen_devices.push(dev_short.clone());
        let name = labels.get(&dev_short).cloned().unwrap_or(dev_short);
        entries.push(MountInfo {
            root,
            name,
            total: 0,
            available: 0,
        });
    }

    // Root filesystem first, then lexicographic by mount point.
    entries.sort_by(|a, b| {
        let a_root = a.root.as_os_str() == "/";
        let b_root = b.root.as_os_str() == "/";
        b_root.cmp(&a_root).then_with(|| a.root.cmp(&b.root))
    });
    for mut entry in entries {
        if seen_roots.contains(&entry.root) {
            continue;
        }
        seen_roots.push(entry.root.clone());
        let (total, available) = fs_usage(&entry.root).unwrap_or((0, 0));
        entry.total = total;
        entry.available = available;
        out.push(entry);
    }
    out
}

/// Map device basename → filesystem label via `/dev/disk/by-label`.
fn device_labels() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-label") else {
        return map;
    };
    for entry in entries.flatten() {
        let label = entry.file_name().to_string_lossy().to_string();
        if let Ok(target) = std::fs::read_link(entry.path()) {
            let dev = target
                .to_string_lossy()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            map.insert(dev, label);
        }
    }
    map
}

fn unescape_mount_path(mnt: &str) -> PathBuf {
    PathBuf::from(mnt.replace("\\040", " ").replace("\\134", "\\"))
}

/// Heuristic filter for locations that are never user-facing scan targets.
fn is_internal_location(root: &Path) -> bool {
    let s = root.to_string_lossy();
    ["/proc", "/sys", "/dev", "/run", "/var/run", "/boot/efi", "/efi"]
        .iter()
        .any(|p| s == *p || s.starts_with(&format!("{p}/")))
}

/// Query (total, free) bytes for a filesystem via GIO.
fn fs_usage(root: &Path) -> Option<(u64, u64)> {
    let file = gio::File::for_path(root);
    let info = file
        .query_filesystem_info(
            "filesystem::size,filesystem::free",
            None::<&gio::Cancellable>,
        )
        .ok()?;
    Some((
        info.attribute_uint64(gio::FILE_ATTRIBUTE_FILESYSTEM_SIZE),
        info.attribute_uint64(gio::FILE_ATTRIBUTE_FILESYSTEM_FREE),
    ))
}
