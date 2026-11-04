//! Data model for scan results (docs/03 §3, docs/04 §4).
//!
//! `EntryMeta` is what the scanner emits per listed entry; `DisplayNode` is
//! the merged, progressively-updated tree the UI and charts render.

// Some helpers are consumed by later phases or unit tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Metadata of one directory entry as reported by the scanner.
#[derive(Debug, Clone)]
pub struct EntryMeta {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// True when the directory exists but could not be read (chmod 000).
    pub error: bool,
    /// `max(st_size, st_blocks × 512)` for this node itself.
    pub own_size: u64,
    pub mtime: Option<i64>,
}

/// A node of the displayed scan tree. Sizes are subtree aggregates for
/// folders (own contribution for files); counts include the node itself and
/// exclude symlinks (docs/04 §4).
#[derive(Debug, Clone)]
pub struct DisplayNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub error: bool,
    /// The node's own `max(st_size, st_blocks × 512)` contribution.
    pub own_size: u64,
    /// Subtree aggregate: own_size + Σ children (folders).
    pub size: u64,
    pub items: u64,
    pub mtime: Option<i64>,
    /// False while the folder's subtree is still being measured.
    pub measured: bool,
    pub children: Vec<DisplayNode>,
}

impl DisplayNode {
    pub fn new_dir(meta: &EntryMeta) -> Self {
        Self {
            name: meta.name.clone(),
            path: meta.path.clone(),
            is_dir: true,
            error: meta.error,
            own_size: meta.own_size,
            size: meta.own_size,
            items: 1,
            mtime: meta.mtime,
            measured: false,
            children: Vec::new(),
        }
    }

    pub fn new_file(meta: &EntryMeta) -> Self {
        Self {
            name: meta.name.clone(),
            path: meta.path.clone(),
            is_dir: false,
            error: false,
            own_size: meta.own_size,
            size: meta.own_size,
            items: 1,
            mtime: meta.mtime,
            measured: true,
            children: Vec::new(),
        }
    }

    /// Depth-first search for a node by exact path (read-only).
    pub fn find(&self, path: &Path) -> Option<&DisplayNode> {
        self.find_impl(path)
    }

    /// Depth-first search for a node by exact path (mutable).
    pub fn find_mut(&mut self, path: &Path) -> Option<&mut DisplayNode> {
        self.find_mut_impl(path)
    }

    fn find_impl(&self, path: &Path) -> Option<&DisplayNode> {
        if self.path == path {
            return Some(self);
        }
        if !path.starts_with(&self.path) {
            return None;
        }
        for child in &self.children {
            if let Some(found) = child.find_impl(path) {
                return Some(found);
            }
        }
        None
    }

    fn find_mut_impl(&mut self, path: &Path) -> Option<&mut DisplayNode> {
        if self.path == path {
            return Some(self);
        }
        if !path.starts_with(&self.path) {
            return None;
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(path) {
                return Some(found);
            }
        }
        None
    }

    /// Finalize a folder's aggregates from the currently known children.
    pub fn finalize_from_children(&mut self) {
        self.size = self.own_size + self.children.iter().map(|c| c.size).sum::<u64>();
        self.items = 1 + self.children.iter().map(|c| c.items).sum::<u64>();
        self.measured = true;
    }
}

/// Display size of a single node: larger of content length and block usage
/// (docs/04 §4). Kept here so scanner and tests share one definition.
pub fn node_contribution(st_size: u64, st_blocks: u64) -> u64 {
    st_size.max(st_blocks * 512)
}
