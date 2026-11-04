//! User interface layer.


// Some helpers are wired up in later implementation phases or used only
// by unit tests.
#![allow(dead_code)]

pub mod about;
pub mod chooser;
pub mod preferences;
pub mod result_view;
pub mod shortcuts;
pub mod start_page;
pub mod window;

use std::path::PathBuf;

/// Chart display mode, per-window state (docs/02 §5, decision D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartMode {
    Rings,
    Treemap,
}

/// A selectable location on the start page (docs/01 §1–2).
#[derive(Debug, Clone)]
pub enum StartLocation {
    /// The user's home folder (fixed row).
    Home,
    /// A mounted filesystem (fixed row).
    Mount(crate::mounts::MountInfo),
    /// A recently scanned folder.
    Recent(PathBuf),
}

impl StartLocation {
    pub fn display_name(&self) -> String {
        match self {
            StartLocation::Home => "Home Folder".to_string(),
            StartLocation::Mount(m) => m.name.clone(),
            StartLocation::Recent(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.display().to_string()),
        }
    }
}
