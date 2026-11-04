//! Main application window (docs/01).
//!
//! Owns the header bar (Back / location pill with breadcrumb / Rescan), the
//! start-page ⇄ result-view stack, the toast overlay and the shared UI state.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Orientation};

use adw::prelude::*;

use super::result_view::ResultView;
use super::start_page::StartPage;
use super::{ChartMode, StartLocation};
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
    /// Breadcrumb paths parallel to the pill segment buttons.
    breadcrumb: RefCell<Vec<PathBuf>>,
}

struct WindowState {
    mode: ChartMode,
    /// The location currently shown in the result view (None → start page).
    current: Option<PathBuf>,
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
            }),
            breadcrumb: RefCell::new(Vec::new()),
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

        // Rescan re-scans the current location (S23; wired to the scanner in
        // a later phase).
        {
            let ui = self.clone();
            self.rescan_btn.connect_clicked(move |_| {
                let current = ui.state.borrow().current.clone();
                if let Some(path) = current {
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

    /// Open (and later scan) a folder, switching to the result view.
    pub fn open_location(self: &Rc<Self>, path: &std::path::Path) {
        if !path.is_dir() {
            self.toast(&format!("“{}” is not a valid folder", path.display()));
            return;
        }
        let display = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.state.borrow_mut().current = Some(path.to_path_buf());

        // Header state (docs/01 §3).
        self.back_btn.set_visible(true);
        self.rescan_btn.set_visible(true);
        self.title_stack().set_visible_child_name("pill");
        self.pill_icon.set_icon_name(Some(match path == glib::home_dir() {
            true => "user-home-symbolic",
            false => "drive-harddisk-symbolic",
        }));
        self.set_breadcrumb(&[path.to_path_buf()]);

        self.window.set_title(Some(&display));
        self.stack.set_visible_child_name("result");
        self.result_view.show_page("spinner");
        // Phase 3 replaces this stub with the real scan pipeline.
    }

    /// Return to the start page (Back/Escape).
    pub fn show_start(self: &Rc<Self>) {
        self.state.borrow_mut().current = None;
        self.back_btn.set_visible(false);
        self.rescan_btn.set_visible(false);
        self.title_stack().set_visible_child_name("title");
        self.stack.set_visible_child_name("start");
        self.window.set_title(Some("Devices & Locations"));
    }

    fn title_stack(&self) -> gtk::Stack {
        self.pill
            .ancestor(gtk::Stack::static_type())
            .and_downcast::<gtk::Stack>()
            .expect("pill is inside the title stack")
    }

    /// Rebuild the clickable breadcrumb segments for `chain` (root..=current).
    pub fn set_breadcrumb(self: &Rc<Self>, chain: &[PathBuf]) {
        while let Some(child) = self.pill_segments.first_child() {
            self.pill_segments.remove(&child);
        }
        *self.breadcrumb.borrow_mut() = chain.to_vec();
        for (i, seg) in chain.iter().enumerate() {
            if i > 0 {
                let slash = gtk::Label::new(Some("/"));
                slash.add_css_class("dim-label");
                self.pill_segments.append(&slash);
            }
            let name = seg
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| seg.display().to_string());
            let btn = gtk::Button::with_label(&name);
            btn.add_css_class("flat");
            let seg = seg.clone();
            let ui = self.clone();
            btn.connect_clicked(move |_| {
                // Clicking a breadcrumb segment navigates to that ancestor
                // (docs/02 §3, S16).
                ui.open_location(&seg);
            });
            self.pill_segments.append(&btn);
        }
    }

    fn show_chart_page(&self) {
        // Charts land in phase 4; until then the toggles switch to the
        // spinner placeholder only if a scan is running.
        let scanning = self.chart_is_scanning();
        if !scanning {
            self.result_view.show_page("rings");
        }
    }

    fn chart_is_scanning(&self) -> bool {
        self.result_view.chart_stack.visible_child_name()
            == Some("spinner".into())
    }

    /// Show a transient toast with an embedded dismiss button, role=alert,
    /// auto-dismissing in ~4 s (docs/07 §7).
    pub fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        toast.set_priority(adw::ToastPriority::High);
        self.toast_overlay.add_toast(toast);
    }

    /// The location currently shown in the result view, if any.
    pub fn current_location(&self) -> Option<PathBuf> {
        self.state.borrow().current.clone()
    }

    /// Refresh the start page from settings (after recents change).
    pub fn refresh_start_page(&self) {
        self.start_page.rebuild(&self.settings);
        save_settings(&self.settings.borrow());
    }
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
        "#,
    );
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
