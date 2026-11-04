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

    /// Remove the node at `path` from the tree and recompute every
    /// ancestor's aggregates (docs/06 R8: fresh totals after a trash
    /// operation). Returns the detached node.
    pub fn detach(&mut self, path: &Path) -> Option<DisplayNode> {
        let Some(parent_path) = path.parent() else { return None };
        let Some(parent) = self.find_mut(parent_path) else { return None };
        let idx = parent.children.iter().position(|c| c.path == path)?;
        let removed = parent.children.remove(idx);
        // Recompute the whole ancestor chain from the remaining children.
        let mut ancestor = parent_path;
        loop {
            let Some(node) = self.find_mut(ancestor) else { break };
            node.finalize_from_children();
            match ancestor.parent() {
                Some(p) => ancestor = p,
                None => break,
            }
        }
        Some(removed)
    }
}

/// Display size of a single node: larger of content length and block usage
/// (docs/04 §4). Kept here so scanner and tests share one definition.
pub fn node_contribution(st_size: u64, st_blocks: u64) -> u64 {
    st_size.max(st_blocks * 512)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str, own: u64, size: u64, items: u64, children: Vec<DisplayNode>) -> DisplayNode {
        DisplayNode {
            name: name.into(),
            path: PathBuf::from(name),
            is_dir: true,
            error: false,
            own_size: own,
            size,
            items,
            mtime: None,
            measured: true,
            children,
        }
    }

    fn file(name: &str, size: u64) -> DisplayNode {
        DisplayNode {
            name: name.into(),
            path: PathBuf::from(name),
            is_dir: false,
            error: false,
            own_size: size,
            size,
            items: 1,
            mtime: None,
            measured: true,
            children: Vec::new(),
        }
    }

    #[test]
    fn detach_removes_subtree_and_recomputes_ancestors() {
        // root (own 10) -> a (own 5) -> {a1 100, a2 200}; b (file 400)
        let mut root = dir(
            "/root",
            10,
            715,
            6,
            vec![
                dir(
                    "/root/a",
                    5,
                    305,
                    3,
                    vec![file("/root/a/a1", 100), file("/root/a/a2", 200)],
                ),
                file("/root/b", 400),
            ],
        );
        let removed = root.detach(Path::new("/root/a")).expect("detached");
        assert_eq!(removed.path, Path::new("/root/a"));
        assert_eq!(removed.children.len(), 2);
        // Root recomputed: own 10 + b 400 = 410 bytes; items: root + b = 2.
        assert_eq!(root.size, 410);
        assert_eq!(root.items, 2);
        assert_eq!(root.children.len(), 1);

        // Detaching a file works the same way.
        let removed = root.detach(Path::new("/root/b")).expect("detached");
        assert_eq!(removed.size, 400);
        assert_eq!(root.size, 10);
        assert_eq!(root.items, 1);
        assert!(root.children.is_empty());
    }

    #[test]
    fn detach_missing_path_is_noop() {
        let mut root = dir("/root", 10, 110, 2, vec![file("/root/b", 100)]);
        assert!(root.detach(Path::new("/root/nope")).is_none());
        assert!(root.detach(Path::new("/other/b")).is_none());
        assert_eq!(root.size, 110);
        assert_eq!(root.children.len(), 1);
    }
}
