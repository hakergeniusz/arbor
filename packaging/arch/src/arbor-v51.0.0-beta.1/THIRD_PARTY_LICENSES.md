# Third-Party Licenses

Arbor is MIT-licensed. This file audits the licenses of everything Arbor
links or builds against, as of version 1.0.0 (`Cargo.lock` at commit
`df1fa30`). License fields were read from each dependency's crate manifest in
the local cargo registry — no manual copying.

## Summary

Every Rust dependency is permissively licensed (MIT and/or Apache-2.0, with
two narrow build-time exceptions listed below). **No copyleft license (GPL,
LGPL, MPL, …) appears anywhere in the dependency tree**, so the MIT license of
Arbor itself is unaffected.

## System libraries (runtime, not redistributed by Arbor)

| Library | License | Notes |
|---|---|---|
| GTK 4 (≥ 4.10 used) | LGPL-2.1-or-later | linked dynamically at runtime |
| libadwaita (≥ 1.8 used) | LGPL-2.1-or-later | linked dynamically at runtime |
| GLib / GObject / GIO / pango / gdk-pixbuf / graphene / cairo | LGPL-2.1-or-later (cairo: LGPL-2.1-or-later or MPL-1.1) | via gtk4-rs `-sys` crates |

Dynamic linking against LGPL libraries imposes no obligations on Arbor's
source; users may replace those libraries independently.

## Rust dependencies (72 crates)

| License | Crates |
|---|---|
| MIT | `cairo-rs`, `cairo-sys-rs`, `gdk-pixbuf`, `gdk-pixbuf-sys`, `gdk4`, `gdk4-sys`, `gio`, `gio-sys`, `glib`, `glib-build-tools`, `glib-macros`, `glib-sys`, `gobject-sys`, `graphene-rs`, `graphene-sys`, `gsk4`, `gsk4-sys`, `gtk4`, `gtk4-macros`, `gtk4-sys`, `libadwaita`, `libadwaita-sys`, `memoffset`, `pango`, `pango-sys`, `slab`, `winnow`, `zmij` |
| MIT OR Apache-2.0 | `bitflags`, `futures-*` (channel, core, executor, io, macro, task, util), `field-offset`, `heck`, `libc`, `proc-macro-crate`, `proc-macro2`, `quote`, `rustc_version`, `semver`, `serde`, `serde_core`, `serde_derive`, `serde_json`, `serde_spanned`, `smallvec`, `syn`, `system-deps` (7.x, 9.x), `toml`, `toml_datetime`, `toml_edit`, `toml_parser`, `toml_writer`, `windows-link`, `windows-sys`, `autocfg`*, `cfg-expr`, `equivalent`*, `hashbrown`, `indexmap`, `itoa`, `pin-project-lite`, `pkg-config`, `version-compare` |
| Unlicense OR MIT | `memchr` |
| Apache-2.0 WITH LLVM-exception | `target-lexicon` (build-time only, via `system-deps`) |
| (MIT OR Apache-2.0) AND Unicode-3.0 | `unicode-ident` (build-time only, via `syn`) |

\* `autocfg` is "Apache-2.0 OR MIT", `equivalent` "Apache-2.0 OR MIT" — same
dual-license set, listed with reversed order in their manifests.

Crates behind `serde`'s derive and the gtk4-rs macro stack (`heck`,
`proc-macro-crate`, `quote`, `syn`, …) are build-time only; they do not ship
in the binary.

## Assets

- Application icon `data/icons/scalable/apps/io.github.hakergeniusz.Arbor.svg`
  — original artwork for Arbor, MIT, same as the project.
- UI copy and documentation in `docs/` and `screenshots/` — part of this
  repository, MIT unless noted otherwise.

## Audit method

```
cargo metadata --locked > /dev/null      # Cargo.lock is committed
# for each (name, version) in Cargo.lock, read `package.license`
# from ~/.cargo/registry/src/index.crates.io-*/name-version/Cargo.toml
```

Re-run after any dependency change; `cargo deny check licenses` can automate
the same check in CI.
