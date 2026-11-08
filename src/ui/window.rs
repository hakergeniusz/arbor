//! Main application window (docs/01).
//!
//! Owns the header bar (Back / location pill with breadcrumb / Rescan), the
//! start-page ⇄ result-view stack, the toast overlay, and the scan pipeline:
//! a worker thread sends incremental `ScanUpdate`s which are merged into a
//! `DisplayNode` tree and rendered as flat list rows.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::{gio, Orientation};

use adw::prelude::*;

use super::charts::{ChartNode, ChartState};
use super::result_view::ResultView;
use super::scan_row::ScanRow;
use super::start_page::StartPage;
use super::{ChartMode, StartLocation};
use crate::models::DisplayNode;
use crate::scanner::{ScanHandle, ScanUpdate};
use crate::settings::{Settings, WindowSize};

/// Column sort state mirrored from the view's header (docs/04 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Name,
    Size,
    Items,
    Modified,
}

/// Result of a cache lookup for a folder about to be opened.
enum CacheLoad {
    /// No usable snapshot — scan from scratch.
    None,
    /// Snapshot decoded and ready to install.
    Ready(crate::cache::CachedScan),
    /// Snapshot exists but is big; the worker decodes it and delivers the
    /// outcome over this channel (Some = decoded, None = unusable).
    Pending(std::sync::mpsc::Receiver<Option<crate::cache::CachedScan>>),
}

/// Trees up to this many nodes decode synchronously on the open path
/// (sub-100 ms); bigger snapshots decode on a worker behind the spinner.
const CACHE_SYNC_DECODE_NODES: u32 = 250_000;

pub struct ArborUi {
    pub window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    stack: gtk::Stack,
    back_btn: gtk::Button,
    rescan_btn: gtk::Button,
    title_start: gtk::Label,
    menu_btn: gtk::MenuButton,
    pill: gtk::Box,
    pill_icon: gtk::Image,
    pill_segments: gtk::Box,
    start_page: Rc<StartPage>,
    result_view: ResultView,
    pub settings: Rc<RefCell<Settings>>,
    state: RefCell<WindowState>,
    /// The one open item context menu, if any.
    item_popover: RefCell<Option<gtk::PopoverMenu>>,
}

struct WindowState {
    mode: ChartMode,
    /// The location currently shown in the result view (None → start page).
    current: Option<PathBuf>,
    /// Running scan handle, if any.
    scan: Option<ScanHandle>,
    /// Measured scan tree root (None until the first DirListed arrives).
    tree: Option<DisplayNode>,
    /// Paths already merged into `tree` (fresh per scan) — the O(1)
    /// duplicate guard for `merge_entries` (see models.rs).
    known: HashSet<PathBuf>,
    /// Folders expanded inline in the list.
    expanded: HashSet<PathBuf>,
    /// Folder the list currently shows (navigation inside a scan).
    current_folder: PathBuf,
    /// Auto-select the largest child on the next rebuild (docs/04 §2).
    pending_autoselect: bool,
    /// Column sort state (mirrors the header, docs/04 §3).
    sort: (SortKey, bool),
    /// The last successful trash operation, for the toast's Undo button
    /// (docs/06 R6).
    undo: Option<crate::trash::TrashRecord>,
    /// Tree mutations from the scanner that the list has not shown yet.
    rows_dirty: bool,
    /// Auto-select deferred to the next throttled repaint (the row may not
    /// exist in the store until the list is rebuilt).
    select_largest: bool,
    /// Last throttled list repaint — the list is rebuilt at most ~8×/s
    /// while a scan streams in.
    last_repaint: Cell<std::time::Instant>,
    /// Last chart update — charts clone the visible subtree, so while
    /// scanning they refresh at most 2×/s.
    last_chart: Cell<std::time::Instant>,
    /// Cached snapshot decode in flight (big trees), with its channel.
    pending_cache: Option<std::sync::mpsc::Receiver<Option<crate::cache::CachedScan>>>,
    /// Entries merged so far in the running scan (progress counters).
    scan_entries: u64,
    /// Own bytes seen so far in the running scan (progress counters).
    scan_bytes: u64,
    /// Bumped on every tree mutation (scan merges, measured aggregates,
    /// trash detach) — invalidates the chart projection caches.
    tree_gen: u64,
    /// Cached rings projection of `current_folder`: (tree_gen, folder, node).
    /// Selection changes reuse it; only tree edits or navigation re-project.
    rings_cache: Option<(u64, PathBuf, Rc<ChartNode>)>,
    /// Cached treemap projection of the treemap source folder.
    treemap_cache: Option<(u64, PathBuf, Rc<ChartNode>)>,
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
        // location pill. The stack expands across the header's center and an
        // AdwClamp caps the pill so it renders as a wide, centered field
        // (docs/01 §3, docs/10 §2) instead of hugging its text.
        let title_start = gtk::Label::new(Some("Devices & Locations"));
        title_start.add_css_class("title");

        let pill = gtk::Box::new(Orientation::Horizontal, 6);
        pill.add_css_class("location-pill");
        let pill_icon = gtk::Image::new();
        pill_icon.set_pixel_size(16);
        let pill_segments = gtk::Box::new(Orientation::Horizontal, 0);
        pill.append(&pill_icon);
        pill.append(&pill_segments);
        pill.set_hexpand(true);

        let pill_clamp = adw::Clamp::new();
        pill_clamp.set_maximum_size(880);
        pill_clamp.set_tightening_threshold(480);
        pill_clamp.set_child(Some(&pill));

        let title_stack = gtk::Stack::new();
        title_stack.set_transition_type(gtk::StackTransitionType::None);
        title_stack.set_hhomogeneous(false);
        title_stack.set_hexpand(true);
        title_stack.add_named(&title_start, Some("title"));
        title_stack.add_named(&pill_clamp, Some("pill"));
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
            menu_btn,
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
                known: HashSet::new(),
                expanded: HashSet::new(),
                current_folder: PathBuf::new(),
                pending_autoselect: false,
                sort: (SortKey::Size, true),
                undo: None,
                rows_dirty: false,
                select_largest: false,
                last_repaint: Cell::new(std::time::Instant::now()),
                last_chart: Cell::new(std::time::Instant::now()),
                pending_cache: None,
                scan_entries: 0,
                scan_bytes: 0,
                tree_gen: 0,
                rings_cache: None,
                treemap_cache: None,
            }),
            item_popover: RefCell::new(None),
        });

        ui.wire_handlers();
        if let Some(size) = ui.settings.borrow().window_size {
            window.set_default_size(size.width.max(400), size.height.max(300));
        }
        // One-time cleanup of state written before mount roots stopped being
        // recorded: their rows duplicated the permanent device rows.
        {
            let mut settings = ui.settings.borrow_mut();
            if !settings.recents.is_empty() {
                let home = glib::home_dir();
                let mounts = crate::mounts::list_mounts();
                let before = settings.recents.len();
                settings
                    .recents
                    .retain(|p| Path::new(p) != home && !mounts.iter().any(|m| m.root == Path::new(p)));
                if settings.recents.len() != before {
                    save_settings(&settings);
                }
            }
        }
        ui.start_page.rebuild(&ui.settings);
        ui.apply_chart_visibility();

        window.present();
        ui
    }

    fn wire_handlers(self: &Rc<Self>) {
        // Back returns to the start page (S11), also mid-scan (S22).
        {
            let ui = self.clone();
            self.back_btn.connect_clicked(move |_| ui.show_start());
        }

        // Rescan re-scans the current location (S23) — a real rescan, not a
        // cache restore.
        {
            let ui = self.clone();
            self.rescan_btn.connect_clicked(move |_| ui.rescan_current());
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

        // ^ on the pinned location row: navigate to the parent folder
        // (docs/02 §3). No-op at the scan root (no parent inside the scan).
        let navigate_up = gio::SimpleAction::new("navigate-up", None);
        {
            let ui = self.clone();
            navigate_up.connect_activate(move |_, _| {
                let current = ui.state.borrow().current_folder.clone();
                if let Some(parent) = current.parent() {
                    if parent != Path::new("") {
                        ui.navigate_within_scan(parent);
                    }
                }
            });
        }
        self.window.add_action(&navigate_up);

        // Treemap interactions (docs/04 §8): click selects the matching row
        // (falling back to the nearest visible ancestor when the hit item is
        // hidden inside a collapsed folder), double-click on a folder region
        // navigates into it. The context menu is wired in the item-actions
        // phase (docs/05).
        {
            let ui = self.clone();
            self.result_view.treemap_chart.connect_rect_selected(
                move |_, path| {
                    let p = PathBuf::from(&path);
                    if ui.result_view.select_row_by_path(&p) {
                        return;
                    }
                    let mut anc = p.parent();
                    while let Some(a) = anc {
                        if ui.result_view.select_row_by_path(&a) {
                            return;
                        }
                        anc = a.parent();
                    }
                },
            );
        }
        {
            let ui = self.clone();
            self.result_view.treemap_chart.connect_rect_activated(
                move |_, path| {
                    ui.navigate_within_scan(Path::new(&path));
                },
            );
        }

        // Selection drives the charts: the treemap follows the selection and
        // the selected ring segment is highlighted (docs/04 §7–§8).
        {
            let ui = self.clone();
            self.result_view
                .selection
                .connect_selection_changed(move |_, _, _| {
                    ui.update_charts();
                });
        }

        // Header sorting: clicking a header mutates the view's internal
        // ColumnViewSorter in place — its `changed` signal (not the property's
        // notify) carries the new order. The order is mirrored into a sibling
        // sort during the row rebuild — a flat model sorter would interleave
        // expanded children, so the hierarchy is preserved here.
        if let Some(sorter) = self.result_view.column_view.sorter() {
            let ui = self.clone();
            sorter.connect_changed(move |_, _| ui.apply_header_sort());
        }
        {
            let ui = self.clone();
            self.result_view
                .column_view
                .connect_notify_local(Some("sorter"), move |_, _| {
                    ui.apply_header_sort();
                });
        }
        // Item actions (docs/05): Open Externally / Copy Path to Clipboard /
        // Move to Trash, offered from the context menu on list rows and
        // treemap rectangles.
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("open-externally", Some(&glib::VariantTy::STRING));
            action.connect_activate(move |_, param| {
                if let Some(path) = param.and_then(|p| p.str()) {
                    ui.open_externally(path);
                }
            });
            self.window.add_action(&action);
        }
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("copy-path", Some(&glib::VariantTy::STRING));
            action.connect_activate(move |_, param| {
                if let Some(path) = param.and_then(|p| p.str()) {
                    ui.copy_path(path);
                }
            });
            self.window.add_action(&action);
        }
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("trash-item", Some(&glib::VariantTy::STRING));
            action.connect_activate(move |_, param| {
                if let Some(path) = param.and_then(|p| p.str()) {
                    ui.begin_trash(path);
                }
            });
            self.window.add_action(&action);
        }
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("undo-trash", None);
            action.connect_activate(move |_, _| ui.undo_trash());
            self.window.add_action(&action);
        }

        // Keyboard paths for the menus (docs/08 §2, §4.2): F10 opens the
        // Main Menu; Menu key / Shift+F10 opens the item context menu for
        // the selected row — the keyboard equivalent of right-clicking.
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("main-menu", None);
            action.connect_activate(move |_, _| {
                // The Main Menu button only exists on the start-page header
                // (docs/10 §2: the result view has no ☰).
                if ui.menu_btn.is_visible() {
                    ui.menu_btn.popup();
                }
            });
            self.window.add_action(&action);
        }
        {
            let ui = self.clone();
            let action = gio::SimpleAction::new("item-menu", None);
            action.connect_activate(move |_, _| {
                // Only meaningful in a result view: on the start page there is
                // no item row (a stale selection from a previous scan must not
                // resurrect the menu there).
                if ui.current_location().is_none() {
                    return;
                }
                let Some(row) = ui.result_view.selected_row() else { return };
                let path = row.path().to_string();
                // Anchor at the focused row cell when the focus is inside
                // the list (arrow-key navigation), else at the list top.
                let list_type = gtk::ColumnView::static_type();
                let (widget, x, y) = match gtk::prelude::GtkWindowExt::focus(&ui.window) {
                    Some(w) if w.ancestor(list_type).is_some() => {
                        // WidgetExt::allocation is superseded in 4.12.
                        #[allow(deprecated)]
                        let alloc = w.allocation();
                        (w, 6.0, f64::from(alloc.height()) - 2.0)
                    }
                    _ => {
                        let view = ui.result_view.column_view.clone();
                        (view.upcast_ref::<gtk::Widget>().clone(), 150.0, 40.0)
                    }
                };
                ui.show_item_menu(&path, &widget, x, y);
            });
            self.window.add_action(&action);
        }

        // Right-click on a row selects it and opens the context menu
        // (docs/05 §1). The gesture lives on the row widgets (see
        // ResultView); here we receive the resolved row.
        {
            let ui = self.clone();
            self.result_view.connect_row_context(
                move |row, position, widget, x, y| {
                    ui.result_view.selection.set_selected(position);
                    let path = row.path().to_string();
                    ui.show_item_menu(&path, widget, x, y);
                },
            );
        }

        // Right-click on a treemap rectangle opens the same menu (docs/05 §1:
        // observed in both chart modes).
        {
            let ui = self.clone();
            self.result_view.treemap_chart.connect_rect_context(
                move |_, path, x, y| {
                    ui.show_item_menu(
                        &path,
                        ui.result_view.treemap_chart.upcast_ref::<gtk::Widget>(),
                        x,
                        y,
                    );
                },
            );
        }

        self.apply_header_sort();

        // Escape in a result view navigates back, even mid-scan (S22);
        // dialogs handle their own Escape first, and so does the item
        // context menu (S14/S15: Escape with the menu open closes it and
        // performs nothing).
        {
            let ui = self.clone();
            let controller = gtk::EventControllerKey::new();
            controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape
                    && ui.window.visible_dialog().is_none()
                    && !ui.focus_in_popover()
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

    /// Read the view's primary sort column/direction and rebuild the rows if
    /// it differs from the mirrored state.
    fn apply_header_sort(self: &Rc<Self>) {
        let Some(sorter) = self.result_view.column_view.sorter() else { return };
        let Some(cvs) = sorter.downcast_ref::<gtk::ColumnViewSorter>() else { return };
        let Some(col) = cvs.primary_sort_column() else { return };
        let key = match col.title().unwrap_or_default().as_str() {
            "Folder" => SortKey::Name,
            "Size" => SortKey::Size,
            "Contents" => SortKey::Items,
            "Modified" => SortKey::Modified,
            _ => return,
        };
        let descending = cvs.primary_sort_order() == gtk::SortType::Descending;
        {
            let mut state = self.state.borrow_mut();
            if state.sort == (key, descending) {
                return;
            }
            state.sort = (key, descending);
        }
        self.rebuild_rows();
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
    /// With the scan cache enabled (user request), a previously finished
    /// scan of this folder is restored instantly and the view stays on it —
    /// no automatic rescan (recalculating on every open defeated the cache;
    /// refreshing is what ↻/Ctrl+R are for). Without a cache hit the folder
    /// is scanned from scratch.
    pub fn open_location(self: &Rc<Self>, path: &Path) {
        if !path.is_dir() {
            self.toast(&format!("“{}” is not a valid folder", path.display()));
            return;
        }

        // A new scan replaces any running one; partial data is freed (D5).
        self.cancel_scan();
        self.prepare_result_view(path);

        let cached = if self.settings.borrow().cache_scans {
            self.load_cached(path)
        } else {
            CacheLoad::None
        };
        match cached {
            CacheLoad::None => self.start_scan(path),
            CacheLoad::Ready(snapshot) => self.install_cached(path, snapshot),
            CacheLoad::Pending(receiver) => {
                // Big snapshot: decode on the worker, spinner meanwhile.
                self.result_view.show_page("spinner");
                self.result_view
                    .set_scan_progress(true, Some("Loading cached scan…"));
                self.state.borrow_mut().pending_cache = Some(receiver);
                let ui = self.clone();
                let path = path.to_path_buf();
                glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                    let outcome = {
                        let mut state = ui.state.borrow_mut();
                        let Some(receiver) = state.pending_cache.as_mut() else {
                            return glib::ControlFlow::Break;
                        };
                        match receiver.try_recv() {
                            Ok(delivered) => {
                                state.pending_cache = None;
                                Some(delivered)
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                drop(state);
                                ui.result_view.set_scan_progress(true, None); // pulse
                                return glib::ControlFlow::Continue;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                state.pending_cache = None;
                                Some(None)
                            }
                        }
                    };
                    match outcome {
                        Some(Some(snapshot)) => ui.install_cached(&path, snapshot),
                        _ => ui.start_scan(&path),
                    }
                    glib::ControlFlow::Break
                });
            }
        }
    }

    /// True re-scan of the current location (S23): bypasses the cache read
    /// entirely and rebuilds the tree from disk.
    pub fn rescan_current(self: &Rc<Self>) {
        let Some(path) = self.current_location() else { return };
        self.cancel_scan();
        self.start_scan(&path);
    }

    /// Header/state shared by every path into the result view (docs/01 §3):
    /// Back + Rescan appear, the Main Menu button does not — the header is
    /// [‹ Back] [pill] [↻] [window controls] (docs/10 §2).
    fn prepare_result_view(self: &Rc<Self>, path: &Path) {
        self.back_btn.set_visible(true);
        self.rescan_btn.set_visible(true);
        self.menu_btn.set_visible(false);
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
    }

    /// Try to restore a cached snapshot for `path`. Small trees decode on
    /// the spot; big ones (header node count above the threshold) decode on
    /// a worker thread so the main loop never stalls on the restore.
    fn load_cached(self: &Rc<Self>, path: &Path) -> CacheLoad {
        let Some(snapshot) = crate::cache::peek(path) else {
            return CacheLoad::None;
        };
        if snapshot.node_count <= CACHE_SYNC_DECODE_NODES {
            match crate::cache::load(path) {
                Some(snapshot) => CacheLoad::Ready(snapshot),
                None => CacheLoad::None,
            }
        } else {
            let probe = path.to_path_buf();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("arbor-cache-load".into())
                .spawn(move || {
                    let _ = sender.send(crate::cache::load(&probe));
                })
                .ok();
            let _ = snapshot; // header info only; the worker re-reads the file
            CacheLoad::Pending(receiver)
        }
    }

    /// Install a restored snapshot as the displayed tree and finalize the
    /// view exactly like a completed scan (rows, charts, recents) — minus
    /// the rescan: the cache hit is the result until the user asks for
    /// fresh data (↻/Ctrl+R). A toast states the restore and its age so a
    /// stale snapshot is never mistaken for a fresh scan.
    fn install_cached(self: &Rc<Self>, path: &Path, snapshot: crate::cache::CachedScan) {
        let scanned_at = snapshot.scanned_at;
        {
            let mut state = self.state.borrow_mut();
            let mut root = snapshot.root;
            // The cache stores the filesystem name; the UI tree uses the display
            // name (e.g. "Home Folder"), exactly like a fresh scan's root.
            root.name = self.display_name(path);
            state.tree = Some(root);
            state.current = Some(path.to_path_buf());
            state.current_folder = path.to_path_buf();
            state.rings_cache = None;
            state.treemap_cache = None;
            state.tree_gen += 1;
            state.expanded.clear();
        }
        self.result_view.set_scan_progress(false, None);
        self.rebuild_rows();
        self.show_chart_page();
        self.select_largest_child();
        self.toast(&format!(
            "Restored scan from cache ({})",
            format_scanned_at(scanned_at)
        ));
        // Recents parity with a completed scan (docs/03 §6) — except mount
        // roots and the home folder, as in `Finished` (no duplicate rows) —
        // deferred off the restore path like the settle housekeeping there.
        let ui = self.clone();
        let path = path.to_path_buf();
        glib::idle_add_local(move || {
            let is_permanent_row = path == glib::home_dir()
                || crate::mounts::list_mounts().iter().any(|m| m.root == path);
            if !is_permanent_row {
                ui.settings.borrow_mut().push_recent(&path.display().to_string());
            }
            ui.refresh_start_page();
            glib::ControlFlow::Break // one-shot housekeeping
        });
    }

    /// Spin up the scanner and its drain loop, rebuilding the tree from
    /// scratch: the spinner page shows while the worker streams updates.
    fn start_scan(self: &Rc<Self>, path: &Path) {
        {
            let mut state = self.state.borrow_mut();
            state.current = Some(path.to_path_buf());
            state.current_folder = path.to_path_buf();
            state.tree = None;
            state.known.clear();
            state.expanded.clear();
            state.pending_autoselect = true;
            state.scan_entries = 0;
            state.scan_bytes = 0;
            // Stale projections from the previous tree are gone.
            state.rings_cache = None;
            state.treemap_cache = None;
        }

        self.result_view.show_page("spinner");
        self.result_view.set_scan_progress(true, Some("Scanning…"));
        self.result_view.set_rows(&[]);

        let ignored = self.settings.borrow().ignored.clone();
        let recursive = self.settings.borrow().recursive_mounts;
        let cache = self.settings.borrow().cache_scans;
        let (handle, receiver) = crate::scanner::spawn_ext(path, recursive, ignored, cache);
        self.state.borrow_mut().scan = Some(handle);

        // Drain scanner updates on the main loop (worker → UI bridge).
        //
        // Scheduled at idle priority: glib dispatches input and frame-clock
        // redraws first, so the pump only ever consumes the main loop's
        // spare time, and the bounded per-dispatch budget keeps event
        // latency at ~10 ms even while a home-folder scan queues hundreds
        // of thousands of updates. Draining everything inside one callback
        // starved the frame clock ("Not Responding"); a fixed 6-of-16 ms
        // timeout instead left ~75% of the loop unused and stretched the
        // same work over several seconds. When the queue runs dry the pump
        // hands off to a cheap poll that re-arms it — no spinning while
        // idle. Repaints throttle in `repaint_rows`.
        let queue: Rc<RefCell<Option<std::sync::mpsc::Receiver<ScanUpdate>>>> =
            Rc::new(RefCell::new(Some(receiver)));
        arm_idle(self, &queue);
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
                        state.known.clear();
                    }
                    // Progress counters (user-requested scan progress bar).
                    state.scan_entries += entries.len() as u64;
                    state.scan_bytes += entries.iter().map(|e| e.own_size).sum::<u64>();
                    // Cache refresh: a listing for an already-measured
                    // directory can only happen when the tree came from the
                    // snapshot cache — its children are stale. Drop them
                    // (and their known-paths) so the fresh listing replaces
                    // them; entries deleted since the snapshot disappear.
                    let parents: HashSet<&Path> = entries
                        .iter()
                        .filter_map(|e| e.path.parent())
                        .collect();
                    let WindowState { tree, known, .. } = &mut *state;
                    if let Some(tree) = tree.as_mut() {
                        for parent in parents {
                            if let Some(node) = tree.find_mut(parent) {
                                if node.is_dir && node.measured {
                                    let mut subtree = HashSet::new();
                                    node.collect_paths(&mut subtree);
                                    for p in subtree {
                                        known.remove(&p);
                                    }
                                    node.clear_children();
                                    // Reset to a fresh-scan look: own data
                                    // only, unmeasured (amber) until this
                                    // folder's new DirMeasured arrives —
                                    // finalize_from_children would wrongly
                                    // flag the partial numbers as final.
                                    node.size = node.own_size;
                                    node.items = 1;
                                    node.measured = false;
                                }
                            }
                        }
                    }
                    // Merge entries under their own parents (shared with the
                    // protocol tests). The batch is consumed: node names move
                    // into the tree instead of being cloned per record.
                    let WindowState { tree, known, rows_dirty, tree_gen, .. } = &mut *state;
                    tree.as_mut()
                        .expect("tree was just created")
                        .merge_entries(entries, known);
                    // Repaint on the throttle in repaint_rows, not per event —
                    // a home-folder scan would otherwise rebuild O(rows) per
                    // directory and O(subtree) for the charts.
                    *rows_dirty = true;
                    *tree_gen += 1;
                }
            }
            ScanUpdate::DirMeasured { path, size, items, error, mtime } => {
                let mut state = self.state.borrow_mut();
                let Some(tree) = &mut state.tree else { return };
                tree.apply_measured(&path, size, items, error, mtime);
                state.tree_gen += 1;
                let is_current = state.current_folder == path;
                if is_current && state.pending_autoselect {
                    state.select_largest = true;
                    state.pending_autoselect = false;
                }
                state.rows_dirty = true;
                drop(state);
            }
            ScanUpdate::Finished { root } => {
                {
                    let mut state = self.state.borrow_mut();
                    state.scan = None;
                }
                self.result_view.set_scan_progress(false, None);
                // Charts become visible first so the forced repaint below
                // projects them (the projection is skipped while hidden).
                self.show_chart_page();
                self.repaint_rows(true);
                // Recents and the start-page rebuild (procfs/GIO mount
                // queries, a synchronous settings write) run after the
                // final paint — on the settle path they delayed the first
                // complete view of the scan result.
                let ui = self.clone();
                glib::idle_add_local(move || {
                    // Completed scans enter the recents list (docs/03 §6) —
                    // except mount roots and the home folder: both have
                    // their own permanent rows in "This Device", and
                    // recording them produced a mismatched duplicate row
                    // (fourth-round user report: "nvme0n1p2" twice, once as
                    // a device, once as a bare folder row).
                    let is_permanent_row = root == glib::home_dir()
                        || crate::mounts::list_mounts().iter().any(|m| m.root == root);
                    if !is_permanent_row {
                        ui.settings.borrow_mut().push_recent(&root.display().to_string());
                    }
                    ui.refresh_start_page();
                    glib::ControlFlow::Break // one-shot housekeeping
                });
            }
            ScanUpdate::Failed { path } => {
                self.result_view.set_scan_progress(false, None);
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
            .filter(|c| c.measured || !c.is_dir)
            .max_by_key(|c| c.size)
            .map(|c| c.path.clone());
        drop(state);
        if let Some(path) = largest {
            self.result_view.select_row_by_path(&path);
        }
    }

    /// Rebuild the flat row list for `current_folder` from the tree.
    fn rebuild_rows(self: &Rc<Self>) {
        let rows = {
            let state = self.state.borrow();
            let Some(tree) = &state.tree else { return };
            let Some(current) = tree.find(&state.current_folder) else { return };
            let now = glib::DateTime::now_local()
                .unwrap_or_else(|_| glib::DateTime::from_unix_local(0).unwrap());
            build_rows(current, &state.expanded, state.sort, &now)
        };
        self.result_view.set_rows(&rows);
        self.update_charts();
    }

    /// Throttled repaint while a scan streams in (docs/03 §5 progressive
    /// fill): tree mutations happen per update, but the list is rebuilt at
    /// most ~8×/s and the charts (an O(subtree) projection) at most 2×/s.
    /// `force` (terminal updates, drain end) repaints immediately.
    fn repaint_rows(self: &Rc<Self>, force: bool) {
        let now = std::time::Instant::now();
        let (list_due, chart_due) = {
            let state = self.state.borrow();
            if !state.rows_dirty && !force {
                (false, false)
            } else {
                (
                    force
                        || now.duration_since(state.last_repaint.get())
                            >= std::time::Duration::from_millis(120),
                    force
                        || now.duration_since(state.last_chart.get())
                            >= std::time::Duration::from_millis(500),
                )
            }
        };
        if list_due {
            let rows = {
                let state = self.state.borrow();
                let Some(tree) = &state.tree else { return };
                let Some(current) = tree.find(&state.current_folder) else { return };
                let now = glib::DateTime::now_local()
                    .unwrap_or_else(|_| glib::DateTime::from_unix_local(0).unwrap());
                build_rows(current, &state.expanded, state.sort, &now)
            };
            self.result_view.set_rows(&rows);
            {
                let mut state = self.state.borrow_mut();
                state.rows_dirty = false;
                state.last_repaint.set(now);
            }
            // Live scan progress (user request): pulse + counters while the
            // worker streams updates.
            if self.state.borrow().scan.is_some() {
                let state = self.state.borrow();
                let text = format!(
                    "{} items · {} scanned",
                    state.scan_entries,
                    crate::format::size(state.scan_bytes)
                );
                self.result_view.set_scan_progress(true, Some(&text));
            }
            // Deferred autoselect: the row exists only after the rebuild.
            if self.state.borrow().select_largest {
                self.state.borrow_mut().select_largest = false;
                self.select_largest_child();
            }
        }
        if chart_due {
            // Projecting charts clones the visible subtree — skip it while
            // the spinner page covers them (the whole scan) so the main
            // loop never pays for invisible work; `Finished` shows the
            // chart page before the forced repaint.
            if self.result_view.charts_visible() {
                self.update_charts();
                self.state.borrow_mut().last_chart.set(now);
            }
        }
    }

    /// Rebuild the shared chart state from the tree + selection and push it
    /// to both charts (docs/04 §7–§8).
    ///
    /// The subtree projections are cached against a tree-generation counter:
    /// selecting rows (the most frequent trigger — every click fires the
    /// selection-changed signal) reuses them; only tree edits or navigation
    /// into a different folder re-project. With the chart pane disabled in
    /// Preferences this returns immediately — no projection, no redraws.
    fn update_charts(&self) {
        if !self.result_view.charts_visible() {
            return;
        }
        let (tree_gen, current_folder, treemap_src_path, selected_path) = {
            let state = self.state.borrow();
            let Some(tree) = &state.tree else { return };
            let Some(current) = tree.find(&state.current_folder) else { return };

            // The location row counts as "current folder", not a selection.
            let selected_path = self
                .result_view
                .selected_row()
                .map(|r| r.path().to_string())
                .filter(|p| p != &current.path.display().to_string());

            // Treemap follows the selection: the selected folder's subtree, the
            // current folder for a selected file (it stays highlighted inside it),
            // or the current folder when nothing is selected (docs/04 §8).
            let treemap_src = selected_path
                .as_deref()
                .and_then(|p| tree.find(Path::new(p)))
                .filter(|n| n.is_dir)
                .unwrap_or(current);

            (
                state.tree_gen,
                state.current_folder.clone(),
                treemap_src.path.clone(),
                selected_path,
            )
        };

        let rings_root = self.chart_projection(tree_gen, &current_folder, true);
        let treemap_root = self.chart_projection(tree_gen, &treemap_src_path, false);

        let chart_state = Rc::new(ChartState {
            rings_root,
            treemap_root,
            selected_path,
        });
        self.result_view.rings_chart.set_state(Some(chart_state.clone()));
        self.result_view.treemap_chart.set_state(Some(chart_state));
    }

    /// The cached projection of `folder` (rings or treemap source), rebuilt
    /// only when the tree generation or the folder changed since.
    fn chart_projection(&self, tree_gen: u64, folder: &Path, rings: bool) -> Rc<ChartNode> {
        {
            let state = self.state.borrow();
            let cache = if rings { &state.rings_cache } else { &state.treemap_cache };
            if let Some((g, p, node)) = cache {
                if *g == tree_gen && p.as_path() == folder {
                    return node.clone();
                }
            }
        }
        let projected = {
            let state = self.state.borrow();
            let Some(tree) = &state.tree else {
                return empty_projection(folder);
            };
            let Some(node) = tree.find(folder) else {
                return empty_projection(folder);
            };
            let Some(root_path) = state.current.clone() else {
                return empty_projection(folder);
            };
            let index = tree
                .find(&root_path)
                .map(|root| crate::ui::palette::ColorIndex::new(&sorted_root_children(root)));
            let mut color_cache: std::collections::HashMap<(usize, u32), String> =
                std::collections::HashMap::new();
            project_chart(node, &root_path, index.as_ref(), &mut color_cache, 0)
        };
        let projected = Rc::new(projected);
        {
            let mut state = self.state.borrow_mut();
            let slot = if rings { &mut state.rings_cache } else { &mut state.treemap_cache };
            *slot = Some((tree_gen, folder.to_path_buf(), projected.clone()));
        }
        projected
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
        // The Main Menu lives on the start-page header (docs/01 §1).
        self.menu_btn.set_visible(true);
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

    /// True when the keyboard focus is inside one of our popovers (the item
    /// context menu) — Escape belongs to it then, not to navigation.
    fn focus_in_popover(&self) -> bool {
        gtk::prelude::GtkWindowExt::focus(&self.window)
            .and_then(|w| w.ancestor(gtk::Popover::static_type()))
            .is_some()
    }

    /// Open the item context menu (docs/05 §1) at a position expressed in
    /// `source` widget coordinates.
    fn show_item_menu(&self, path: &str, source: &gtk::Widget, x: f64, y: f64) {
        if let Some(old) = self.item_popover.borrow_mut().take() {
            old.unparent();
        }
        let menu = gio::Menu::new();
        for (label, action) in [
            ("Open Externally", "win.open-externally"),
            ("Copy Path to Clipboard", "win.copy-path"),
            ("Move to Trash", "win.trash-item"),
        ] {
            let item = gio::MenuItem::new(Some(label), None);
            item.set_action_and_target_value(Some(action), Some(&path.to_variant()));
            menu.append_item(&item);
        }
        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_parent(&self.result_view.root);
        if let Some(pt) = source.compute_point(
            &self.result_view.root,
            &gtk::graphene::Point::new(x as f32, y as f32),
        ) {
            popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                pt.x() as i32,
                pt.y() as i32,
                1,
                1,
            )));
        }
        popover.popup();
        *self.item_popover.borrow_mut() = Some(popover);
    }

    /// Launch the item with the system default handler; on failure a
    /// non-blocking toast, no fallback (docs/05 §2).
    fn open_externally(self: &Rc<Self>, path: &str) {
        let uri = gio::File::for_path(path).uri();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            if gio::AppInfo::launch_default_for_uri_future(
                &uri,
                None::<&gio::AppLaunchContext>,
            )
            .await
            .is_err()
            {
                ui.toast("Failed to open file");
            }
        });
    }

    /// Copy the absolute path to the clipboard (docs/05 §3, with the
    /// documented "Copied to clipboard" toast).
    fn copy_path(&self, path: &str) {
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(path);
            self.toast("Copied to clipboard");
        }
    }

    /// Ask for confirmation, then trash (docs/06 R1–R4). Nothing happens
    /// unless the user explicitly confirms.
    fn begin_trash(self: &Rc<Self>, path: &str) {
        let p = PathBuf::from(path);
        let is_dir = std::fs::metadata(&p).map(|m| m.is_dir()).unwrap_or(true);
        let ui = self.clone();
        let p_for_confirm = p.clone();
        crate::ui::confirm::confirm_trash(&self.window, &p, is_dir, move || {
            ui.do_trash(&p_for_confirm);
        });
    }

    /// Move the item to the Trash and refresh immediately (docs/06 R5, R6,
    /// R8). Failure leaves the item in place; there is no deletion fallback.
    fn do_trash(self: &Rc<Self>, path: &Path) {
        let is_dir = std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false);
        let record = crate::trash::undo_record(path);
        if let Err(err) = crate::trash::trash(path) {
            let kind = if is_dir { "folder" } else { "file" };
            self.toast(&format!("Failed to move {} to Trash", kind));
            eprintln!("arbor: trash failed for {}: {err}", path.display());
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            if let Some(tree) = state.tree.as_mut() {
                tree.detach(path);
            }
            state.tree_gen += 1; // the projections no longer match the tree
            state.undo = Some(record);
        }
        // R8: row removed and totals recomputed immediately (optimistic —
        // exact figures return with the next rescan).
        self.rebuild_rows();
        let toast = adw::Toast::new("Moved to Trash");
        toast.set_timeout(5);
        toast.set_priority(adw::ToastPriority::High);
        toast.set_button_label(Some("Undo"));
        toast.set_action_name(Some("win.undo-trash"));
        self.toast_overlay.add_toast(toast);
    }

    /// Undo the last trash operation (docs/06 R6). Conservative restore:
    /// never overwrites, never touches other trash entries.
    fn undo_trash(self: &Rc<Self>) {
        let record = self.state.borrow().undo.clone();
        let Some(record) = record else { return };
        let result = crate::trash::find_entry(&record)
            .ok_or_else(|| "entry not found in Trash".to_string())
            .and_then(|entry| crate::trash::restore(&entry, &record.original));
        match result {
            Ok(()) => {
                self.state.borrow_mut().undo = None; // one-shot
                self.toast("Restored from Trash");
                // Re-measure so the restored item is shown (the displayed
                // totals were optimistic after the trash operation).
                if let Some(current) = self.current_location() {
                    self.open_location(&current);
                }
            }
            Err(reason) => {
                self.toast(&format!("Failed to restore from Trash: {reason}"));
            }
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

    /// Apply the Preferences "Show chart" switch: hide/show the chart pane
    /// and let the result view pick the change up immediately. When the pane
    /// comes back mid-result, force one repaint so the charts project.
    pub fn apply_chart_visibility(self: &Rc<Self>) {
        let on = self.settings.borrow().show_chart;
        self.result_view.set_chart_enabled(on);
        if on && self.current_location().is_some() && self.state.borrow().scan.is_none() {
            self.show_chart_page();
            self.repaint_rows(true);
        }
    }
}

use crate::models::EntryMeta;

/// Build the flat row list: pinned location row + sorted children, expanded
/// folders inline (docs/04 §2, docs/02 §3).
/// Local time of a cached scan for the restore toast: time-of-day for
/// today's scans, full date+time otherwise.
fn format_scanned_at(secs: u64) -> String {
    let Ok(dt) = glib::DateTime::from_unix_local(secs as i64) else {
        return "unknown time".into();
    };
    let now = glib::DateTime::now_local().unwrap_or_else(|_| dt.clone());
    let today = dt.year() == now.year()
        && dt.month() == now.month()
        && dt.day_of_month() == now.day_of_month();
    dt.format(if today { "%H:%M" } else { "%Y-%m-%d %H:%M" })
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unknown time".into())
}

/// What the drain tick wants next from the scheduler.
enum DrainFlow {
    /// Keep this source attached (work remains).
    Stay,
    /// Queue ran dry: drop this source, poll instead.
    Poll,
    /// Terminal update seen; stop scheduling.
    Done,
}

type ScanQueue = Rc<RefCell<Option<std::sync::mpsc::Receiver<ScanUpdate>>>>;

/// Process scan updates for up to `DRAIN_BUDGET_MS` of wall time.
fn drain_tick(ui: &Rc<ArborUi>, queue: &ScanQueue) -> DrainFlow {
    const DRAIN_BUDGET_MS: u64 = 10;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(DRAIN_BUDGET_MS);
    loop {
        // Scope the RefCell guard so the queue can be cleared below.
        let update = {
            let guard = queue.borrow();
            let Some(receiver) = guard.as_ref() else {
                return DrainFlow::Done;
            };
            match receiver.try_recv() {
                Ok(update) => update,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    drop(guard);
                    ui.repaint_rows(false);
                    return DrainFlow::Poll;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    drop(guard);
                    ui.repaint_rows(true);
                    *queue.borrow_mut() = None;
                    return DrainFlow::Done;
                }
            }
        };
        let terminal = matches!(
            update,
            ScanUpdate::Finished { .. }
                | ScanUpdate::Failed { .. }
                | ScanUpdate::Cancelled
        );
        ui.handle_scan_update(update);
        if terminal {
            ui.repaint_rows(true);
            *queue.borrow_mut() = None;
            return DrainFlow::Done;
        }
        if std::time::Instant::now() >= deadline {
            ui.repaint_rows(false);
            return DrainFlow::Stay;
        }
    }
}

/// Run the drain on the idle source (runs after input/redraw each
/// iteration — full spare duty cycle without starving the frame clock).
fn arm_idle(ui: &Rc<ArborUi>, queue: &ScanQueue) {
    let (ui, queue) = (ui.clone(), queue.clone());
    glib::idle_add_local(move || match drain_tick(&ui, &queue) {
        DrainFlow::Stay => glib::ControlFlow::Continue,
        DrainFlow::Poll => {
            arm_poll(&ui, &queue);
            glib::ControlFlow::Break // Break removes the idle source
        }
        DrainFlow::Done => glib::ControlFlow::Break,
    });
}

/// Queue is empty: poll every 16 ms until work reappears, then hand back
/// to the idle pump. Costs nothing while nothing arrives.
fn arm_poll(ui: &Rc<ArborUi>, queue: &ScanQueue) {
    let (ui, queue) = (ui.clone(), queue.clone());
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        match drain_tick(&ui, &queue) {
            DrainFlow::Stay => {
                arm_idle(&ui, &queue);
                glib::ControlFlow::Break // Break removes the poll source
            }
            DrainFlow::Poll => glib::ControlFlow::Continue,
            DrainFlow::Done => glib::ControlFlow::Break,
        }
    });
}

fn build_rows(
    current: &DisplayNode,
    expanded: &HashSet<PathBuf>,
    sort: (SortKey, bool),
    now: &glib::DateTime,
) -> Vec<ScanRow> {
    let total = current.size.max(1);
    let mut rows = Vec::new();

    // Pinned location row with up-arrow (docs/02 §3).
    let loc = ScanRow::new(current.path.clone());
    fill_row(&loc, current, total, true, 0, false, now);
    rows.push(loc);

    emit_children(current, expanded, total, sort, 1, &mut rows, now);
    rows
}

fn emit_children(
    node: &DisplayNode,
    expanded: &HashSet<PathBuf>,
    total: u64,
    sort: (SortKey, bool),
    depth: u32,
    rows: &mut Vec<ScanRow>,
    now: &glib::DateTime,
) {
    // Siblings sort by the active column; expanded children stay nested
    // under their parent (hierarchy beats flat sorting, docs/04 §3).
    // Sort keys are computed once per child (decorate-sort-undecorate) —
    // the comparator used to allocate two lowercase Strings per
    // comparison, which dominated wide-folder rebuilds.
    let (key, descending) = sort;
    let mut children: Vec<(&DisplayNode, i64, String)> = node
        .children
        .iter()
        .map(|c| {
            (
                c,
                match key {
                    SortKey::Name => 0,
                    SortKey::Size => c.size as i64,
                    SortKey::Items => c.items as i64,
                    SortKey::Modified => c.mtime.unwrap_or(0),
                },
                c.name.to_lowercase(),
            )
        })
        .collect();
    children.sort_unstable_by(|a, b| {
        let ord = match key {
            SortKey::Name => a.2.cmp(&b.2),
            SortKey::Size | SortKey::Items | SortKey::Modified => a.1.cmp(&b.1),
        };
        let ord = if descending { ord.reverse() } else { ord };
        // Stable tie-break: case-insensitive name ascending.
        ord.then_with(|| a.2.cmp(&b.2))
    });
    for (child, _, _) in children {
        let row = ScanRow::new(child.path.clone());
        fill_row(&row, child, total, false, depth, expanded.contains(&child.path), now);
        rows.push(row);
        if child.is_dir && expanded.contains(&child.path) {
            emit_children(child, expanded, total, sort, depth + 1, rows, now);
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
    now: &glib::DateTime,
) {
    row.set_name(node.name.as_str());
    row.set_size_str(crate::format::size(node.size).as_str());
    row.set_items_str(crate::format::items(node.items).as_str());
    // `now` is shared per rebuild — a fresh glib::DateTime per row made
    // streaming repaints of wide folders pay thousands of allocations.
    row.set_modified_str(
        node.mtime
            .map(|m| crate::format::relative_day(m, now))
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
    // Names render plain (like the observed reference); a still-unmeasured
    // folder is highlighted amber until its aggregate is final, and
    // permission-error rows stay red (docs/10 §5).
    row.set_measured(node.measured || !node.is_dir);
}

/// The scan root's children as name-sorted paths — the stable palette index
/// shared by list rows and chart segments.
fn sorted_root_children(root: &DisplayNode) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = root.children.iter().map(|c| c.path.clone()).collect();
    v.sort_by(|a, b| {
        let an = a
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let bn = b
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        an.cmp(&bn)
    });
    v
}

/// Snapshot a `DisplayNode` subtree into a colored `ChartNode` — depth-capped
/// at `CHART_MAX_DEPTH` (deeper nodes are never drawn), with children's
/// angular segments precomputed and colors interned per (hue index, depth).
fn project_chart(
    node: &DisplayNode,
    root_path: &Path,
    index: Option<&crate::ui::palette::ColorIndex>,
    color_cache: &mut std::collections::HashMap<(usize, u32), String>,
    depth: usize,
) -> ChartNode {
    let color = if node.error {
        crate::ui::palette::ERROR_COLOR.to_string()
    } else if node.path == root_path {
        String::new() // the scan root itself: neutral
    } else {
        match index.and_then(|i| i.top_index_of(&node.path, root_path)) {
            None => String::new(),
            Some(idx) => {
                let rel_depth = crate::ui::palette::ColorIndex::rel_depth(&node.path, root_path) as u32;
                color_cache
                    .entry((idx, rel_depth))
                    .or_insert_with(|| crate::ui::palette::color_for_indexed(idx, f64::from(rel_depth)))
                    .clone()
            }
        }
    };
    let (children, segs) = if depth >= super::charts::CHART_MAX_DEPTH || node.children.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        // Chart children render name-sorted (stable palette order, docs/10 §5).
        let mut ordered: Vec<(&DisplayNode, String)> = node
            .children
            .iter()
            .map(|c| (c, c.name.to_lowercase()))
            .collect();
        ordered.sort_by(|a, b| a.1.cmp(&b.1));
        let children: Vec<ChartNode> = ordered
            .into_iter()
            .map(|(c, _)| project_chart(c, root_path, index, color_cache, depth + 1))
            .collect();
        let weights: Vec<f64> = children.iter().map(|c| c.size as f64).collect();
        let segs = crate::squarify::segments(&weights);
        (children, segs)
    };
    ChartNode {
        name: node.name.clone(),
        size: node.size,
        color,
        is_dir: node.is_dir,
        path: node.path.display().to_string(),
        children,
        segs,
    }
}

/// Neutral empty chart node for degenerate lookups (folder not in the tree).
fn empty_projection(folder: &Path) -> Rc<ChartNode> {
    Rc::new(ChartNode {
        name: String::new(),
        size: 0,
        color: String::new(),
        is_dir: true,
        path: folder.display().to_string(),
        children: Vec::new(),
        segs: Vec::new(),
    })
}

fn load_settings() -> Settings {
    Settings::default_path()
        .map(|p| Settings::load_from(&p))
        .unwrap_or_default()
}

pub(crate) fn save_settings(s: &Settings) {
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
        /* Amber highlight for folders whose subtree is still being measured
           (observed reference behavior): name and badge fill turn amber and
           settle to the plain/accent look once the aggregate is final. */
        @define-color arbor_amber #e5a50a;
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
        /* Percent badge: one dark pill whose fill (a full-size ProgressBar)
           represents the share, with the percentage centered on top
           (docs/10 §3 row anatomy). */
        .percent-badge {
            border-radius: 9999px;
            background-color: alpha(currentColor, 0.14);
        }
        .percent-badge progressbar {
            min-width: 64px;
        }
        .percent-badge progressbar > trough {
            min-width: 64px;
        }
        .percent-badge progressbar > trough {
            min-height: 17px;
            border-radius: 9999px;
            background: transparent;
        }
        .percent-badge progressbar > trough > progress {
            min-height: 17px;
            border-radius: 9999px;
            background-color: @accent_color;
        }
        .percent-badge.unmeasured progressbar > trough > progress {
            background-color: @arbor_amber;
        }
        .percent-badge > label.mini {
            font-size: 0.82em;
            font-weight: bold;
        }
        label.unmeasured {
            color: @arbor_amber;
        }
        /* Denser item list (user request): the default button chrome and row
           padding made every row ~33 logical px tall — mostly air. */
        columnview row {
            min-height: 0px;
            padding-top: 0px;
            padding-bottom: 0px;
        }
        button.row-expander {
            min-height: 0px;
            padding-top: 0px;
            padding-bottom: 0px;
            padding-left: 2px;
            padding-right: 2px;
        }
        "#,
    );
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
