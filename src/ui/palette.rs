//! Original chart palette (docs/10 §5): distinct hues for the scan root's
//! top-level children, descendants inherit the parent hue with lightness
//! variation; permission-error rows render red.

// Consumed by the charts and list coloring.
#![allow(dead_code)]

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
