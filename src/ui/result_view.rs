//! Result view: two-pane layout with the item list on the left and the chart
//! pane on the right (docs/04 §1), plus the always-on infobar (docs/04 §6).
//!
//! Phase 2 scope: layout shell, chart-mode toggles, spinner page. The item
//! list and the charts themselves are wired in later phases.


// Some helpers are wired up in later implementation phases or used only
// by unit tests.
#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Align, Orientation};

use super::ChartMode;

pub struct ResultView {
    pub root: gtk::Box,
    /// Placeholder list area (ColumnView arrives with the scanner phase).
    pub list_area: gtk::Box,
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
        paned.set_wide_handle(true);
        root.append(&paned);

        // Left pane: item list (placeholder for now).
        let list_area = gtk::Box::new(Orientation::Vertical, 0);
        let list_scroll = gtk::ScrolledWindow::new();
        list_scroll.set_vexpand(true);
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
            list_area,
            chart_stack,
            spinner,
            toggle_rings,
            toggle_treemap,
        }
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
}

impl Default for ResultView {
    fn default() -> Self {
        Self::new()
    }
}
