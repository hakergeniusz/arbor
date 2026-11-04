//! Start page: "Devices & Locations" (docs/01 §1–2).
//!
//! A centered, max-width card list: fixed Home Folder + mounted filesystem
//! rows (live available/total + usage bar), then recently scanned folders.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align, Orientation};

use adw::prelude::*;

use super::StartLocation;
use crate::mounts::{self, MountInfo};
use crate::settings::Settings;

pub struct StartPage {
    pub root: gtk::ScrolledWindow,
    list: gtk::ListBox,
    /// Locations matching the row order at rebuild time.
    locations: Rc<RefCell<Vec<StartLocation>>>,
    on_activate: RefCell<Option<Rc<dyn Fn(&StartLocation)>>>,
}

impl StartPage {
    pub fn new(settings: &Rc<RefCell<Settings>>) -> Rc<Self> {
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(580);
        clamp.set_tightening_threshold(580);
        let content = gtk::Box::new(Orientation::Vertical, 0);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(12);
        content.set_margin_end(12);
        clamp.set_child(Some(&content));

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&clamp)
            .build();

        let heading = gtk::Label::new(Some("This Device"));
        heading.set_halign(Align::Start);
        heading.add_css_class("title-4");
        heading.set_margin_bottom(12);
        content.append(&heading);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_valign(Align::Start);
        content.append(&list);

        let page = Rc::new(Self {
            root,
            list,
            locations: Rc::new(RefCell::new(Vec::new())),
            on_activate: RefCell::new(None),
        });

        {
            let page_clone = page.clone();
            page.list.connect_row_activated(move |_, row| {
                let idx = row.index().max(0) as usize;
                if let Some(loc) = page_clone.locations.borrow().get(idx).cloned() {
                    // Any row activation opens and scans the location
                    // (recent rows included — decision D9).
                    if let Some(cb) = page_clone.on_activate.borrow().as_ref() {
                        cb(&loc);
                    }
                }
            });
        }

        page.rebuild(settings);
        page
    }

    /// The scrollable root widget of the start page.
    pub fn root(&self) -> &gtk::ScrolledWindow {
        &self.root
    }

    /// Set the activation callback (called by the window after construction).
    pub fn connect_activate(&self, cb: impl Fn(&StartLocation) + 'static) {
        *self.on_activate.borrow_mut() = Some(Rc::new(cb));
    }

    /// Rebuild all rows from settings (used on init and after recents change).
    pub fn rebuild(&self, settings: &Rc<RefCell<Settings>>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let mut locations = Vec::new();

        locations.push(StartLocation::Home);
        self.list.append(&location_row(
            "user-home-symbolic",
            "Home Folder",
            &home_dir_string(),
        ));

        for mount in mounts::list_mounts() {
            locations.push(StartLocation::Mount(mount.clone()));
            self.list.append(&mount_row(&mount));
        }

        for path in settings.borrow().recents.clone() {
            let path = std::path::PathBuf::from(path);
            locations.push(StartLocation::Recent(path.clone()));
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            self.list.append(&location_row(
                "folder-symbolic",
                &name,
                &path.display().to_string(),
            ));
        }

        *self.locations.borrow_mut() = locations;
    }
}

/// A plain location row: disc icon, title, path subtitle, chevron.
fn location_row(icon: &str, title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .activatable(true)
        .build();
    row.add_prefix(&disc_icon(icon));
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.add_css_class("dim-label");
    row.add_suffix(&chevron);
    row
}

/// A filesystem row: title + mount path, right-aligned live space figures and
/// a usage bar spanning the row (docs/01 §1, S01).
fn mount_row(mount: &MountInfo) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);

    let outer = gtk::Box::new(Orientation::Vertical, 6);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(12);
    outer.set_margin_end(12);

    let top = gtk::Box::new(Orientation::Horizontal, 12);
    outer.append(&top);

    top.append(&disc_icon("drive-harddisk-symbolic"));

    let labels = gtk::Box::new(Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.set_valign(Align::Center);
    let title = gtk::Label::new(Some(&mount.name));
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let subtitle = gtk::Label::new(Some(&mount.root.display().to_string()));
    subtitle.set_halign(Align::Start);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    labels.append(&title);
    labels.append(&subtitle);
    top.append(&labels);

    let space = gtk::Box::new(Orientation::Vertical, 0);
    space.set_valign(Align::Center);
    let avail = gtk::Label::new(Some(&format!(
        "{} Available",
        crate::format::size(mount.available)
    )));
    avail.set_halign(Align::End);
    let total = gtk::Label::new(Some(&format!(
        "{} Total",
        crate::format::size(mount.total)
    )));
    total.set_halign(Align::End);
    total.add_css_class("dim-label");
    total.add_css_class("caption");
    space.append(&avail);
    space.append(&total);
    top.append(&space);

    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.add_css_class("dim-label");
    chevron.set_valign(Align::Center);
    top.append(&chevron);

    let used_fraction = if mount.total > 0 {
        ((mount.total - mount.available) as f64 / mount.total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar = gtk::ProgressBar::new();
    bar.set_fraction(used_fraction);
    bar.add_css_class("usage-bar");
    bar.set_hexpand(true);
    outer.append(&bar);

    row.set_child(Some(&outer));
    row
}

/// A 40px circular disc with a symbolic icon, as used in the reference rows.
pub fn disc_icon(icon_name: &str) -> gtk::Box {
    let disc = gtk::Box::new(Orientation::Horizontal, 0);
    disc.set_size_request(40, 40);
    disc.set_halign(Align::Center);
    disc.set_valign(Align::Center);
    disc.add_css_class("disc");
    let image = gtk::Image::from_icon_name(icon_name);
    image.set_pixel_size(18);
    disc.append(&image);
    disc
}

fn home_dir_string() -> String {
    glib::home_dir().display().to_string()
}
