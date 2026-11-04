//! Result view: two-pane layout with the item list on the left and the chart
//! pane on the right (docs/04 §1), plus the always-on infobar (docs/04 §6).

// The chart pages and sort wiring land with the visualization phase; some
// accessors are reserved for it.
#![allow(dead_code)]

use gtk::gio;
use gtk::prelude::*;
use gtk::{Align, Orientation};

use super::scan_row::ScanRow;
use super::ChartMode;

pub struct ResultView {
    pub root: gtk::Box,
    pub list_scroll: gtk::ScrolledWindow,
    pub column_view: gtk::ColumnView,
    pub store: gio::ListStore,
    pub selection: gtk::SingleSelection,
    pub chart_stack: gtk::Stack,
    pub spinner: gtk::Spinner,
    pub toggle_rings: gtk::ToggleButton,
    pub toggle_treemap: gtk::ToggleButton,
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

        let folder_col = gtk::ColumnViewColumn::new(Some("Folder"), Some(folder_factory()));
        folder_col.set_resizable(true);
        folder_col.set_expand(true);
        column_view.append_column(&folder_col);
        let size_col = text_column("Size", CellKind::Size);
        size_col.set_fixed_width(88);
        column_view.append_column(&size_col);
        let items_col = text_column("Contents", CellKind::Items);
        items_col.set_fixed_width(74);
        column_view.append_column(&items_col);
        let modified_col = text_column("Modified", CellKind::Modified);
        modified_col.set_fixed_width(82);
        column_view.append_column(&modified_col);

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

        let placeholder = gtk::Box::new(Orientation::Vertical, 0);
        chart_stack.add_named(&placeholder, Some("rings"));
        chart_stack.add_named(&placeholder, Some("treemap"));

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
            chart_stack,
            spinner,
            toggle_rings,
            toggle_treemap,
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

    /// Select the row at `position` (used to auto-select the largest child,
    /// docs/04 §2).
    pub fn select_position(&self, position: u32) {
        if position < self.store.n_items() {
            self.selection.set_selected(position);
        }
    }
}

impl Default for ResultView {
    fn default() -> Self {
        Self::new()
    }
}

/// The "Folder" column: expander + percent badge + color-coded name.
fn folder_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, obj| {
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

        let badge = gtk::Box::new(Orientation::Horizontal, 4);
        badge.add_css_class("percent-badge");
        badge.set_valign(Align::Center);
        let badge_bar = gtk::ProgressBar::new();
        badge_bar.add_css_class("mini");
        badge_bar.set_width_request(24);
        let badge_label = gtk::Label::new(None);
        badge_label.add_css_class("mini");
        badge_label.set_width_request(42);
        badge.append(&badge_bar);
        badge.append(&badge_label);

        let name = gtk::Label::new(None);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_hexpand(true);
        name.set_halign(Align::Start);

        row.append(&expander);
        row.append(&badge);
        row.append(&name);
        list_item.set_child(Some(&row));
    });
    factory.connect_bind(|_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(row_obj) = list_item.item().and_downcast::<ScanRow>() else { return };
        let Some(container) = list_item.child().and_downcast::<gtk::Box>() else { return };
        // Children were appended in this order in setup.
        let Some(expander) = container.first_child().and_downcast::<gtk::Button>() else { return };
        let Some(badge) = expander.next_sibling().and_downcast::<gtk::Box>() else { return };
        let Some(badge_bar) = badge.first_child().and_downcast::<gtk::ProgressBar>() else { return };
        let Some(badge_label) = badge_bar.next_sibling().and_downcast::<gtk::Label>() else { return };
        let Some(name) = badge.next_sibling().and_downcast::<gtk::Label>() else { return };
        let Some(arrow) = expander.child().and_downcast::<gtk::Image>() else { return };

        name.set_text(&row_obj.name());
        let color = if row_obj.error() {
            ERROR_COLOR.to_string()
        } else if row_obj.is_location() {
            String::new()
        } else {
            row_obj.color().to_string()
        };
        if color.is_empty() {
            name.set_attributes(None);
        } else if let Ok(rgba) = gtk::gdk::RGBA::parse(&color) {
            let attrs = gtk::pango::AttrList::new();
            attrs.insert(gtk::pango::AttrColor::new_foreground(
                (rgba.red() * 65535.0) as u16,
                (rgba.green() * 65535.0) as u16,
                (rgba.blue() * 65535.0) as u16,
            ));
            name.set_attributes(Some(&attrs));
        }

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

fn text_column(title: &str, kind: CellKind) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let label = gtk::Label::new(None);
        label.set_halign(Align::Start);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_margin_end(6);
        list_item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, obj| {
        let list_item = obj.downcast_ref::<gtk::ListItem>().expect("ListItem");
        let Some(row_obj) = list_item.item().and_downcast::<ScanRow>() else { return };
        let Some(label) = list_item.child().and_downcast::<gtk::Label>() else { return };
        let text = match kind {
            CellKind::Size => row_obj.size_str().to_string(),
            CellKind::Items => row_obj.items_str().to_string(),
            CellKind::Modified => row_obj.modified_str().to_string(),
        };
        label.set_text(&text);
    });
    gtk::ColumnViewColumn::new(Some(title), Some(factory))
}

#[derive(Clone, Copy)]
enum CellKind {
    Size,
    Items,
    Modified,
}
