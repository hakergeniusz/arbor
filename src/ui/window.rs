//! Main application window (docs/01).
//!
//! Owns the header bar (Back / location pill with breadcrumb / Rescan), the
//! start-page ⇄ result-view stack, the toast overlay, and the scan pipeline:
//! a worker thread sends incremental `ScanUpdate`s which are merged into a
//! `DisplayNode` tree and rendered as flat list rows.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, Orientation};

use adw::prelude::*;

use super::result_view::ResultView;
use super::scan_row::ScanRow;
use super::start_page::StartPage;
use super::{ChartMode, StartLocation};
use crate::models::DisplayNode;
use crate::scanner::{ScanHandle, ScanUpdate};
use crate::settings::{Settings, WindowSize};

pub struct ArborUi {
    pub window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    stack: gtk::Stack,
    back_btn: gtk::Button,
    rescan_btn: gtk::Button,
    title_start: gtk::Label,
    pill: gtk::Box,
    pill_icon: gtk::Image,
    pill_segments: gtk::Box,
    start_page: Rc<StartPage>,
    result_view: ResultView,
    pub settings: Rc<RefCell<Settings>>,
    state: RefCell<WindowState>,
}

struct WindowState {
    mode: ChartMode,
    /// The location currently shown in the result view (None → start page).
    current: Option<PathBuf>,
    /// Running scan handle, if any.
    scan: Option<ScanHandle>,
    /// Measured scan tree root (None until the first DirListed arrives).
    tree: Option<DisplayNode>,
    /// Folders expanded inline in the list.
    expanded: HashSet<PathBuf>,
    /// Folder the list currently shows (navigation inside a scan).
    current_folder: PathBuf,
    /// Auto-select the largest child on the next rebuild (docs/04 §2).
    pending_autoselect: bool,
}

impl ArborUi {
    pub fn build(app: &adw::Application) -> Rc<Self> {
        let settings = Rc::new(RefCell::new(load_settings()));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Devices & Locations")
            .icon_name(crate::app::APP_ID)
            .build();
        window.set_default_size(960, 600);

        load_css();

        let toast_overlay = adw::ToastOverlay::new();
        window.set_content(Some(&toast_overlay));

        let outer = gtk::Box::new(Orientation::Vertical, 0);
        toast_overlay.set_child(Some(&outer));

        // Header bar ------------------------------------------------------
        let header = gtk::HeaderBar::new();

        let back_btn = gtk::Button::from_icon_name("go-previous-symbolic");
        back_btn.set_tooltip_text(Some("Back"));
        back_btn.set_visible(false);
        header.pack_start(&back_btn);

        // Title widget: a stack switching between the plain title and the
        // location pill.
        let title_start = gtk::Label::new(Some("Devices & Locations"));
        title_start.add_css_class("title");

        let pill = gtk::Box::new(Orientation::Horizontal, 6);
        pill.add_css_class("location-pill");
        let pill_icon = gtk::Image::new();
        pill_icon.set_pixel_size(16);
        let pill_segments = gtk::Box::new(Orientation::Horizontal, 0);
        pill.append(&pill_icon);
        pill.append(&pill_segments);

        let title_stack = gtk::Stack::new();
        title_stack.set_transition_type(gtk::StackTransitionType::None);
        title_stack.set_hhomogeneous(false);
        title_stack.add_named(&title_start, Some("title"));
        title_stack.add_named(&pill, Some("pill"));
        header.set_title_widget(Some(&title_stack));

        let rescan_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
        rescan_btn.set_tooltip_text(Some("Rescan"));
        rescan_btn.set_visible(false);
        header.pack_end(&rescan_btn);

        let menu_btn = gtk::MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_tooltip_text(Some("Main Menu"));
        menu_btn.set_primary(true);
        menu_btn.set_menu_model(Some(&crate::app::main_menu_model()));
        header.pack_end(&menu_btn);

        outer.append(&header);

        // Content stack ---------------------------------------------------
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::None);
        outer.append(&stack);

        let start_page = StartPage::new(&settings);
        stack.add_named(start_page.root(), Some("start"));

        let result_view = ResultView::new();
        stack.add_named(&result_view.root, Some("result"));

        let ui = Rc::new(Self {
            window: window.clone(),
            toast_overlay,
            stack,
            back_btn,
            rescan_btn,
            title_start,
            pill,
            pill_icon,
            pill_segments,
            start_page,
            result_view,
            settings,
            state: RefCell::new(WindowState {
                mode: ChartMode::Rings,
                current: None,
                scan: None,
                tree: None,
                expanded: HashSet::new(),
                current_folder: PathBuf::new(),
                pending_autoselect: false,
            }),
        });

        ui.wire_handlers();
        if let Some(size) = ui.settings.borrow().window_size {
            window.set_default_size(size.width.max(400), size.height.max(300));
        }
        ui.start_page.rebuild(&ui.settings);

        window.present();
        ui
    }

    fn wire_handlers(self: &Rc<Self>) {
        // Back returns to the start page (S11), also mid-scan (S22).
        {
            let ui = self.clone();
            self.back_btn.connect_clicked(move |_| ui.show_start());
        }

        // Rescan re-scans the current location (S23).
        {
            let ui = self.clone();
            self.rescan_btn.connect_clicked(move |_| {
                if let Some(path) = ui.current_location() {
                    ui.open_location(&path);
                }
            });
        }

        // Chart mode toggle (per-window state, docs/02 §5).
        {
            let ui = self.clone();
            self.result_view.toggle_rings.connect_toggled(move |btn| {
                if btn.is_active() {
                    ui.state.borrow_mut().mode = ChartMode::Rings;
                    ui.show_chart_page();
                }
            });
        }
        {
            let ui = self.clone();
            self.result_view.toggle_treemap.connect_toggled(move |btn| {
                if btn.is_active() {
                    ui.state.borrow_mut().mode = ChartMode::Treemap;
                    ui.show_chart_page();
                }
            });
        }

        // Start-page row activation → open the location (S05, S10, D9).
        {
            let ui = self.clone();
            self.start_page
                .connect_activate(move |loc| ui.activate_start_location(loc));
        }

        // Row activation: folder click navigates, file click selects
        // (docs/02 §3), location row's up-arrow navigates to the parent.
        {
            let ui = self.clone();
            self.result_view.column_view.connect_activate(
                move |_, position| {
                    let Some(row) = ui
                        .result_view
                        .store
                        .item(position)
                        .and_downcast::<ScanRow>()
                    else {
                        return;
                    };
                    let path = PathBuf::from(row.path().to_string());
                    if row.is_location() {
                        // Up-arrow row: go to the parent folder (S15).
                        if let Some(parent) = path.parent() {
                            if parent != Path::new("") {
                                ui.navigate_within_scan(&parent);
                            }
                        }
                    } else if row.is_dir() {
                        ui.navigate_within_scan(&path);
                    }
                    // Files: single click selects (handled by the selection
                    // model); no navigation.
                },
            );
        }

        // Expander click → inline expand/collapse (S13b), routed as a window
        // action so the factory stays stateless.
        let toggle_expand = gio::SimpleAction::new(
            "toggle-expand",
            Some(&glib::VariantTy::STRING),
        );
        {
            let ui = self.clone();
            toggle_expand.connect_activate(move |_, param| {
                let Some(path_str) = param.and_then(|p| p.str()) else { return };
                let path = PathBuf::from(path_str);
                let mut state = ui.state.borrow_mut();
                if !state.expanded.remove(&path) {
                    state.expanded.insert(path);
                }
                drop(state);
                ui.rebuild_rows();
            });
        }
        self.window.add_action(&toggle_expand);

        // Escape in a result view navigates back, even mid-scan (S22);
        // dialogs handle their own Escape first.
        {
            let ui = self.clone();
            let controller = gtk::EventControllerKey::new();
            controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape
                    && ui.window.visible_dialog().is_none()
                    && ui.current_location().is_some()
                {
                    ui.show_start();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            self.window.add_controller(controller);
        }

        // Persist window size on close (decision D3).
        {
            let ui = self.clone();
            self.window.connect_close_request(move |window| {
                let (w, h) = window.default_size();
                ui.settings.borrow_mut().window_size = Some(WindowSize { width: w, height: h });
                save_settings(&ui.settings.borrow());
                glib::Propagation::Proceed
            });
        }
    }

    fn activate_start_location(self: &Rc<Self>, loc: &StartLocation) {
        let path = match loc {
            StartLocation::Home => glib::home_dir(),
            StartLocation::Mount(m) => m.root.clone(),
            StartLocation::Recent(p) => p.clone(),
        };
        self.open_location(&path);
    }

    /// Open (scan) a folder, switching to the result view (docs/03 §1).
    pub fn open_location(self: &Rc<Self>, path: &Path) {
        if !path.is_dir() {
            self.toast(&format!("“{}” is not a valid folder", path.display()));
            return;
        }

        // A new scan replaces any running one; partial data is freed (D5).
        self.cancel_scan();
        {
            let mut state = self.state.borrow_mut();
            state.current = Some(path.to_path_buf());
            state.current_folder = path.to_path_buf();
            state.tree = None;
            state.expanded.clear();
            state.pending_autoselect = true;
        }

        // Header state (docs/01 §3).
        self.back_btn.set_visible(true);
        self.rescan_btn.set_visible(true);
        self.title_stack().set_visible_child_name("pill");
        self.pill_icon.set_icon_name(Some(if path.to_path_buf() == glib::home_dir() {
            "user-home-symbolic"
        } else if path == Path::new("/") {
            "drive-harddisk-symbolic"
        } else {
            "folder-symbolic"
        }));
        self.set_breadcrumb(&[path.to_path_buf()]);

        self.window.set_title(Some(&self.display_name(path)));
        self.stack.set_visible_child_name("result");
        self.result_view.show_page("spinner");
        self.result_view.set_rows(&[]);

        let ignored = self.settings.borrow().ignored.clone();
        let recursive = self.settings.borrow().recursive_mounts;
        let (handle, receiver) = crate::scanner::spawn(path, recursive, ignored);
        self.state.borrow_mut().scan = Some(handle);

        // Drain scanner updates on the main loop (worker → UI bridge).
        let ui = self.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            loop {
                match receiver.try_recv() {
                    Ok(update) => {
                        let terminal = matches!(
                            update,
                            ScanUpdate::Finished { .. }
                                | ScanUpdate::Failed { .. }
                                | ScanUpdate::Cancelled
                        );
                        ui.handle_scan_update(update);
                        if terminal {
                            return glib::ControlFlow::Break;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        return glib::ControlFlow::Continue;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return glib::ControlFlow::Break;
                    }
                }
            }
        });
    }

    /// Navigate within the running/finished scan result (docs/02 §3).
    fn navigate_within_scan(self: &Rc<Self>, path: &Path) {
        {
            let mut state = self.state.borrow_mut();
            let Some(tree) = &state.tree else { return };
            if tree.find(path).is_none() {
                return; // not (yet) part of the scan
            }
            state.current_folder = path.to_path_buf();
            state.pending_autoselect = true;
        }
        // Breadcrumb = chain from scan root to this folder.
        let chain = self.breadcrumb_chain(path);
        self.set_breadcrumb(&chain);
        self.window
            .set_title(Some(&self.display_name(path)));
        self.pill_icon.set_icon_name(Some(if path.to_path_buf() == glib::home_dir() {
            "user-home-symbolic"
        } else {
            "folder-symbolic"
        }));
        self.rebuild_rows();
    }

    /// Ancestor chain from the scan root to `path` (inclusive).
    fn breadcrumb_chain(&self, path: &Path) -> Vec<PathBuf> {
        let state = self.state.borrow();
        let Some(root) = state.current.as_ref() else { return vec![] };
        let mut chain = vec![root.clone()];
        let mut current = root.clone();
        while current != path && path.starts_with(&current) {
            // Descend one component.
            let rel = path.strip_prefix(&current).unwrap_or(Path::new(""));
            let mut components = rel.components();
            if let Some(comp) = components.next() {
                current.push(comp);
                chain.push(current.clone());
            } else {
                break;
            }
        }
        chain
    }

    fn handle_scan_update(self: &Rc<Self>, update: ScanUpdate) {
        match update {
            ScanUpdate::DirListed { entries } => {
                {
                    let mut state = self.state.borrow_mut();
                    // The first DirListed carries the scan root's children.
                    if state.tree.is_none() {
                        let Some(root_meta) = entries.first() else { return };
                        let Some(root_path) = root_meta.path.parent() else { return };
                        state.tree = Some(DisplayNode::new_dir(&EntryMeta {
                            name: self.display_name(root_path),
                            path: root_path.to_path_buf(),
                            is_dir: true,
                            error: false,
                            own_size: 0,
                            mtime: None,
                        }));
                    }
                    let tree = state.tree.as_mut().unwrap();
                    // Merge entries under their common parent.
                    for meta in &entries {
                        let Some(parent_path) = meta.path.parent() else { continue };
                        if let Some(parent) = tree.find_mut(parent_path) {
                            if parent
                                .children
                                .iter()
                                .any(|c| c.path == meta.path)
                            {
                                continue; // already known; DirMeasured refines it
                            }
                            let node = if meta.is_dir {
                                DisplayNode::new_dir(meta)
                            } else {
                                DisplayNode::new_file(meta)
                            };
                            parent.size += node.size;
                            parent.items += 1;
                            parent.children.push(node);
                        }
                    }
                }
                self.rebuild_rows();
            }
            ScanUpdate::DirMeasured { path, size, items, error, mtime } => {
                let mut state = self.state.borrow_mut();
                let Some(tree) = &mut state.tree else { return };
                if let Some(node) = tree.find_mut(&path) {
                    let old = node.size;
                    node.size = size;
                    node.items = items;
                    node.error = error;
                    node.mtime = Some(mtime);
                    node.measured = true;
                    propagate_delta_up(tree, &path, size.saturating_sub(old), 0);
                }
                let is_current = state.current_folder == path;
                let mut select_largest = false;
                if is_current && state.pending_autoselect {
                    select_largest = true;
                    state.pending_autoselect = false;
                }
                drop(state);
                self.rebuild_rows();
                if select_largest {
                    self.select_largest_child();
                }
            }
            ScanUpdate::Finished { root } => {
                {
                    let mut state = self.state.borrow_mut();
                    state.scan = None;
                }
                // Completed scans enter the recents list (docs/03 §6).
                self.settings.borrow_mut().push_recent(&root.display().to_string());
                self.refresh_start_page();
                self.rebuild_rows();
                self.show_chart_page();
            }
            ScanUpdate::Failed { path } => {
                self.cancel_scan();
                self.show_start();
                self.toast(&format!(
                    "“{}” is not a valid folder",
                    path.display()
                ));
            }
            ScanUpdate::Cancelled => {
                // The view is already gone; nothing to render.
            }
        }
    }

    /// Select the largest measured child of the current folder
    /// (docs/04 §2: after navigation the largest child is auto-selected).
    fn select_largest_child(self: &Rc<Self>) {
        let state = self.state.borrow();
        let Some(tree) = &state.tree else { return };
        let Some(current) = tree.find(&state.current_folder) else { return };
        let largest = current
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| c.measured || !c.is_dir)
            .max_by_key(|(_, c)| c.size)
            .map(|(i, _)| i);
        drop(state);
        if let Some(index) = largest {
            // +1 for the pinned location row.
            self.result_view.select_position((index + 1) as u32);
        }
    }

    /// Rebuild the flat row list for `current_folder` from the tree.
    fn rebuild_rows(self: &Rc<Self>) {
        let rows = {
            let state = self.state.borrow();
            let Some(tree) = &state.tree else { return };
            let Some(current) = tree.find(&state.current_folder) else { return };
            build_rows(current, &state.expanded)
        };
        self.result_view.set_rows(&rows);
    }

    fn show_chart_page(&self) {
        // Charts land in phase 4; until then the toggles switch to the
        // placeholder only if no scan is running.
        if self.state.borrow().scan.is_none() {
            let name = match self.state.borrow().mode {
                ChartMode::Rings => "rings",
                ChartMode::Treemap => "treemap",
            };
            self.result_view.show_page(name);
        }
    }

    /// Return to the start page (Back/Escape), stopping any running scan.
    pub fn show_start(self: &Rc<Self>) {
        self.cancel_scan();
        self.state.borrow_mut().current = None;
        self.back_btn.set_visible(false);
        self.rescan_btn.set_visible(false);
        self.title_stack().set_visible_child_name("title");
        self.stack.set_visible_child_name("start");
        self.window.set_title(Some("Devices & Locations"));
    }

    fn cancel_scan(&self) {
        self.state.borrow_mut().scan = None; // Drop cancels + joins the worker.
    }

    /// The location currently shown in the result view, if any.
    pub fn current_location(&self) -> Option<PathBuf> {
        self.state.borrow().current.clone()
    }

    fn title_stack(&self) -> gtk::Stack {
        self.pill
            .ancestor(gtk::Stack::static_type())
            .and_downcast::<gtk::Stack>()
            .expect("pill is inside the title stack")
    }

    /// Human display name of a location (docs/02 §8).
    pub fn display_name(&self, path: &Path) -> String {
        if path.to_path_buf() == glib::home_dir() {
            return "Home Folder".to_string();
        }
        for mount in crate::mounts::list_mounts() {
            if mount.root == path {
                return mount.name;
            }
        }
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
    }

    /// Rebuild the clickable breadcrumb segments for `chain` (root..=current).
    fn set_breadcrumb(self: &Rc<Self>, chain: &[PathBuf]) {
        while let Some(child) = self.pill_segments.first_child() {
            self.pill_segments.remove(&child);
        }
        for (i, seg) in chain.iter().enumerate() {
            if i > 0 {
                let slash = gtk::Label::new(Some("/"));
                slash.add_css_class("dim-label");
                self.pill_segments.append(&slash);
            }
            let name = self.display_name(seg);
            let btn = gtk::Button::with_label(&name);
            btn.add_css_class("flat");
            let seg = seg.clone();
            let ui = self.clone();
            btn.connect_clicked(move |_| {
                // Clicking a breadcrumb segment navigates to that ancestor
                // (docs/02 §3, S16).
                let is_root = {
                    let state = ui.state.borrow();
                    state.current.as_ref() == Some(&seg)
                };
                if is_root {
                    ui.open_location(&seg);
                } else {
                    ui.navigate_within_scan(&seg);
                }
            });
            self.pill_segments.append(&btn);
        }
    }

    /// Show a transient toast with an embedded dismiss button, role=alert,
    /// auto-dismissing in ~4 s (docs/07 §7).
    pub fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        toast.set_priority(adw::ToastPriority::High);
        self.toast_overlay.add_toast(toast);
    }

    /// Refresh the start page from settings (after recents change).
    pub fn refresh_start_page(&self) {
        self.start_page.rebuild(&self.settings);
        save_settings(&self.settings.borrow());
    }
}

use crate::models::EntryMeta;

/// After a child node's aggregate was (re)computed, roll the delta up to all
/// ancestors so running totals stay live during the scan (docs/03 §2).
fn propagate_delta_up(tree: &mut DisplayNode, child_path: &Path, size_delta: u64, items_delta: u64) {
    let mut parent_path = child_path.parent();
    while let Some(pp) = parent_path {
        if let Some(parent) = tree.find_mut(pp) {
            parent.size = parent.size.saturating_add(size_delta);
            parent.items = parent.items.saturating_add(items_delta);
            parent_path = pp.parent();
        } else {
            break;
        }
    }
}

/// When a child node is replaced (re-listed), adjust the parent aggregate.
fn propagate_add(parent: &mut DisplayNode, old_child_size: u64) {
    let new_total: u64 = parent.children.iter().map(|c| c.size).sum();
    parent.size = parent.size.saturating_sub(old_child_size).max(new_total + 1);
}

/// Build the flat row list: pinned location row + sorted children, expanded
/// folders inline (docs/04 §2, docs/02 §3).
fn build_rows(current: &DisplayNode, expanded: &HashSet<PathBuf>) -> Vec<ScanRow> {
    let total = current.size.max(1);
    let mut rows = Vec::new();

    // Pinned location row with up-arrow (docs/02 §3).
    let loc = ScanRow::new(current.path.clone());
    fill_row(&loc, current, total, true, 0, false);
    rows.push(loc);

    emit_children(current, expanded, total, 1, &mut rows);
    rows
}

fn emit_children(
    node: &DisplayNode,
    expanded: &HashSet<PathBuf>,
    total: u64,
    depth: u32,
    rows: &mut Vec<ScanRow>,
) {
    // Initial order: size descending, ties by case-insensitive name (D7).
    let mut children: Vec<&DisplayNode> = node.children.iter().collect();
    children.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for child in children {
        let row = ScanRow::new(child.path.clone());
        fill_row(&row, child, total, false, depth, expanded.contains(&child.path));
        rows.push(row);
        if child.is_dir && expanded.contains(&child.path) {
            emit_children(child, expanded, total, depth + 1, rows);
        }
    }
}

fn fill_row(
    row: &ScanRow,
    node: &DisplayNode,
    total: u64,
    is_location: bool,
    depth: u32,
    is_expanded: bool,
) {
    row.set_name(node.name.as_str());
    row.set_size_str(crate::format::size(node.size).as_str());
    row.set_items_str(crate::format::items(node.items).as_str());
    let now = glib::DateTime::now_local()
        .unwrap_or_else(|_| glib::DateTime::from_unix_local(0).unwrap());
    row.set_modified_str(
        node.mtime
            .map(|m| crate::format::relative_day(m, &now))
            .unwrap_or_else(|| "Unknown".to_string())
            .as_str(),
    );
    row.set_percent(node.size as f64 / total as f64);
    row.set_size_bytes(node.size);
    row.set_items_count(node.items);
    row.set_mtime(node.mtime.unwrap_or(0));
    row.set_is_location(is_location);
    row.set_is_dir(node.is_dir);
    row.set_error(node.error);
    row.set_depth(depth);
    row.set_has_children(node.is_dir && (!node.measured || !node.children.is_empty()));
    row.set_expanded(is_expanded);
}

fn load_settings() -> Settings {
    Settings::default_path()
        .map(|p| Settings::load_from(&p))
        .unwrap_or_default()
}

fn save_settings(s: &Settings) {
    if let Some(path) = Settings::default_path() {
        if let Err(err) = s.save_to(&path) {
            eprintln!("arbor: failed to persist settings: {err}");
        }
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .location-pill {
            background-color: alpha(currentColor, 0.08);
            border-radius: 9999px;
            padding: 2px 8px;
        }
        .infobar {
            background-color: alpha(currentColor, 0.08);
        }
        .disc {
            background-color: alpha(currentColor, 0.12);
            border-radius: 9999px;
        }
        progressbar.usage-bar > trough {
            border-radius: 4px;
            min-height: 8px;
        }
        progressbar.usage-bar > trough > progress {
            border-radius: 4px;
            min-height: 8px;
        }
        .percent-badge {
            border-radius: 9999px;
            padding: 0 6px;
            background-color: alpha(@accent_color, 0.15);
        }
        .percent-badge > progressbar.mini {
            min-width: 24px;
        }
        .percent-badge > progressbar.mini > trough {
            min-height: 6px;
            min-width: 24px;
            border-radius: 3px;
        }
        .percent-badge > progressbar.mini > trough > progress {
            border-radius: 3px;
            min-height: 6px;
        }
        .percent-badge > label.mini {
            font-size: 0.85em;
        }
        "#,
    );
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
