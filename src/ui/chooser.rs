//! "Select Folder" chooser with the "Recursively analyze mount points"
//! checkbox (docs/03 §1, decision D1).
//!
//! GTK4's `GtkFileChooser` no longer supports extra widgets, so the dialog is
//! assembled from a `FileChooserWidget` inside a modal window: file list and
//! sidebar (Recent, Home, …, Ctrl+L location entry with completion) come from
//! the standard widget; the checkbox and Cancel/Open buttons sit in a bottom
//! bar, matching the reference layout (docs/10 §1 #9).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{glib, Align, Orientation};

/// What the chooser reported back to the caller.
pub enum Outcome {
    /// A valid folder plus the checkbox state at confirmation time.
    Chosen(PathBuf, bool),
    /// A confirmed location that is not a valid folder (docs/03 §1: the
    /// chooser closes and the main window shows the invalid-path toast).
    Invalid(String),
    /// Cancelled or dismissed — nothing happens.
    Cancelled,
}

type Callback = Box<dyn FnOnce(Outcome)>;

thread_local! {
    /// One pending outcome callback per open chooser, keyed by dialog pointer.
    /// Guarantees the callback fires exactly once, whichever path closes it.
    static PENDING: RefCell<HashMap<usize, Callback>> = RefCell::new(HashMap::new());
}

fn key(dialog: &gtk::Window) -> usize {
    dialog.as_ptr() as *const () as usize
}

/// Present a folder chooser; `recursive` presets the mount-points checkbox.
/// `on_outcome` runs on the main thread when the dialog finishes.
pub fn choose_folder(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    recursive: bool,
    on_outcome: impl FnOnce(Outcome) + 'static,
) {
    let dialog = gtk::Window::builder()
        .title(title)
        .modal(true)
        .default_width(900)
        .default_height(600)
        .transient_for(parent)
        .build();

    let content = gtk::Box::new(Orientation::Vertical, 0);
    dialog.set_child(Some(&content));

    // GtkFileChooserWidget is deprecated since GTK 4.10 in favor of
    // GtkFileDialog, which cannot host the "Recursively analyze mount
    // points" checkbox required by docs/03 §1; the widget is the only way to
    // reproduce the reference chooser (sidebar + Ctrl+L entry + checkbox).
    #[allow(deprecated)]
    let chooser = gtk::FileChooserWidget::new(gtk::FileChooserAction::SelectFolder);
    chooser.set_vexpand(true);
    content.append(&chooser);

    content.append(&gtk::Separator::new(Orientation::Horizontal));

    let bottom = gtk::Box::new(Orientation::Horizontal, 8);
    bottom.set_margin_top(12);
    bottom.set_margin_bottom(12);
    bottom.set_margin_start(12);
    bottom.set_margin_end(12);
    content.append(&bottom);

    let check = gtk::CheckButton::with_label("Recursively analyze mount points");
    check.set_active(recursive);
    check.set_valign(Align::Center);
    bottom.append(&check);

    bottom.append(&{
        let spacer = gtk::Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        spacer
    });

    let cancel = gtk::Button::with_label("Cancel");
    cancel.set_valign(Align::Center);
    bottom.append(&cancel);

    let open = gtk::Button::with_label("Open");
    open.add_css_class("suggested-action");
    open.set_valign(Align::Center);
    bottom.append(&open);

    PENDING.with(|p| {
        p.borrow_mut()
            .insert(key(&dialog), Box::new(on_outcome));
    });

    cancel.connect_clicked(glib::clone!(
        #[strong]
        dialog,
        move |_| finish(&dialog, Outcome::Cancelled),
    ));

    // Dismissing the window (✕ / Escape) cancels — no side effects.
    dialog.connect_close_request(glib::clone!(
        #[strong]
        dialog,
        move |_| {
            finish(&dialog, Outcome::Cancelled);
            glib::Propagation::Proceed
        },
    ));

    open.connect_clicked(glib::clone!(
        #[strong]
        dialog,
        #[strong]
        check,
        move |_| {
            #[allow(deprecated)]
            let file = chooser.file();
            match file.and_then(|f| f.path()) {
                Some(path) if path.is_dir() => {
                    finish(&dialog, Outcome::Chosen(path, check.is_active()));
                }
                Some(path) => finish(&dialog, Outcome::Invalid(path.display().to_string())),
                None => finish(&dialog, Outcome::Cancelled),
            }
        },
    ));

    dialog.present();
}

/// Destroy the dialog and deliver exactly one outcome.
fn finish(dialog: &gtk::Window, outcome: Outcome) {
    let k = key(dialog);
    dialog.destroy();
    PENDING.with(|p| {
        if let Some(cb) = p.borrow_mut().remove(&k) {
            cb(outcome);
        }
    });
}