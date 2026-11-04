//! List row model for the result-view table (docs/04 §2).
//!
//! One `ScanRow` object per displayed row; the flat store is rebuilt whenever
//! the tree, expansion state, sort order or navigation changes.

#![allow(dead_code)]

use std::path::PathBuf;

use gtk::glib;
use gtk::glib::prelude::*;
use gtk::glib::Properties;

mod imp {
    use super::*;
    use gtk::glib::subclass::prelude::*;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::ScanRow)]
    pub struct ScanRow {
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub path: RefCell<String>,
        #[property(get, set)]
        pub size_str: RefCell<String>,
        #[property(get, set)]
        pub items_str: RefCell<String>,
        #[property(get, set)]
        pub modified_str: RefCell<String>,
        #[property(get, set)]
        pub percent: Cell<f64>,
        /// Sortable numeric mirrors (docs/04 §3).
        #[property(get, set)]
        pub size_bytes: Cell<u64>,
        #[property(get, set)]
        pub items_count: Cell<u64>,
        #[property(get, set)]
        pub mtime: Cell<i64>,
        #[property(get, set)]
        pub is_location: Cell<bool>,
        #[property(get, set)]
        pub is_dir: Cell<bool>,
        #[property(get, set)]
        pub error: Cell<bool>,
        #[property(get, set)]
        pub expanded: Cell<bool>,
        #[property(get, set)]
        pub depth: Cell<u32>,
        /// Name color (matches the chart segment color; docs/10 §5).
        #[property(get, set)]
        pub color: RefCell<String>,
        #[property(get, set)]
        pub has_children: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ScanRow {
        const NAME: &'static str = "ArborScanRow";
        type Type = super::ScanRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ScanRow {}

    // ObjectExt (property access) comes from the glib prelude, which the
    // outer module already imports via gtk::glib.
}

glib::wrapper! {
    pub struct ScanRow(ObjectSubclass<imp::ScanRow>);
}

impl ScanRow {
    pub fn new(path: PathBuf) -> Self {
        glib::Object::builder()
            .property("path", path.display().to_string())
            .build()
    }

    /// One readable string per row for assistive technology
    /// (docs/08 §1: rows fully readable as text).
    pub fn accessible_text(&self) -> String {
        format!(
            "{}{} {} {} {}",
            if self.is_dir() && !self.is_location() && self.has_children() {
                if self.expanded() { "expanded, " } else { "collapsed, " }
            } else {
                ""
            },
            self.name(),
            self.size_str(),
            self.items_str(),
            self.modified_str(),
        )
    }
}
