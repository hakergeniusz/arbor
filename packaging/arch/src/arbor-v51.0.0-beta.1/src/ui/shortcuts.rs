//! Keyboard Shortcuts window (docs/01 §7, docs/08 §2).

use adw::prelude::*;

const XML: &str = r#"
<interface>
  <object class="AdwShortcutsDialog" id="shortcuts">
    <child>
      <object class="AdwShortcutsSection">
        <property name="title" translatable="no">General</property>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Show Help</property>
            <property name="accelerator">F1</property>
          </object>
        </child>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Open Main Menu</property>
            <property name="accelerator">F10</property>
          </object>
        </child>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Quit</property>
            <property name="accelerator">&lt;Ctrl&gt;Q</property>
          </object>
        </child>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Show Keyboard Shortcuts</property>
            <property name="accelerator">&lt;Ctrl&gt;question</property>
          </object>
        </child>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Show Preferences</property>
            <property name="accelerator">&lt;Ctrl&gt;comma</property>
          </object>
        </child>
      </object>
    </child>
    <child>
      <object class="AdwShortcutsSection">
        <property name="title" translatable="no">Scanning</property>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Scan Folder</property>
            <property name="accelerator">&lt;Ctrl&gt;O</property>
          </object>
        </child>
        <child>
          <object class="AdwShortcutsItem">
            <property name="title" translatable="no">Rescan Current Location</property>
            <property name="accelerator">&lt;Ctrl&gt;R</property>
          </object>
        </child>
      </object>
    </child>
  </object>
</interface>
"#;

/// Present the shortcuts dialog over `parent`.
pub fn present(parent: &impl gtk::prelude::IsA<gtk::Widget>) {
    let builder = gtk::Builder::from_string(XML);
    let dialog = builder
        .object::<adw::ShortcutsDialog>("shortcuts")
        .expect("shortcuts dialog must exist in embedded UI definition");
    dialog.present(Some(parent));
}
