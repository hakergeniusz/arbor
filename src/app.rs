//! Application assembly: actions, menu model, accelerators, single-instance
//! CLI handling (docs/01 §4, §9; docs/03 §1).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::ui::window::ArborUi;
use crate::ui::{about, chooser, preferences, shortcuts};

pub const APP_ID: &str = "io.github.hakergeniusz.Arbor";
pub const HELP_URL: &str = "https://github.com/hakergeniusz/arbor#readme";

thread_local! {
    /// The (single) main window UI. GTK runs on one thread, so a thread-local
    /// registry is sufficient and keeps closures simple.
    static UI: RefCell<Option<Rc<ArborUi>>> = const { RefCell::new(None) };
}

pub fn run() -> glib::ExitCode {
    // Register the embedded gresource (application icon) before any window
    // exists, and make it resolvable by the icon theme.
    gio::resources_register_include!("arbor.gresource").expect("embedded gresource");
    if let Some(display) = gtk::gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);
        theme.add_resource_path("/io/github/hakergeniusz/Arbor/icons");
    }

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(|app| {
        ensure_window(app);
    });

    // `arbor <folder>` — navigates the existing window when already running
    // (single-instance behavior, docs/01 §9 / S06).
    app.connect_open(|app, files, _hint| {
        ensure_window(app);
        UI.with(|ui| {
            let guard = ui.borrow();
            let Some(ui) = guard.as_ref() else { return };
            for file in files {
                match file.path() {
                    Some(path) if path.is_dir() => ui.open_location(&path),
                    Some(path) => {
                        ui.toast(&format!(
                            "“{}” is not a valid folder",
                            path.display()
                        ));
                    }
                    None => {}
                }
            }
        });
    });

    app.run()
}

fn ensure_window(app: &adw::Application) {
    let already = UI.with(|ui| ui.borrow().is_some());
    if !already {
        let ui = ArborUi::build(app);
        register_actions(app, &ui);
        UI.with(|slot| *slot.borrow_mut() = Some(ui));
    }
    UI.with(|slot| {
        if let Some(ui) = slot.borrow().as_ref() {
            ui.window.present();
        }
    });
}

fn register_actions(app: &adw::Application, ui: &Rc<ArborUi>) {
    let ui_scan = ui.clone();
    let ui_prefs = ui.clone();
    let ui_shortcuts = ui.clone();
    let ui_about = ui.clone();

    app.add_action_entries([
        // Scan Folder… (S09)
        gio::ActionEntry::builder("scan-folder")
            .activate(move |_: &adw::Application, _, _| {
                chooser::choose_folder(
                    &ui_scan.window,
                    "Select Folder",
                    ui_scan.settings.borrow().recursive_mounts,
                    |outcome| match outcome {
                        chooser::Outcome::Chosen(path, recursive) => {
                            UI.with(|slot| {
                                let slot = slot.borrow();
                                if let Some(ui) = slot.as_ref() {
                                    ui.settings.borrow_mut().recursive_mounts = recursive;
                                    ui.refresh_start_page();
                                    ui.open_location(&path);
                                }
                            });
                        }
                        chooser::Outcome::Invalid(path) => {
                            UI.with(|slot| {
                                let slot = slot.borrow();
                                if let Some(ui) = slot.as_ref() {
                                    ui.toast(&format!("“{path}” is not a valid folder"));
                                }
                            });
                        }
                        chooser::Outcome::Cancelled => {}
                    },
                );
            })
            .build(),
        // Clear Recent List (S25) — immediate, no confirmation.
        gio::ActionEntry::builder("clear-recents")
            .activate(|_: &adw::Application, _, _| {
                UI.with(|slot| {
                    let slot = slot.borrow();
                    if let Some(ui) = slot.as_ref() {
                        ui.settings.borrow_mut().clear_recents();
                        ui.refresh_start_page();
                    }
                });
            })
            .build(),
        // Preferences (S04)
        gio::ActionEntry::builder("preferences")
            .activate(move |_: &adw::Application, _, _| {
                preferences::present(&ui_prefs);
            })
            .build(),
        // Keyboard Shortcuts (S03)
        gio::ActionEntry::builder("shortcuts")
            .activate(move |_: &adw::Application, _, _| {
                shortcuts::present(&ui_shortcuts.window);
            })
            .build(),
        // Help (F1) — opens the project documentation (decision D10).
        gio::ActionEntry::builder("help")
            .activate(|_: &adw::Application, _, _| {
                open_url(HELP_URL);
            })
            .build(),
        // About (S08)
        gio::ActionEntry::builder("about")
            .activate(move |_: &adw::Application, _, _| {
                about::present(&ui_about.window);
            })
            .build(),
        // Rescan (S23; scanner wiring lands in phase 3).
        gio::ActionEntry::builder("rescan")
            .activate(|_: &adw::Application, _, _| {
                UI.with(|slot| {
                    let slot = slot.borrow();
                    if let Some(ui) = slot.as_ref() {
                        if let Some(path) = ui.current_location() {
                            ui.open_location(&path);
                        }
                    }
                });
            })
            .build(),
        // Quit
        gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| {
                app.quit();
            })
            .build(),
    ]);

    // Accelerators (docs/01 §4, docs/08 §2).
    app.set_accels_for_action("app.scan-folder", &["<Ctrl>O"]);
    app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
    app.set_accels_for_action("app.shortcuts", &["<Ctrl>question"]);
    app.set_accels_for_action("app.help", &["F1"]);
    app.set_accels_for_action("app.rescan", &["<Ctrl>R"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
}

/// The main menu model (docs/01 §4, S02).
pub fn main_menu_model() -> gio::Menu {
    let menu = gio::Menu::new();

    let scan = gio::Menu::new();
    scan.append(Some("Scan Folder…"), Some("app.scan-folder"));
    menu.append_section(None, &scan);

    let recents = gio::Menu::new();
    recents.append(Some("Clear Recent List"), Some("app.clear-recents"));
    menu.append_section(None, &recents);

    let general = gio::Menu::new();
    general.append(Some("Preferences"), Some("app.preferences"));
    general.append(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
    general.append(Some("Help"), Some("app.help"));
    general.append(Some("About Arbor"), Some("app.about"));
    menu.append_section(None, &general);

    menu
}

/// Open a URL with the platform default handler; failure surfaces as a toast
/// (matches the Open-Externally failure pattern, docs/05 §2).
fn open_url(url: &str) {
    let url = url.to_string();
    glib::spawn_future_local(async move {
        let result =
            gio::AppInfo::launch_default_for_uri_future(&url, None::<&gio::AppLaunchContext>)
                .await;
        if let Err(err) = result {
            UI.with(|slot| {
                let slot = slot.borrow();
                if let Some(ui) = slot.as_ref() {
                    ui.toast("Failed to open help");
                }
            });
            eprintln!("arbor: failed to open {url}: {err}");
        }
    });
}
