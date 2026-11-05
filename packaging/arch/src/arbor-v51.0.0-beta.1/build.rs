fn main() {
    // Embed the original Arbor icon into a gresource so the icon theme can
    // resolve the application icon without a system-wide install (dev runs
    // and tests happen inside an isolated Xvfb session with a fake HOME).
    glib_build_tools::compile_resources(
        &["data"],
        "data/arbor.gresource.xml",
        "arbor.gresource",
    );
    println!("cargo:rerun-if-changed=data");
}
