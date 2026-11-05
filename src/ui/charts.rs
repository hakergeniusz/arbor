//! Rings and treemap chart widgets (docs/04 §7–§8).
//!
//! Both are custom `DrawingArea` subclasses drawing with Cairo from a shared
//! `ChartState`. Interaction per the specification and decisions D6/D17:
//!
//! - rings: display + hover only (clicks are no-ops); the widget exposes an
//!   accessible summary of the top-level children
//! - treemap: single click selects, double-click on a folder rectangle
//!   navigates, arrow keys walk rectangles, and the focused rectangle's
//!   name+size are exposed to assistive technology
//!
//! Performance notes (the charts sit next to a list that can hold a
//! multi-million-entry scan):
//!
//! - each `ChartNode` carries its children's angular segments, precomputed
//!   once at projection time — draws and hover hit-tests never re-run the
//!   subdivision, they binary-search the cached segments
//! - ring segments whose arc is sub-pixel (span < [`MIN_SPAN`]) are skipped
//!   together with their whole subtree — visually identical, and it prunes
//!   the long tail of tiny entries
//! - the rings only repaint when the hovered *segment* changes, not on every
//!   motion event; the treemap does not repaint on motion at all (it has no
//!   hover effect)
//! - treemap rectangles hold flat per-rect data, not a cloned `ChartNode`
//!   subtree, and the layout is cached against (root, size) so highlight-only
//!   redraws don't re-run the squarified layout

// Reserved accessors for the context-menu phase.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::cairo;
use gtk::glib::Propagation;
use gtk::accessible::Property as AccessibleProperty;
use gtk::DrawingArea;

/// A node of the chart data tree (a snapshot of the relevant `DisplayNode`
/// subtree with colors resolved).
#[derive(Debug, Clone)]
pub struct ChartNode {
    pub name: String,
    pub size: u64,
    pub color: String,
    pub is_dir: bool,
    pub path: String,
    pub children: Vec<ChartNode>,
    /// Children's angular segments in the node's LOCAL frame (0° = the
    /// parent's start edge, clockwise), precomputed at projection time.
    /// Aligned with `children` (empty when `children` is empty).
    pub segs: Vec<crate::squarify::Segment>,
}

/// Shared chart inputs, rebuilt by the window on any relevant change.
pub struct ChartState {
    /// Subtree of the folder the rings show (the current folder).
    pub rings_root: Rc<ChartNode>,
    /// Subtree the treemap shows (selected item, else current folder).
    pub treemap_root: Rc<ChartNode>,
    pub selected_path: Option<String>,
}

/// Deepest projected tree depth. The rings draw bands 0..[`RING_DEPTH`]
/// (children at tree depth ≤ 4) and the treemap lays out rectangles down to
/// [`TREEMAP_DEPTH`]; one extra level is projected so the depth-`5`
/// rectangles still know their children count (accessibility summary).
pub const CHART_MAX_DEPTH: usize = TREEMAP_DEPTH + 1;

/// One laid-out treemap rectangle: flat per-rect data (no subtree clone).
#[derive(Debug, Clone)]
pub(crate) struct TreeRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    depth: usize,
    name: String,
    path: String,
    color: String,
    #[allow(dead_code)]
    size: u64,
    is_dir: bool,
    child_count: usize,
}

const RING_DEPTH: usize = 4;
const TREEMAP_DEPTH: usize = 5;
const MIN_RECT: f64 = 2.0;
/// Ring segments narrower than this many degrees are not drawn (nor
/// recursed into): at the outermost ring of a 720 px chart their arc is
/// ≈0.1 px — invisible. This bounds the drawn-segment count regardless of
/// how many entries the folder holds.
const MIN_SPAN: f64 = 0.02;

// ---------------------------------------------------------------- rings ---

mod ring_imp {
    use super::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct RingsChart {
        pub state: RefCell<Option<Rc<ChartState>>>,
        pub hover: Cell<Option<(f64, f64)>>,
        /// Index chain of the hovered segment (one index per ring level,
        /// `depth + 1` entries) — the chart repaints only when this changes,
        /// not on every motion event.
        pub hover_chain: RefCell<Option<Vec<usize>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RingsChart {
        const NAME: &'static str = "ArborRingsChart";
        type Type = super::RingsChart;
        type ParentType = DrawingArea;

        fn class_init(klass: &mut Self::Class) {
            // Expose the chart to assistive technology as an image with a
            // text summary (docs/08 §4: chart data must be SR-reachable).
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for RingsChart {
        fn constructed(&self) {
            self.parent_constructed();
            let area = self.obj();
            area.set_hexpand(true);
            area.set_vexpand(true);

            let this = self.obj().downgrade();
            area.set_draw_func(move |_, cairo, width, height| {
                if let Some(chart) = this.upgrade() {
                    let imp = chart.imp();
                    imp.draw(&cairo, width, height);
                }
            });

            let ctrl = gtk::EventControllerMotion::new();
            {
                let obj = area.downgrade();
                ctrl.connect_motion(move |_, x, y| {
                    if let Some(o) = obj.upgrade() {
                        let imp = o.imp();
                        imp.hover.set(Some((x, y)));
                        let chain = imp.hovered_chain(o.width(), o.height(), x, y);
                        if *imp.hover_chain.borrow() != chain {
                            *imp.hover_chain.borrow_mut() = chain;
                            o.queue_draw();
                        }
                    }
                });
            }
            {
                let obj = area.downgrade();
                ctrl.connect_leave(move |_| {
                    if let Some(o) = obj.upgrade() {
                        let imp = o.imp();
                        imp.hover.set(None);
                        if imp.hover_chain.borrow_mut().take().is_some() {
                            o.queue_draw();
                        }
                    }
                });
            }
            area.add_controller(ctrl);
        }
    }

    impl WidgetImpl for RingsChart {}
    impl DrawingAreaImpl for RingsChart {}

    impl RingsChart {
        fn draw(&self, cairo: &cairo::Context, width: i32, height: i32) {
            let Some(state) = self.state.borrow().clone() else { return };
            let (w, h) = (f64::from(width), f64::from(height));
            let (cx, cy, center_r, ring_width) = ring_geometry(w, h);

            let mut c = cairo.clone();

            draw_ring_node(
                &mut c,
                &state.rings_root,
                state.selected_path.as_deref(),
                cx,
                cy,
                center_r,
                ring_width,
                0,
                0.0,
                360.0,
            );

            // Center circle with the location total (docs/04 §7).
            c.set_source_rgb(1.0, 1.0, 1.0);
            c.arc(cx, cy, center_r - 2.0, 0.0, 2.0 * std::f64::consts::PI);
            c.fill().unwrap();
            c.set_source_rgb(0.2, 0.2, 0.2);
            c.select_font_face("Inter", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            c.set_font_size(14.0);
            let total = crate::format::size(state.rings_root.size);
            let extents = c.text_extents(&total).unwrap();
            c.move_to(cx - extents.width() / 2.0, cy + extents.height() / 2.0);
            c.show_text(&total).unwrap();

            // Hover highlight (docs/02 §6: segments brighten). The hovered
            // segment is resolved from the cached per-node segments — cheap.
            if let Some(chain) = self.hover_chain.borrow().clone() {
                if let Some((node, seg, depth)) = resolve_chain(&state.rings_root, &chain) {
                    let (r, g, b) = css_to_rgb(&node.color);
                    c.set_source_rgba(r, g, b, 1.0);
                    fill_segment(&mut c, &seg, depth, cx, cy, center_r, ring_width, node_selected(state.selected_path.as_deref(), &node.path));
                    c.set_source_rgba(1.0, 1.0, 1.0, 0.45);
                    fill_segment(&mut c, &seg, depth, cx, cy, center_r, ring_width, node_selected(state.selected_path.as_deref(), &node.path));
                }
            }
        }

        /// The index chain of the segment under the point, from the cached
        /// per-node segments (binary search per level — no layout work).
        fn hovered_chain(&self, width: i32, height: i32, x: f64, y: f64) -> Option<Vec<usize>> {
            let (cx, cy, center_r, ring_width) = ring_geometry(f64::from(width), f64::from(height));
            let state = self.state.borrow().clone()?;
            let dx = x - cx;
            let dy = y - cy;
            let dist = dx.hypot(dy);
            if dist < center_r || dist > center_r + RING_DEPTH as f64 * ring_width {
                return None; // center hole or beyond the outer ring
            }
            let band = ((dist - center_r) / ring_width).floor();
            if !(0.0..RING_DEPTH as f64).contains(&band) {
                return None;
            }
            let mut angle = (-dy).atan2(dx).to_degrees() + 90.0;
            if angle >= 360.0 {
                angle -= 360.0;
            }
            let depth = band as usize;
            let mut current: &ChartNode = &state.rings_root;
            // Absolute frame (start, span) of the current level's local
            // 360° coordinate space.
            let mut frame = crate::squarify::Segment { start: 0.0, span: 360.0 };
            let mut chain = Vec::with_capacity(depth + 1);
            for level in 0..=depth {
                // Absolute angle → the current level's local frame.
                let local = (angle - frame.start) / frame.span * 360.0;
                let idx = seg_index_at(&current.segs, local)?;
                chain.push(idx);
                if level == depth {
                    return Some(chain);
                }
                let seg = current.segs[idx];
                frame = crate::squarify::Segment {
                    start: frame.start + seg.start / 360.0 * frame.span,
                    span: seg.span / 360.0 * frame.span,
                };
                current = current.children.get(idx)?;
            }
            None
        }
    }
}

glib::wrapper! {
    pub struct RingsChart(ObjectSubclass<ring_imp::RingsChart>)
        @extends DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for RingsChart {
    fn default() -> Self {
        Self::new()
    }
}

impl RingsChart {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_state(&self, state: Option<Rc<ChartState>>) {
        *self.imp().state.borrow_mut() = state;
        *self.imp().hover_chain.borrow_mut() = None;
        self.queue_draw();
        self.update_accessible_summary();
    }

    /// Screen-reader summary: the gap-closer of docs/08 §4 (rings segments
    /// are not individually exposed; the list remains the data surface).
    fn update_accessible_summary(&self) {
        let text = self
            .imp()
            .state
            .borrow()
            .as_ref()
            .map(|s| {
                let kids: Vec<String> = s
                    .rings_root
                    .children
                    .iter()
                    .take(8)
                    .map(|c| format!("{} {}", c.name, crate::format::size(c.size)))
                    .collect();
                if kids.is_empty() {
                    format!("Rings chart, {}", crate::format::size(s.rings_root.size))
                } else {
                    format!(
                        "Rings chart, total {}; top items: {}",
                        crate::format::size(s.rings_root.size),
                        kids.join(", ")
                    )
                }
            })
            .unwrap_or_else(|| "Rings chart".to_string());
        self.update_property(&[
            AccessibleProperty::Label("Rings chart"),
            AccessibleProperty::Description(&text),
        ]);
    }
}

// -------------------------------------------------------------- treemap ---

mod tree_imp {
    use super::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct TreemapChart {
        pub state: RefCell<Option<Rc<ChartState>>>,
        pub(crate) rects: RefCell<Vec<TreeRect>>,
        /// Layout cache key: (treemap root node, width, height). The
        /// rectangle layout does not depend on the selection, so
        /// highlight-only redraws reuse it.
        layout_key: RefCell<Option<(*const ChartNode, i32, i32)>>,
        pub focus_index: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TreemapChart {
        const NAME: &'static str = "ArborTreemapChart";
        type Type = super::TreemapChart;
        type ParentType = DrawingArea;

        fn class_init(klass: &mut Self::Class) {
            // Expose the chart to assistive technology as an image with a
            // text summary; keyboard focus walks the rectangles (docs/08 §4).
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for TreemapChart {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: std::sync::OnceLock<Vec<glib::subclass::Signal>> =
                std::sync::OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    // Single click on a rectangle selects the item.
                    glib::subclass::Signal::builder("rect-selected")
                        .param_types([String::static_type()])
                        .build(),
                    // Double click on a folder rectangle navigates into it.
                    glib::subclass::Signal::builder("rect-activated")
                        .param_types([String::static_type()])
                        .build(),
                    // Right click opens the context menu for the rectangle.
                    glib::subclass::Signal::builder("rect-context")
                        .param_types([String::static_type(), f64::static_type(), f64::static_type()])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let area = self.obj();
            area.set_hexpand(true);
            area.set_vexpand(true);
            area.set_focusable(true);

            let this = self.obj().downgrade();
            area.set_draw_func(move |_, cairo, width, height| {
                if let Some(chart) = this.upgrade() {
                    let imp = chart.imp();
                    imp.draw(&cairo, width, height);
                }
            });

            // No motion controller: the treemap has no hover effect, and a
            // repaint per motion event would re-run the whole draw for
            // nothing on large trees.

            let click = gtk::GestureClick::new();
            // GtkGestureSingle defaults to the primary button; the treemap
            // distinguishes select (1), activate (double) and context (3).
            click.set_button(0);
            {
                let obj = area.downgrade();
                click.connect_pressed(move |gesture, n_press, x, y| {
                    let Some(chart) = obj.upgrade() else { return };
                    let imp = chart.imp();
                    let button = gesture.current_button();
                    let Some(rect) = imp.rect_at(x, y) else { return };
                    let path = rect.path.clone();
                    match (button, n_press) {
                        (3, _) => {
                            chart.emit_by_name::<()>("rect-context", &[&path, &x, &y]);
                        }
                        (_, 2) => {
                            // Navigate into the deepest folder containing the
                            // point — a folder's hittable area includes its
                            // children's tiles (docs/04 §8).
                            if let Some(dir) = imp.deepest_dir_at(x, y) {
                                chart.emit_by_name::<()>("rect-activated", &[&dir.path]);
                            }
                            // Double click on a bare file rectangle: no effect
                            // (docs/04 §8).
                        }
                        _ => {
                            chart.emit_by_name::<()>("rect-selected", &[&path]);
                            imp.focus_index.set(imp.rects.borrow().iter().position(|r| r.path == path).unwrap_or(0));
                        }
                    }
                });
            }
            area.add_controller(click);

            let key = gtk::EventControllerKey::new();
            {
                let obj = area.downgrade();
                key.connect_key_pressed(move |_, key, _, _| {
                    let Some(chart) = obj.upgrade() else { return Propagation::Proceed };
                    let imp = chart.imp();
                    let count = imp.rects.borrow().len();
                    if count == 0 {
                        return Propagation::Proceed;
                    }
                    let idx = match key {
                        gtk::gdk::Key::Right | gtk::gdk::Key::Down => {
                            (imp.focus_index.get() + 1).min(count - 1)
                        }
                        gtk::gdk::Key::Left | gtk::gdk::Key::Up => {
                            imp.focus_index.get().saturating_sub(1)
                        }
                        gtk::gdk::Key::Return | gtk::gdk::Key::space => {
                            let rects = imp.rects.borrow();
                            if let Some(r) = rects.get(imp.focus_index.get()) {
                                if r.is_dir {
                                    chart.emit_by_name::<()>("rect-activated", &[&r.path.clone()]);
                                }
                            }
                            return Propagation::Stop;
                        }
                        _ => return Propagation::Proceed,
                    };
                    imp.focus_index.set(idx);
                    chart.update_accessible_rect();
                    chart.queue_draw();
                    Propagation::Stop
                });
            }
            area.add_controller(key);
        }
    }

    impl WidgetImpl for TreemapChart {}
    impl DrawingAreaImpl for TreemapChart {}

    impl TreemapChart {
        fn draw(&self, cairo: &cairo::Context, width: i32, height: i32) {
            let Some(state) = self.state.borrow().clone() else { return };
            let cairo = cairo.clone();

            // Degenerate state: one flat gray tile with the folder name
            // (docs/04 §9).
            if state.treemap_root.children.is_empty() {
                let (r, g, b) = css_to_rgb(crate::ui::palette::DEGENERATE_COLOR);
                cairo.set_source_rgb(r, g, b);
                cairo.rectangle(6.0, 6.0, width as f64 - 12.0, height as f64 - 12.0);
                cairo.fill().unwrap();
                cairo.set_source_rgb(0.15, 0.15, 0.15);
                cairo.set_font_size(13.0);
                let name = &state.treemap_root.name;
                let ext = cairo.text_extents(name).unwrap();
                cairo.move_to(
                    (width as f64 - ext.width()) / 2.0,
                    (height as f64 + ext.height()) / 2.0,
                );
                cairo.show_text(name).unwrap();
                self.rects.borrow_mut().clear();
                *self.layout_key.borrow_mut() = None;
                return;
            }

            // (Re)lay out only when the treemap root or the size changed;
            // everything else (selection, keyboard focus) reuses the rects.
            let key = (Rc::as_ptr(&state.treemap_root), width, height);
            let needs_layout = match self.layout_key.borrow().as_ref() {
                Some((ptr, w, h)) => *ptr != key.0 || *w != width || *h != height,
                None => true,
            };
            if needs_layout {
                let mut rects = Vec::new();
                layout_node(
                    &state.treemap_root,
                    6.0,
                    6.0,
                    width as f64 - 12.0,
                    height as f64 - 12.0,
                    0,
                    &mut rects,
                );
                *self.rects.borrow_mut() = rects;
                *self.layout_key.borrow_mut() = Some(key);
            }

            let selected = state.selected_path.as_deref();
            let focus = self.focus_index.get();
            for (i, r) in self.rects.borrow().iter().enumerate() {
                let (cr, cg, cb) = css_to_rgb(&r.color);
                cairo.set_source_rgb(cr, cg, cb);
                cairo.rectangle(r.x, r.y, r.w, r.h);
                cairo.fill().unwrap();
                // Parent/child separation + labels when they fit (docs/04 §8).
                cairo.set_source_rgb(1.0, 1.0, 1.0);
                cairo.set_line_width(1.5);
                cairo.rectangle(r.x, r.y, r.w, r.h);
                cairo.stroke().unwrap();
                if r.w > 60.0 && r.h > 24.0 {
                    cairo.set_source_rgb(0.12, 0.12, 0.12);
                    cairo.set_font_size(12.0);
                    let name = &r.name;
                    let ext = cairo.text_extents(name).unwrap();
                    if ext.width() < r.w - 12.0 {
                        cairo.move_to(r.x + (r.w - ext.width()) / 2.0, r.y + (r.h + ext.height()) / 2.0);
                        cairo.show_text(name).unwrap();
                    }
                }
                if Some(r.path.as_str()) == selected {
                    cairo.set_source_rgb(1.0, 1.0, 1.0);
                    cairo.set_line_width(3.0);
                    cairo.rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
                    cairo.stroke().unwrap();
                }
                if i == focus && self.obj().has_focus() {
                    cairo.set_source_rgb(1.0, 1.0, 1.0);
                    cairo.set_line_width(2.0);
                    cairo.set_dash(&[4.0, 3.0], 0.0);
                    cairo.rectangle(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0);
                    cairo.stroke().unwrap();
                    cairo.set_dash(&[], 0.0);
                }
            }
        }

        fn rect_at(&self, x: f64, y: f64) -> Option<TreeRect> {
            let rects = self.rects.borrow();
            // Deepest (last laid-out) rect containing the point wins.
            rects.iter().rev().find(|r| {
                x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
            }).cloned()
        }

        /// Deepest *folder* rectangle containing the point: a folder's
        /// clickable area spans its whole subtree region.
        fn deepest_dir_at(&self, x: f64, y: f64) -> Option<TreeRect> {
            let rects = self.rects.borrow();
            rects.iter().rev().find(|r| {
                r.is_dir
                    && x >= r.x
                    && x <= r.x + r.w
                    && y >= r.y
                    && y <= r.y + r.h
            }).cloned()
        }
    }
}

glib::wrapper! {
    pub struct TreemapChart(ObjectSubclass<tree_imp::TreemapChart>)
        @extends DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for TreemapChart {
    fn default() -> Self {
        Self::new()
    }
}

impl TreemapChart {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_state(&self, state: Option<Rc<ChartState>>) {
        *self.imp().state.borrow_mut() = state;
        self.imp().focus_index.set(0);
        self.queue_draw();
        self.update_accessible_rect();
    }

    pub fn connect_rect_selected<F: Fn(&Self, String) + 'static>(&self, f: F) {
        self.connect_local("rect-selected", true, move |args| {
            let obj = args[0].get::<Self>().unwrap();
            let path = args[1].get::<String>().unwrap();
            f(&obj, path);
            None
        });
    }

    pub fn connect_rect_activated<F: Fn(&Self, String) + 'static>(&self, f: F) {
        self.connect_local("rect-activated", true, move |args| {
            let obj = args[0].get::<Self>().unwrap();
            let path = args[1].get::<String>().unwrap();
            f(&obj, path);
            None
        });
    }

    pub fn connect_rect_context<F: Fn(&Self, String, f64, f64) + 'static>(&self, f: F) {
        self.connect_local("rect-context", true, move |args| {
            let obj = args[0].get::<Self>().unwrap();
            let path = args[1].get::<String>().unwrap();
            let x = args[2].get::<f64>().unwrap();
            let y = args[3].get::<f64>().unwrap();
            f(&obj, path, x, y);
            None
        });
    }

    /// Expose the focused rectangle's name+size to assistive technology
    /// (decision D17).
    fn update_accessible_rect(&self) {
        let imp = self.imp();
        let text = {
            let rects = imp.rects.borrow();
            rects
                .get(imp.focus_index.get())
                .map(|r| {
                    format!(
                        "{}, {}, {} item{}",
                        r.name,
                        crate::format::size(r.size),
                        r.child_count,
                        if r.child_count == 1 { "" } else { "s" }
                    )
                })
                // Before the first draw lays out the rectangles, summarize the
                // shown folder from the state itself.
                .or_else(|| {
                    imp.state.borrow().as_ref().map(|s| {
                        format!(
                            "Treemap of {}, total {}",
                            s.treemap_root.name,
                            crate::format::size(s.treemap_root.size)
                        )
                    })
                })
                .unwrap_or_else(|| "Treemap chart".to_string())
        };
        self.update_property(&[
            AccessibleProperty::Label("Treemap chart"),
            AccessibleProperty::Description(&text),
        ]);
    }
}

// -------------------------------------------------------------- layout ---

/// Recursively lay out `node`'s children; `rects` receives one entry per
/// drawn rectangle (parents first, children inset inside them). Only flat
/// per-rect data is stored — no `ChartNode` subtree is cloned.
fn layout_node(node: &ChartNode, x: f64, y: f64, w: f64, h: f64, depth: usize, rects: &mut Vec<TreeRect>) {
    if depth > TREEMAP_DEPTH || w < MIN_RECT || h < MIN_RECT {
        return;
    }
    rects.push(TreeRect {
        x,
        y,
        w,
        h,
        depth,
        name: node.name.clone(),
        path: node.path.clone(),
        color: node.color.clone(),
        size: node.size,
        is_dir: node.is_dir,
        child_count: node.children.len(),
    });
    if node.children.is_empty() || depth == TREEMAP_DEPTH {
        return;
    }
    let weights: Vec<f64> = node.children.iter().map(|c| c.size as f64).collect();
    const INSET: f64 = 3.0;
    let laid = crate::squarify::squarify(&weights, x + INSET, y + INSET, (w - 2.0 * INSET).max(0.0), (h - 2.0 * INSET).max(0.0));
    for r in laid {
        let child = &node.children[r.index];
        if r.w < MIN_RECT || r.h < MIN_RECT {
            continue;
        }
        layout_node(child, r.x, r.y, r.w, r.h, depth + 1, rects);
    }
}

// --------------------------------------------------------------- rings ---

/// Ring geometry shared by the draw path and the motion hit-test.
fn ring_geometry(width: f64, height: f64) -> (f64, f64, f64, f64) {
    let cx = width / 2.0;
    let cy = height / 2.0;
    let outer = width.min(height) / 2.0 - 8.0;
    let ring_width = outer / (RING_DEPTH as f64 + 1.0);
    let center_r = outer - RING_DEPTH as f64 * ring_width;
    (cx, cy, center_r, ring_width)
}

/// Recursively fill ring bands; band `depth` holds `node`'s children,
/// subdividing the parent's arc (rings never repaint the full circle).
/// Segment geometry comes from the per-node cache (`node.segs`); segments
/// below `MIN_SPAN` are skipped with their whole subtree — their arc is
/// sub-pixel, so the picture is unchanged while the long tail of tiny
/// entries costs nothing.
#[allow(clippy::too_many_arguments)]
fn draw_ring_node(
    cairo: &mut cairo::Context,
    node: &ChartNode,
    selected: Option<&str>,
    cx: f64,
    cy: f64,
    center_r: f64,
    ring_width: f64,
    depth: usize,
    start: f64,
    span: f64,
) {
    if depth >= RING_DEPTH || node.children.is_empty() || span <= 0.0 {
        return;
    }
    for (child, seg) in node.children.iter().zip(&node.segs) {
        // Map the child's local segment into the parent's arc.
        let child_seg = crate::squarify::Segment {
            start: start + seg.start / 360.0 * span,
            span: seg.span / 360.0 * span,
        };
        if child_seg.span < MIN_SPAN {
            continue;
        }
        let is_sel = node_selected(selected, &child.path);
        let (r, g, b) = css_to_rgb(&child.color);
        cairo.set_source_rgba(r, g, b, 1.0);
        fill_segment(cairo, &child_seg, depth, cx, cy, center_r, ring_width, is_sel);
        draw_ring_node(
            cairo,
            child,
            selected,
            cx,
            cy,
            center_r,
            ring_width,
            depth + 1,
            child_seg.start,
            child_seg.span,
        );
    }
}

fn fill_segment(
    cairo: &mut cairo::Context,
    seg: &crate::squarify::Segment,
    depth: usize,
    cx: f64,
    cy: f64,
    center_r: f64,
    ring_width: f64,
    selected: bool,
) {
    let r_in = center_r + depth as f64 * ring_width + if selected { 5.0 } else { 0.0 };
    let r_out = r_in + ring_width - 1.0;
    let a0 = (seg.start - 90.0).to_radians();
    let a1 = (seg.start + seg.span - 90.0).to_radians();
    cairo.new_path();
    cairo.arc(cx, cy, r_out, a0, a1);
    cairo.arc_negative(cx, cy, r_in, a1, a0);
    cairo.close_path();
    cairo.fill().unwrap();
    // Thin white separation between segments.
    cairo.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    cairo.set_line_width(1.0);
    cairo.new_path();
    cairo.arc(cx, cy, r_out, a0, a1);
    cairo.stroke().unwrap();
}

/// Index of the cached segment containing `angle` (degrees, local frame).
/// Starts are non-decreasing, so a binary search suffices.
fn seg_index_at(segs: &[crate::squarify::Segment], angle: f64) -> Option<usize> {
    let idx = segs.partition_point(|s| s.start <= angle).checked_sub(1)?;
    let s = &segs[idx];
    (angle >= s.start && angle < s.start + s.span).then_some(idx)
}

/// Resolve a hover index chain to the hovered node, its absolute segment
/// (12-o'clock origin) and its band depth, via the cached per-node segments.
/// Each level's local 360° frame is mapped onto the parent's arc, exactly
/// like `draw_ring_node` maps segments while painting.
fn resolve_chain<'a>(
    root: &'a ChartNode,
    chain: &[usize],
) -> Option<(&'a ChartNode, crate::squarify::Segment, usize)> {
    let mut current = root;
    // Absolute frame (start, span) of `current`'s local coordinate space.
    let mut frame = crate::squarify::Segment { start: 0.0, span: 360.0 };
    for (level, &idx) in chain.iter().enumerate() {
        let seg = *current.segs.get(idx)?;
        let child = crate::squarify::Segment {
            start: frame.start + seg.start / 360.0 * frame.span,
            span: seg.span / 360.0 * frame.span,
        };
        let node = current.children.get(idx)?;
        if level == chain.len() - 1 {
            return Some((node, child, level));
        }
        frame = child;
        current = node;
    }
    None
}

fn node_selected(selected: Option<&str>, path: &str) -> bool {
    selected == Some(path)
}

pub fn css_to_rgb(css: &str) -> (f64, f64, f64) {
    match gtk::gdk::RGBA::parse(css) {
        Ok(rgba) => (rgba.red() as f64, rgba.green() as f64, rgba.blue() as f64),
        Err(_) => (0.6, 0.6, 0.6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(name: &str, size: u64) -> ChartNode {
        ChartNode {
            name: name.to_string(),
            size,
            color: "red".into(),
            is_dir: false,
            path: format!("/{name}"),
            children: Vec::new(),
            segs: Vec::new(),
        }
    }

    /// A node with cached segments from its children's sizes.
    fn with_segs(mut node: ChartNode) -> ChartNode {
        let weights: Vec<f64> = node.children.iter().map(|c| c.size as f64).collect();
        node.segs = crate::squarify::segments(&weights);
        node
    }

    #[test]
    fn seg_index_at_finds_containing_segment() {
        let segs = crate::squarify::segments(&[1.0, 1.0, 2.0]);
        assert_eq!(seg_index_at(&segs, 0.0), Some(0));
        assert_eq!(seg_index_at(&segs, 89.999), Some(0));
        assert_eq!(seg_index_at(&segs, 90.0), Some(1));
        assert_eq!(seg_index_at(&segs, 200.0), Some(2));
        // Boundary semantics match the old linear scan: end-exclusive.
        assert_eq!(seg_index_at(&segs, 360.0), None);
        assert_eq!(seg_index_at(&segs, -0.5), None);
    }

    #[test]
    fn resolve_chain_hits_deep_segments_with_absolute_angles() {
        // root: A (25%) + B (75%); B: B1 (50%) + B2 (50%)
        let root = with_segs(ChartNode {
            name: "root".into(),
            size: 400,
            color: "red".into(),
            is_dir: true,
            path: "/root".into(),
            children: vec![
                with_segs(ChartNode {
                    name: "A".into(),
                    size: 100,
                    color: "green".into(),
                    is_dir: true,
                    path: "/root/A".into(),
                    children: vec![leaf("a1", 100)],
                    segs: Vec::new(),
                }),
                with_segs(ChartNode {
                    name: "B".into(),
                    size: 300,
                    color: "blue".into(),
                    is_dir: true,
                    path: "/root/B".into(),
                    children: vec![
                        leaf("B1", 150),
                        leaf("B2", 150),
                    ],
                    segs: Vec::new(),
                }),
            ],
            segs: Vec::new(),
        });
        // A spans [0, 90); B [90, 360); inside B, B1 [90, 225), B2 [225, 360).
        let (_, seg, depth) = resolve_chain(&root, &[1, 0]).unwrap();
        assert_eq!(depth, 1);
        assert!((seg.start - 90.0).abs() < 1e-9 && (seg.span - 135.0).abs() < 1e-9);
        let (node, _, _) = resolve_chain(&root, &[1, 1]).unwrap();
        assert_eq!(node.name, "B2");
        // Level-0 hit resolves in the root frame.
        let (node, seg, _) = resolve_chain(&root, &[0]).unwrap();
        assert_eq!(node.name, "A");
        assert!((seg.start - 0.0).abs() < 1e-9 && (seg.span - 90.0).abs() < 1e-9);
        // Malformed chains resolve to None, not panic.
        assert!(resolve_chain(&root, &[]).is_none());
        assert!(resolve_chain(&root, &[5]).is_none());
        assert!(resolve_chain(&root, &[1, 0, 0]).is_none());
    }
}
