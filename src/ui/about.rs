//! About dialog (docs/01 §8, decision D18).

use adw::prelude::*;

/// Present the Arbor about dialog over `parent`.
///
/// libadwaita's about dialog implements the version-badge click → copy +
/// "Copied to clipboard" toast behavior natively.
pub fn present(parent: &impl gtk::prelude::IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_name("Arbor")
        .application_icon(crate::app::APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("hakergeniusz")
        .license_type(gtk::License::MitX11)
        .website("https://github.com/hakergeniusz/arbor")
        .issue_url("https://github.com/hakergeniusz/arbor/issues")
        .build();
    // The spec's About carries a Credits sub-page (docs/01 §8); libadwaita
    // only shows the row when a credit section exists.
    about.add_credit_section(Some("Implementation"), &["Team B — independent clean-room implementation"]);
    about.present(Some(parent));
}
