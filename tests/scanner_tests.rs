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
