//! Preferences dialog: "Locations to Ignore" (docs/01 §6, decision D15).

use std::rc::Rc;

use adw::prelude::*;
use gtk::Align;

use crate::ui::window::ArborUi;

/// Present the Preferences dialog over the main window.
pub fn present(ui: &Rc<ArborUi>) {
    let prefs = adw::PreferencesDialog::new();
    prefs.set_title("Preferences");
    prefs.set_search_enabled(false);

    let group = adw::PreferencesGroup::builder()
        .title("Locations to Ignore")
        .description("Ignored locations are excluded from scans.")
        .build();
    let page = adw::PreferencesPage::new();
    page.add(&group);
    prefs.add(&page);

    // Existing ignore entries.
    for path in ui.settings.borrow().ignored.clone() {
        group.add(&ignore_row(ui, &group, path));
    }

    // "+ Add Location" opens a folder chooser (S04).
    let add_btn = gtk::Button::with_label("+ Add Location");
    add_btn.add_css_class("flat");
    add_btn.set_hexpand(true);
    {
        let ui = ui.clone();
        let group = group.clone();
        add_btn.connect_clicked(move |_| {
            let window = ui.window.clone();
            let ui2 = ui.clone();
            let group2 = group.clone();
            super::chooser::choose_folder(
                &window,
                "Select Location to Ignore",
                false,
                move |outcome| {
                    if let super::chooser::Outcome::Chosen(path, _) = outcome {
                        let path_str = path.display().to_string();
                        ui2.settings.borrow_mut().add_ignored(&path_str);
                        group2.add(&ignore_row(&ui2, &group2, path_str));
                        ui2.refresh_start_page();
                    }
                },
            );
        });
    }
    group.add(&add_btn);

    prefs.present(Some(&ui.window));
}

/// One ignore entry row with a remove button.
fn ignore_row(
    ui: &Rc<ArborUi>,
    group: &adw::PreferencesGroup,
    path: String,
) -> adw::ActionRow {
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let row = adw::ActionRow::builder()
        .title(&name)
        .subtitle(&path)
        .build();

    let remove = gtk::Button::from_icon_name("user-trash-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some("Remove"));
    remove.set_valign(Align::Center);
    let ui = ui.clone();
    let group = group.clone();
    let path_owned = path.clone();
    let row_for_remove = row.clone();
    remove.connect_clicked(move |_| {
        ui.settings.borrow_mut().remove_ignored(&path_owned);
        ui.refresh_start_page();
        group.remove(&row_for_remove);
    });
    row.add_suffix(&remove);
    row
}
