//! Original chart palette (docs/10 §5): distinct hues for the scan root's
//! top-level children, descendants inherit the parent hue with lightness
//! variation; permission-error rows render red.

// Consumed by the charts and list coloring.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

/// CSS color for a list name / chart segment.
///
/// `root_children` is the scan root's children in a stable (name-sorted)
/// order; the ancestor of `path` directly below the root determines the hue.
pub fn color_for(path: &Path, root: &Path, root_children: &[std::path::PathBuf]) -> String {
    if path == root {
        return String::new(); // location row: neutral
    }
    let Ok(rel) = path.strip_prefix(root) else { return String::new() };
    let Some(top) = rel.components().next() else { return String::new() };
    let top_path = root.join(top);
    let Some(idx) = root_children.iter().position(|p| *p == top_path) else {
        return String::new();
    };
    let depth = rel.components().count() as f64;
    color_for_indexed(idx, depth)
}

/// O(1) hue lookup for chart projections: the same stable index space as
/// `color_for`, but the root child → index mapping is built once instead of
/// searched linearly per projected node.
pub struct ColorIndex {
    map: HashMap<std::path::PathBuf, usize>,
}

impl ColorIndex {
    pub fn new(root_children: &[std::path::PathBuf]) -> Self {
        ColorIndex {
            map: root_children
                .iter()
                .enumerate()
                .map(|(i, p)| (p.clone(), i))
                .collect(),
        }
    }

    /// The palette index of `path`'s top-level ancestor, or None when `path`
    /// is the scan root itself (neutral) or escapes it.
    pub fn top_index_of(&self, path: &Path, root: &Path) -> Option<usize> {
        let rel = path.strip_prefix(root).ok()?;
        let top = rel.components().next()?;
        self.map.get(&root.join(top)).copied()
    }

    /// Number of components of `path` below `root` (its color depth).
    pub fn rel_depth(path: &Path, root: &Path) -> usize {
        path.strip_prefix(root)
            .map(|rel| rel.components().count())
            .unwrap_or(0)
    }
}

/// Hue for the root child at stable index `idx`, `depth` components below
/// the scan root (shared by `color_for` and the chart projection).
pub fn color_for_indexed(idx: usize, depth: f64) -> String {
    // Golden-angle hue distribution keeps neighbors distinguishable.
    let hue = (idx as f64 * 137.508) % 360.0;
    // Descendants stay in the family: lighten/darken with depth.
    let light = 42.0 + ((depth - 1.0) * 9.0).rem_euclid(26.0) - 8.0;
    let sat = 62.0 - ((depth - 1.0) * 4.0).rem_euclid(18.0);
    hsl_to_css(hue, sat.clamp(30.0, 75.0), light.clamp(25.0, 65.0))
}

pub const ERROR_COLOR: &str = "#e01b24";
pub const DEGENERATE_COLOR: &str = "#9a9996"; // single flat tile / empty ring

pub fn hsl_to_css(h: f64, s: f64, l: f64) -> String {
    format!("hsl({h:.0}, {s:.0}%, {l:.0}%)")
}

/// Parse a CSS color to (r, g, b) floats via gdk (charts need components for
/// Cairo drawing; gdk::RGBA::parse handles hsl()).
pub fn parse_rgba(css: &str) -> Option<(f32, f32, f32, f32)> {
    let rgba = gtk::gdk::RGBA::parse(css).ok()?;
    Some((rgba.red(), rgba.green(), rgba.blue(), rgba.alpha()))
}
