//! User-facing persistent state (docs/01 §2, docs/03 §1, decisions D1–D3, D15).
//!
//! Stored as JSON under `$XDG_STATE_HOME/arbor/state.json` (fallback:
//! `$HOME/.local/state/arbor/state.json`). Kept free of GTK types so it can
//! be unit-tested without a display.


// Some helpers are wired up in later implementation phases or used only
// by unit tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Everything Arbor persists between runs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Recently scanned folders, newest first (docs/01 §2).
    #[serde(default)]
    pub recents: Vec<String>,
    /// Locations excluded from scans (docs/01 §6, decision D15).
    #[serde(default)]
    pub ignored: Vec<String>,
    /// "Recursively analyze mount points" checkbox (docs/03 §1, decision D1).
    #[serde(default)]
    pub recursive_mounts: bool,
    /// Remembered window size; None → 960×600 default (docs/01 §1, D3).
    #[serde(default)]
    pub window_size: Option<WindowSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: i32,
    pub height: i32,
}

impl Settings {
    /// Insert/update a recent path at the front, de-duplicating and capping
    /// the list at a sane length.
    pub fn push_recent(&mut self, path: &str) {
        self.recents.retain(|p| p != path);
        self.recents.insert(0, path.to_string());
        self.recents.truncate(20);
    }

    pub fn clear_recents(&mut self) {
        self.recents.clear();
    }

    pub fn add_ignored(&mut self, path: &str) {
        if !self.ignored.iter().any(|p| p == path) {
            self.ignored.push(path.to_string());
        }
    }

    pub fn remove_ignored(&mut self, path: &str) {
        self.ignored.retain(|p| p != path);
    }

    /// True when `path` is an ignored location or located inside one (D15).
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.ignored
            .iter()
            .any(|ig| path.starts_with(ig) || Path::new(ig).starts_with(path))
    }

    pub fn load_from(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Default on-disk location: `$XDG_STATE_HOME/arbor/state.json`.
    pub fn default_path() -> Option<PathBuf> {
        if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
            if !state.is_empty() {
                return Some(PathBuf::from(state).join("arbor").join("state.json"));
            }
        }
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/state/arbor/state.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recents_order_and_dedup() {
        let mut s = Settings::default();
        s.push_recent("/a");
        s.push_recent("/b");
        s.push_recent("/a");
        assert_eq!(s.recents, vec!["/a", "/b"]);
        s.clear_recents();
        assert!(s.recents.is_empty());
    }

    #[test]
    fn ignore_semantics() {
        let mut s = Settings::default();
        s.add_ignored("/tmp/x");
        s.add_ignored("/tmp/x");
        assert_eq!(s.ignored.len(), 1);
        assert!(s.is_ignored(Path::new("/tmp/x/sub/deep")));
        assert!(!s.is_ignored(Path::new("/tmp/xy")));
        s.remove_ignored("/tmp/x");
        assert!(!s.is_ignored(Path::new("/tmp/x")));
    }

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("arbor-test-{}", std::process::id()));
        let path = dir.join("state.json");
        let mut s = Settings::default();
        s.push_recent("/some/dir");
        s.recursive_mounts = true;
        s.window_size = Some(WindowSize { width: 800, height: 600 });
        s.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(s, loaded);
        std::fs::remove_dir_all(&dir).ok();
    }
}
