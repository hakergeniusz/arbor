//! Arbor — a Linux disk usage analyzer. See where your disk space grows.

fn main() -> gtk::glib::ExitCode {
    arbor::app::run()
}
