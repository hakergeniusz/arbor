# Arbor — see where your disk space grows

Arbor is a disk usage analyzer for Linux (GTK4 / libadwaita, Rust): it
scans folders asynchronously and shows sizes, item counts and modification
dates as a sortable tree, plus rings and treemap visualizations. Items can
be opened with the default handler, their path copied, and moved to the
Trash — always behind a mandatory confirmation dialog, with an undo.

Arbor is an **independent, clean-room implementation** of the user-facing
behavior of GNOME Baobab 50.0, built from a black-box behavioral
specification. It contains no upstream code. See `PROVENANCE.md`.

## Build

Dependencies: Rust ≥ 1.85, GTK 4 ≥ 4.10, libadwaita ≥ 1.8, `pkg-config`.

```
cargo build --release
./target/release/arbor            # start page
./target/release/arbor <folder>   # scan that folder (single instance)
```

## Test

```
cargo test                        # unit + scanner tests, no GUI needed
```

GUI validation (Xvfb, hermetic session) and the acceptance scenarios are
described in `docs/09-test-scenarios.md`; implementation evidence in
`docs/FINAL_REPORT.md`.

## Install / packaging

| Format | Location | Status |
|---|---|---|
| Debian | `packaging/debian/` (`dpkg-buildpackage` from repo root) | provided, not built in CI |
| RPM | `packaging/rpm/arbor.spec` (`rpmbuild -ba`) | provided, not built in CI |
| Arch | `packaging/arch/PKGBUILD` (`makepkg`) | provided, not built in CI |
| Flatpak | `packaging/flatpak/io.github.hakergeniusz.Arbor.json` | provided, not built in CI |

All targets cover `x86_64` and `aarch64` (`arch: any`, `%{rust_tier1_arches}`,
`arch=('x86_64' 'aarch64')`); only `x86_64` was exercised locally.

## Repository layout

```
docs/           Team A behavioral specification (numbered reading order)
screenshots/    Original UI reference screenshots from the observation
src/            Arbor implementation (Rust, GTK4/libadwaita)
data/           desktop file, appstream metainfo, application icon
packaging/      deb / rpm / arch / flatpak packaging
tests/          integration tests (scanner fixtures)
tools/          Team A's observation harness (not used by the implementation)
```

## Documentation

- `ARCHITECTURE.md` — module map, data flow, invariants
- `IMPLEMENTATION_NOTES.md` — decisions, GTK pitfalls, harness notes
- `THIRD_PARTY_LICENSES.md` — dependency license audit (permissive only)
- `CONTRIBUTING.md` — clean-room rules and development workflow
- `LICENSE` — MIT

## Provenance statement

All behavioral knowledge in `docs/` was obtained by interacting with the
installed Baobab 50.0 as an ordinary user inside an isolated Xvfb display
(see `docs/11-provenance-and-observation-log.md`). The implementation
consulted only those documents, the screenshots, and public GTK/Rust
documentation — never upstream source code.
