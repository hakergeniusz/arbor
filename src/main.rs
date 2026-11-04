//! Arbor — a Linux disk usage analyzer. See where your disk space grows.
//!
//! Entry point: initializes GTK/libadwaita, registers embedded resources and
//! hands control to the application module.

mod app;
mod format;
mod mounts;
mod settings;
mod ui;

fn main() -> gtk::glib::ExitCode {
    app::run()
}
