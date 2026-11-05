//! Result view: two-pane layout with the item list on the left and the chart
//! pane on the right (docs/04 §1), plus the always-on infobar (docs/04 §6).

// The context-menu wiring lands with the item-actions phase.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Align, Orientation};

use super::charts::{RingsChart, TreemapChart};
use super::scan_row::ScanRow;
use super::ChartMode;

/// Right-click callback: (row, model position, source widget, x, y).
pub type RowContextCb = Rc<dyn Fn(&ScanRow, u32, &gtk::Widget, f64, f64)>;

pub struct ResultView {
    pub root: gtk::Box,
    pub list_scroll: gtk::ScrolledWindow,
    pub column_view: gtk::ColumnView,
    pub store: gio::ListStore,
    pub selection: gtk::SingleSelection,
    chart_area: gtk::Box,
    pub chart_stack: gtk::Stack,
    pub spinner: gtk::Spinner,
    pub toggle_rings: gtk::ToggleButton,
    pub toggle_treemap: gtk::ToggleButton,
    pub rings_chart: RingsChart,
    pub treemap_chart: TreemapChart,
    /// The chart pane can be disabled in Preferences ("Show chart"); when
    /// off, the list takes the full width and no chart work happens.
    charts_enabled: Cell<bool>,
    row_context_cb: Rc<RefCell<Option<RowContextCb>>>,
}

impl ResultView {
    pub fn new() -> Self {
        let root = gtk::Box::new(Orientation::Vertical, 0);

        // Full-width gray banner, not dismissible (docs/04 §6).
        let infobar = gtk::Box::new(Orientation::Horizontal, 0);
        infobar.add_css_class("infobar");
        let info_label = gtk::Label::new(None);
        info_label.set_use_markup(true);
        info_label.set_justify(gtk::Justification::Center);
        info_label.set_hexpand(true);
        info_label.set_halign(Align::Center);
        info_label.set_margin_top(6);
        info_label.set_margin_bottom(6);
        info_label.set_markup("<b>Files may take more space than shown</b>");
        infobar.append(&info_label);
        root.append(&infobar);

        let paned = gtk::Paned::new(Orientation::Horizontal);
        paned.set_position(480); // ≈50/50 at the 960×600 default; fluid on resize
        paned.set_vexpand(true);
        root.append(&paned);

        // Left pane: item list (docs/04 §2).
        let list_area = gtk::Box::new(Orientation::Vertical, 0);
        let store = gio::ListStore::new::<ScanRow>();
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        let column_view = gtk::ColumnView::new(Some(selection.clone()));
        column_view.set_single_click_activate(true);
        column_view.set_hexpand(true);
        column_view.set_vexpand(true);

        let row_context_cb: Rc<RefCell<Option<RowContextCb>>> = Rc::new(RefCell::new(None));

        let folder_col =
            gtk::ColumnViewColumn::new(Some("Folder"), Some(folder_factory(row_context_cb.clone())));
        folder_col.set_resizable(true);
        folder_col.set_expand(true);
        folder_col.set_sorter(Some(&row_sorter(CellKind::Name)));
        column_view.append_column(&folder_col);
        let size_col = text_column("Size", CellKind::Size, &row_context_cb);
        size_col.set_fixed_width(88);
        size_col.set_sorter(Some(&row_sorter(CellKind::Size)));
        column_view.append_column(&size_col);
        let items_col = text_column("Contents", CellKind::Items, &row_context_cb);
        items_col.set_fixed_width(92);
        items_col.set_sorter(Some(&row_sorter(CellKind::Items)));
        column_view.append_column(&items_col);
        let modified_col = text_column("Modified", CellKind::Modified, &row_context_cb);
        modified_col.set_fixed_width(82);
        modified_col.set_sorter(Some(&row_sorter(CellKind::Modified)));
        column_view.append_column(&modified_col);

        // Initial header state: Size ▼ (the list itself is pre-sorted this
        // way by the window; docs/04 §3).
        column_view.sort_by_column(Some(&size_col), gtk::SortType::Descending);

        let list_scroll = gtk::ScrolledWindow::new();
        list_scroll.set_vexpand(true);
        list_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        list_scroll.set_child(Some(&column_view));
        list_area.append(&list_scroll);
        paned.set_start_child(Some(&list_area));

        // Right pane: chart stack + mode toggles.
        let chart_area = gtk::Box::new(Orientation::Vertical, 0);
        let chart_stack = gtk::Stack::new();
        chart_stack.set_vexpand(true);
        chart_stack.set_transition_type(gtk::StackTransitionType::None);
        chart_area.append(&chart_stack);

        let spinner = gtk::Spinner::new();
        spinner.set_halign(Align::Center);
        spinner.set_valign(Align::Center);
        spinner.set_size_request(48, 48);
        chart_stack.add_named(&spinner, Some("spinner"));

        let rings_chart = RingsChart::new();
        chart_stack.add_named(&rings_chart, Some("rings"));
        let treemap_chart = TreemapChart::new();
        chart_stack.add_named(&treemap_chart, Some("treemap"));

        let toggle_bar = gtk::Box::new(Orientation::Horizontal, 0);
        toggle_bar.set_halign(Align::End);
        toggle_bar.set_valign(Align::End);
        toggle_bar.set_margin_bottom(12);
        toggle_bar.set_margin_end(12);
        toggle_bar.add_css_class("linked");
        let toggle_rings = gtk::ToggleButton::with_label("Rings Chart");
        let toggle_treemap = gtk::ToggleButton::with_label("Treemap Chart");
        toggle_treemap.set_group(Some(&toggle_rings));
        toggle_rings.set_active(true);
        toggle_bar.append(&toggle_rings);
        toggle_bar.append(&toggle_treemap);
        chart_area.append(&toggle_bar);
        paned.set_end_child(Some(&chart_area));

        Self {
            root,
            list_scroll,
            column_view,
            store,
            selection,
            chart_area,
            chart_stack,
            spinner,
            toggle_rings,
            toggle_treemap,
            rings_chart,
            treemap_chart,
            charts_enabled: Cell::new(true),
            row_context_cb,
        }
    }

    /// Replace the flat row list (location row first, then children).
    pub fn set_rows(&self, rows: &[ScanRow]) {
        self.store.remove_all();
        for row in rows {
            self.store.append(row);
        }
        self.selection.set_selected(u32::MAX); // GTK_NO_SELECTION
    }

    /// Show the spinner page (scan in progress) or a chart page.
    pub fn show_page(&self, name: &str) {
        self.chart_stack.set_visible_child_name(name);
        if name == "spinner" {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
    }

    /// True while a chart page (not the scanning spinner) is frontmost AND
    /// the pane is enabled — the chart projection is skipped otherwise.
    pub fn charts_visible(&self) -> bool {
        self.charts_enabled.get()
            && self.chart_stack.visible_child_name().as_deref() != Some("spinner")
    }

    /// Show/hide the chart pane (Preferences "Show chart"). Hidden, the item
    /// list takes the whole width; the toggles hide with the pane.
    pub fn set_chart_enabled(&self, on: bool) {
        self.charts_enabled.set(on);
        self.chart_area.set_visible(on);
    }

    pub fn charts_enabled(&self) -> bool {
        self.charts_enabled.get()
    }

    /// Reflect `mode` in the toggle buttons. Signal handlers live in the
    /// window and are idempotent w.r.t. state, so plain set_active is safe.
    pub fn set_mode_buttons(&self, mode: ChartMode) {
        match mode {
            ChartMode::Rings => self.toggle_rings.set_active(true),
            ChartMode::Treemap => self.toggle_treemap.set_active(true),
        }
    }

    /// Selected row object, if any.
    pub fn selected_row(&self) -> Option<ScanRow> {
        self.selection
            .selected_item()
            .and_downcast::<ScanRow>()
    }

    /// Select the row whose path is `path` (view positions differ from store
    /// order once header sorting is in play). Returns true on success.
    pub fn select_row_by_path(&self, path: &std::path::Path) -> bool {
        let want = path.to_string_lossy().to_string();
        for i in 0..self.selection.n_items() {
            let Some(row) = self.selection.item(i).and_downcast::<ScanRow>() else {
                continue;
            };
            if row.path().as_str() == want {
                self.selection.set_selected(i);
                return true;
            }
        }
        false
    }

    /// Set the right-click handler for list rows (docs/05 §1). The handler
    /// receives the row, its model position and the click position in row
    /// widget coordinates.
    pub fn connect_row_context(&self, cb: impl Fn(&ScanRow, u32, &gtk::Widget, f64, f64) + 'static) {
        *self.row_context_cb.borrow_mut() = Some(Rc::new(cb));
    }
}

impl Default for ResultView {
    fn default() -> Self {
        Self::new()
    }
}

/// The "Folder" column: expander + percent badge + color-coded name.
fn folder_factory(row_context_cb: Rc<RefCell<Option<RowContextCb>>>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let row = gtk::Box::new(Orientation::Horizontal, 6);
        row.set_margin_start(6);
        row.set_margin_end(6);

        let expander = gtk::Button::new();
        expander.add_css_class("flat");
        let arrow = gtk::Image::from_icon_name("pan-end-symbolic");
        arrow.set_pixel_size(12);
        expander.set_child(Some(&arrow));
        expander.set_valign(Align::Center);
        // ▶/▼ toggles inline expansion; the pinned location row's ^ navigates
        // to the parent folder (docs/02 §3). The row object is read at click
        // time — the button widget is recycled across items.
        {
            let item = list_item.clone();
            let expander_btn = expander.clone();
            expander.connect_clicked(move |_| {
                let Some(row) = item.item().and_downcast::<ScanRow>() else { return };
                if row.is_location() {
                    expander_btn.activate_action("win.navigate-up", None);
                } else {
                    let param = row.path().to_variant();
                    expander_btn.activate_action("win.toggle-expand", Some(&param));
                }
            });
        }

        // Percent badge: one dark pill whose fill represents the share with
        // the percentage centered on top (docs/10 §3 row anatomy).
        let badge = gtk::Overlay::new();
        badge.add_css_class("percent-badge");
        badge.set_width_request(64);
        badge.set_valign(Align::Center);
        badge.set_halign(Align::Start);
        let badge_bar = gtk::ProgressBar::new();
        // No hexpand here: expand computes through the Overlay to the row
        // Box, which would starve the name label of its column width.
        // The bar's raw fraction would otherwise be concatenated into the
        // row's accessible name ("0.985563 98.56% …"); the visible percentage
        // label already carries the value (docs/10 §5 row format).
        badge_bar.set_accessible_role(gtk::AccessibleRole::None);
        badge.set_child(Some(&badge_bar));
        let badge_label = gtk::Label::new(None);
        badge_label.add_css_class("mini");
        badge_label.set_halign(Align::Center);
        badge_label.set_valign(Align::Center);
        badge.add_overlay(&badge_label);

        let name = gtk::Label::new(None);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_hexpand(true);
        name.set_halign(Align::Start);

        row.append(&expander);
        row.append(&badge);
        row.append(&name);
        list_item.set_child(Some(&row));
        attach_row_context_gesture(row.upcast_ref::<gtk::Widget>().clone(), list_item.clone(), &row_context_cb);
    });
    factory.connect_bind(move |_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(row_obj) = list_item.item().and_downcast::<ScanRow>() else { return };
        let Some(container) = list_item.child().and_downcast::<gtk::Box>() else { return };
        // The whole row as one readable string for assistive technology
        // (docs/08 §1: "the complete row content is one accessible string").
        container.update_property(&[gtk::accessible::Property::Description(
            &row_obj.accessible_text(),
        )]);
        // Children were appended in this order in setup.
        let Some(expander) = container.first_child().and_downcast::<gtk::Button>() else { return };
        let Some(badge) = expander.next_sibling().and_downcast::<gtk::Overlay>() else { return };
        let Some(badge_bar) = badge.child().and_downcast::<gtk::ProgressBar>() else { return };
        // The overlay's first overlay child follows the main child in the
        // widget tree (setup appended exactly one).
        let Some(badge_label) = badge_bar.next_sibling().and_downcast::<gtk::Label>() else { return };
        let Some(name) = badge.next_sibling().and_downcast::<gtk::Label>() else { return };
        let Some(arrow) = expander.child().and_downcast::<gtk::Image>() else { return };

        name.set_text(&row_obj.name());
        // Plain name color; red only for permission-error rows, amber while
        // the folder's subtree is still being measured (docs/10 §5).
        if row_obj.error() {
            if let Ok(rgba) = gtk::gdk::RGBA::parse(ERROR_COLOR) {
                let attrs = gtk::pango::AttrList::new();
                attrs.insert(gtk::pango::AttrColor::new_foreground(
                    (rgba.red() * 65535.0) as u16,
                    (rgba.green() * 65535.0) as u16,
                    (rgba.blue() * 65535.0) as u16,
                ));
                name.set_attributes(Some(&attrs));
            }
        } else {
            name.set_attributes(None);
        }
        let unmeasured = !row_obj.measured();
        set_class_flag(&name, "unmeasured", unmeasured && !row_obj.error());
        set_class_flag(&badge, "unmeasured", unmeasured);

        let depth = row_obj.depth();
        expander.set_margin_start(12 * depth as i32);

        // Expander: up-arrow on the pinned location row (docs/02 §3), ▶/▼ for
        // folders, hidden for files.
        if row_obj.is_location() {
            arrow.set_icon_name(Some("pan-up-symbolic"));
            expander.set_visible(true);
        } else if row_obj.is_dir() {
            arrow.set_icon_name(Some(if row_obj.expanded() {
                "pan-down-symbolic"
            } else {
                "pan-end-symbolic"
            }));
            expander.set_visible(true);
        } else {
            expander.set_visible(false);
        }

        badge.set_visible(!row_obj.is_location());
        let pct = row_obj.percent();
        badge_bar.set_fraction(pct.clamp(0.0, 1.0));
        badge_label.set_text(&format!("{:.2}%", pct * 100.0));
    });
    factory
}

const ERROR_COLOR: &str = "#e01b24";

/// Add/remove a CSS class by flag (GTK 4.10 has no toggle_css_class).
fn set_class_flag(widget: &impl gtk::prelude::IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// Right-click on a row: report it to the window's context-menu handler
/// (docs/05 §1). The gesture is attached to the row cell; the owning
/// ListItem supplies the item and its model position.
fn attach_row_context_gesture(
    widget: gtk::Widget,
    list_item: gtk::ListItem,
    row_context_cb: &Rc<RefCell<Option<RowContextCb>>>,
) {
    let gesture = gtk::GestureClick::new();
    // GtkGestureSingle defaults to the primary button; the context menu is
    // button 3 (docs/05 §1). Capture phase sees the press before the
    // list's internal click gesture.
    gesture.set_button(3);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let slot = row_context_cb.clone();
    let item = list_item.clone();
    let widget_for_cb = widget.clone();
    gesture.connect_pressed(move |_gesture, _n, x, y| {
        let Some(row) = item.item().and_downcast::<ScanRow>() else { return };
        let position = item.position();
        if let Some(cb) = slot.borrow().as_ref() {
            cb(&row, position, &widget_for_cb, x, y);
        }
    });
    widget.add_controller(gesture);}

fn text_column(title: &str, kind: CellKind, row_context_cb: &Rc<RefCell<Option<RowContextCb>>>) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    let slot = row_context_cb.clone();
    factory.connect_setup(move |_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let label = gtk::Label::new(None);
        label.set_halign(Align::Start);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_margin_end(6);
        list_item.set_child(Some(&label));
        attach_row_context_gesture(label.upcast_ref::<gtk::Widget>().clone(), list_item.clone(), &slot);
    });
    factory.connect_bind(move |_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(row_obj) = list_item.item().and_downcast::<ScanRow>() else { return };
        let Some(label) = list_item.child().and_downcast::<gtk::Label>() else { return };
        let text = match kind {
            CellKind::Size => row_obj.size_str().to_string(),
            CellKind::Items => row_obj.items_str().to_string(),
            CellKind::Modified => row_obj.modified_str().to_string(),
            // The Folder column has its own factory.
            CellKind::Name => String::new(),
        };
        label.set_text(&text);
    });
    gtk::ColumnViewColumn::new(Some(title), Some(factory))
}

#[derive(Clone, Copy)]
enum CellKind {
    Name,
    Size,
    Items,
    Modified,
}

/// Header sorter for one column. The pinned location row always compares
/// first so it stays on top in every sort order (docs/02 §3).
fn row_compare(a: &glib::Object, b: &glib::Object, kind: CellKind) -> std::cmp::Ordering {
    let (Some(a), Some(b)) = (a.downcast_ref::<ScanRow>(), b.downcast_ref::<ScanRow>()) else {
        return std::cmp::Ordering::Equal;
    };
    match (a.is_location(), b.is_location()) {
        (true, true) => return std::cmp::Ordering::Equal,
        (true, false) => return std::cmp::Ordering::Less,
        (false, true) => return std::cmp::Ordering::Greater,
        (false, false) => {}
    }
    match kind {
        CellKind::Name => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
        CellKind::Size => a.size_bytes().cmp(&b.size_bytes()),
        CellKind::Items => a.items_count().cmp(&b.items_count()),
        CellKind::Modified => a.mtime().cmp(&b.mtime()),
    }
}

fn row_sorter(kind: CellKind) -> gtk::CustomSorter {
    gtk::CustomSorter::new(move |a, b| row_compare(a, b, kind).into())
}
