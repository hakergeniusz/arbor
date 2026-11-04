//! Squarified treemap layout (Bruls, Huizing & van Wijk's algorithm —
//! published, standard technique) plus ring-geometry helpers.
//!
//! Pure functions, no GTK types: unit-testable without a display.

// Some helpers are consumed by the chart widgets only.
#![allow(dead_code)]

/// A laid-out rectangle in the coordinate space given by the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaidRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Index into the input weights slice.
    pub index: usize,
}

/// Squarified layout of `weights` into the rectangle `(x, y, w, h)`.
/// Weights must be non-negative and sum > 0; zero-weight items receive
/// sliver rectangles at the end.
pub fn squarify(weights: &[f64], x: f64, y: f64, w: f64, h: f64) -> Vec<LaidRect> {
    let total: f64 = weights.iter().sum();
    let area = w * h;
    if weights.is_empty() || total <= 0.0 || area <= 0.0 {
        return Vec::new();
    }

    // Order by descending weight for the classic algorithm; remember indices.
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|a, b| weights[*b].partial_cmp(&weights[*a]).unwrap());

    let scale = area / total;
    let mut out = vec![
        LaidRect { x: 0.0, y: 0.0, w: 0.0, h: 0.0, index: 0 };
        weights.len()
    ];
    let mut remaining = (x, y, w, h);
    let mut i = 0;
    while i < order.len() {
        // The row strip runs along the SHORTER edge of the remaining
        // rectangle (its length is min(w, h)).
        let long = remaining.2.min(remaining.3);
        let mut row: Vec<usize> = vec![order[i]];
        let mut row_areas: Vec<f64> = vec![weights[order[i]] * scale];
        i += 1;
        while i < order.len() {
            let candidate = weights[order[i]] * scale;
            // Adding the candidate improves (or keeps) the worst aspect ratio
            // in the row → grow the row; otherwise lay the row out.
            let mut with_candidate = row_areas.clone();
            with_candidate.push(candidate);
            if worst_ratio(&row_areas, long) >= worst_ratio(&with_candidate, long) {
                row.push(order[i]);
                row_areas.push(candidate);
                i += 1;
            } else {
                break;
            }
        }
        let row_area: f64 = row_areas.iter().sum();

        // Strip thickness measured along the longer edge.
        let strip = (row_area / long).min(remaining.2.max(remaining.3));
        if remaining.3 < remaining.2 {
            // Height < width: the shorter edge is vertical → vertical strip
            // on the left, items stacked along it.
            let mut cy = remaining.1;
            for idx in &row {
                let ch = if strip > 0.0 {
                    (weights[*idx] * scale / strip).min(remaining.1 + remaining.3 - cy)
                } else {
                    0.0
                };
                out[*idx] = LaidRect {
                    x: remaining.0,
                    y: cy,
                    w: strip,
                    h: ch,
                    index: *idx,
                };
                cy += ch;
            }
            remaining = (
                remaining.0 + strip,
                remaining.1,
                (remaining.2 - strip).max(0.0),
                remaining.3,
            );
        } else {
            // Width ≤ height: horizontal strip on the top, items side by
            // side along it.
            let mut cx = remaining.0;
            for idx in &row {
                let cw = if strip > 0.0 {
                    (weights[*idx] * scale / strip).min(remaining.0 + remaining.2 - cx)
                } else {
                    0.0
                };
                out[*idx] = LaidRect {
                    x: cx,
                    y: remaining.1,
                    w: cw,
                    h: strip,
                    index: *idx,
                };
                cx += cw;
            }
            remaining = (
                remaining.0,
                remaining.1 + strip,
                remaining.2,
                (remaining.3 - strip).max(0.0),
            );
        }
    }
    out
}

/// Worst aspect ratio of the items in a row strip laid along an edge of
/// length `long` (item thickness = area / long, ratio = max(long²/a, a/long²)).
fn worst_ratio(areas: &[f64], long: f64) -> f64 {
    if long <= 0.0 || areas.is_empty() {
        return f64::INFINITY;
    }
    let long2 = long * long;
    areas
        .iter()
        .filter(|a| **a > 0.0)
        .map(|a| (long2 / a).max(a / long2))
        .fold(0.0f64, f64::max)
}

/// An angular segment of a ring.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// Start angle in degrees (0 = 12 o'clock, clockwise).
    pub start: f64,
    pub span: f64,
}

/// Split `[0, total)` proportionally into segments.
pub fn segments(weights: &[f64]) -> Vec<Segment> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut angle = 0.0;
    weights
        .iter()
        .map(|w| {
            let span = w / total * 360.0;
            let seg = Segment { start: angle, span };
            angle += span;
            seg
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_are_proportional_and_cover() {
        let weights = [50.0, 30.0, 20.0];
        let rects = squarify(&weights, 0.0, 0.0, 100.0, 100.0);
        assert_eq!(rects.len(), 3);
        let total_area: f64 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((total_area - 10_000.0).abs() < 1e-6, "rects must cover the area");
        let ratio = rects[0].w * rects[0].h / (rects[1].w * rects[1].h);
        assert!((ratio - 50.0 / 30.0).abs() < 1e-6, "areas must be proportional");
    }

    #[test]
    fn aspect_ratios_stay_reasonable_for_typical_input() {
        let weights: Vec<f64> = (1..=12).map(|i| (i * 37 % 100 + 1) as f64).collect();
        let rects = squarify(&weights, 0.0, 0.0, 400.0, 300.0);
        for r in &rects {
            if r.w > 1.0 && r.h > 1.0 {
                let ratio = r.w.max(r.h) / r.w.min(r.h);
                assert!(ratio < 6.0, "aspect ratio {} too extreme", ratio);
            }
        }
    }

    #[test]
    fn degenerate_inputs() {
        assert!(squarify(&[], 0.0, 0.0, 10.0, 10.0).is_empty());
        assert!(squarify(&[0.0, 0.0], 0.0, 0.0, 10.0, 10.0).is_empty());
        assert!(squarify(&[1.0], 0.0, 0.0, 0.0, 10.0).is_empty());
    }

    #[test]
    fn segments_sum_to_full_circle() {
        let segs = segments(&[1.0, 2.0, 1.0]);
        let last = segs.last().unwrap();
        assert!((last.start + last.span - 360.0).abs() < 1e-9);
        assert!((segs[1].span - 180.0).abs() < 1e-9);
    }
}
