# Packaging

Arbor ships as native packages for the four major Linux distribution
families plus a Flatpak manifest. All targets install the same four files:

- `usr/bin/arbor`
- `usr/share/applications/io.github.hakergeniusz.Arbor.desktop`
- `usr/share/metainfo/io.github.hakergeniusz.Arbor.metainfo.xml`
- `usr/share/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg`

Build requirements: Rust ≥ 1.85 (edition 2024), GTK 4 ≥ 4.10,
libadwaita ≥ 1.8, `pkg-config`. Builds are `cargo build --release
--locked --offline` against the committed `Cargo.lock`.

## Debian

```
cp -r packaging/debian .        # debian/ must sit at the source root
dpkg-buildpackage -us -uc -b
```

Produces `../arbor_1.0.0-1_<arch>.deb`. `Architecture: any` — builds on
`amd64` and `arm64`.

## RPM

```
rpmbuild -ba packaging/rpm/arbor.spec    # from a v1.0.0 source tarball
```

`ExclusiveArch: %{rust_tier1_arches}` — x86_64 and aarch64.

## Arch Linux

```
cd packaging/arch && makepkg -p PKGBUILD
```

`arch=('x86_64' 'aarch64')`. Replace `sha256sums=('SKIP')` with the real
checksum when cutting the release tarball.

## Flatpak

```
flatpak-builder --user --install --force-clean \
    build-dir packaging/flatpak/io.github.hakergeniusz.Arbor.json
```

Uses `org.gnome.Platform//50` + the rust SDK extension. Set the manifest's
`commit` field to the released commit hash. The sandbox grants
`--filesystem=host` because the scanner reads arbitrary paths and moves
items to the host XDG Trash; the trash confirmation dialog (docs/06) is
enforced in-process and cannot be bypassed by the sandbox.

## Status

No dpkg/rpm/makepkg toolchain was available in the implementation
environment, and `flatpak-builder` requires network access to fetch the
GNOME SDK — so the manifests are **provided and reviewed, but not
built/tested** (see `docs/FINAL_REPORT.md`). The install layout and file
names are the contract; adjust only paths if a distribution requires it.
