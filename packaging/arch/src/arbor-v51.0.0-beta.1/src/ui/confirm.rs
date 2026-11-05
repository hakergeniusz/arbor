//! Trash confirmation dialog (docs/06 §2–§3, MANDATORY).
//!
//! R2 exact structure; R3 default action is Cancel (Enter and Escape both
//! cancel); R4 no side effects unless the user explicitly activates
//! "Move to Trash".

use std::path::Path;

use adw::prelude::*;
use gtk::{Align, Orientation};

/// Ask for confirmation before moving `path` to the Trash.
///
/// `on_confirm` runs only when the user explicitly activates the
/// "Move to Trash" response; every other dismissal path (Cancel click,
/// Enter, Escape, dialog close) performs nothing.
pub fn confirm_trash(
    window: &adw::ApplicationWindow,
    path: &Path,
    is_dir: bool,
    on_confirm: impl FnOnce() + 'static,
) {
    let kind = if is_dir { "folder" } else { "file" };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let dialog = adw::AlertDialog::builder()
        .heading(format!("Move {} to Trash?", kind))
        .build();

    // R2/R9: show the item exactly as it will be trashed.
    let details = gtk::Box::new(Orientation::Vertical, 12);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(12);
    grid.set_row_spacing(4);
    let name_key = gtk::Label::new(Some(if is_dir { "Folder:" } else { "File:" }));
    let path_key = gtk::Label::new(Some("Path:"));
    for key in [&name_key, &path_key] {
        key.set_halign(Align::Start);
        key.add_css_class("dim-label");
    }
    let name_val = gtk::Label::new(Some(&name));
    let path_val = gtk::Label::new(Some(&path.display().to_string()));
    for val in [&name_val, &path_val] {
        val.set_halign(Align::Start);
        val.set_selectable(true);
        // R2/A6: the full path must be visible — wrap, never truncate.
        val.set_wrap(true);
    }
    grid.attach(&name_key, 0, 0, 1, 1);
    grid.attach(&name_val, 1, 0, 1, 1);
    grid.attach(&path_key, 0, 1, 1, 1);
    grid.attach(&path_val, 1, 1, 1, 1);
    details.append(&grid);
    let explanation = gtk::Label::new(Some(&if is_dir {
        "The folder and all of its contents will be moved to the Trash. \
         You can restore it from the Trash later."
            .to_string()
    } else {
        "The file will be moved to the Trash. You can restore it from the \
         Trash later."
            .to_string()
    }));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    explanation.set_halign(Align::Fill);
    details.append(&explanation);
    dialog.set_extra_child(Some(&details));

    // R3: Cancel is the default; the destructive button is never focused
    // and never triggered by Enter. Escape maps to the close response,
    // which is also Cancel.
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("trash", "Move to Trash");
    dialog.set_response_appearance("trash", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    // R4: only the explicit "trash" response may start the operation.
    let confirm = std::cell::RefCell::new(Some(on_confirm));
    dialog.connect_response(None, move |_, response| {
        if response == "trash" {
            if let Some(cb) = confirm.borrow_mut().take() {
                cb();
            }
        }
    });
    dialog.present(Some(window));
}
