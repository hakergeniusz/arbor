# Packaging

Arbor ships as native packages for the four major Linux distribution
families plus a Flatpak bundle and a portable archive. All package targets
install the same four files:

- `usr/bin/arbor`
- `usr/share/applications/io.github.hakergeniusz.Arbor.desktop`
- `usr/share/metainfo/io.github.hakergeniusz.Arbor.metainfo.xml`
- `usr/share/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg`

Build requirements: Rust ≥ 1.85 (edition 2024), GTK 4 ≥ 4.10,
libadwaita ≥ 1.8, `pkg-config`. Builds are `cargo build --release --locked`
against the committed `Cargo.lock`.

Release artifacts are cut with the scripts in `packaging/scripts/` (they
need `docker` with root for the Debian/Fedora chroot builds, `makepkg`,
`flatpak-builder` with the `org.gnome.Sdk//51` runtime, and `zstd`):

```
packaging/scripts/make-tarball.sh v51.0.0-beta.1     # git-archive source tarball
packaging/scripts/build-arch.sh                      # makepkg → .pkg.tar.zst
packaging/scripts/build-deb.sh                       # debian:trixie → .deb
packaging/scripts/build-rpm.sh                       # fedora:latest → .rpm
packaging/scripts/build-flatpak.sh                   # flatpak-builder → .flatpak bundle
packaging/scripts/build-portable.sh                  # debian:trixie binary → .tar.zst
```

## Debian

```
cp -r packaging/debian .        # debian/ must sit at the source root
dpkg-buildpackage -us -uc -b
```

Produces `../arbor_51.0.0~beta.1-1_<arch>.deb`. `Architecture: any` — builds
on `amd64` and `arm64`. The released .deb is built against Ubuntu 25.10's
stack (libadwaita 1.9), so it needs Ubuntu 25.10+ / Debian sid-or-newer at
runtime; the package's Depends carry the exact floors of the build image.

## RPM

```
rpmbuild -ba packaging/rpm/arbor.spec    # from the v51.0.0-beta.1 source tarball
```

`ExclusiveArch: %{rust_tier1_arches}` — x86_64 and aarch64. Pre-release
versioning per Fedora guidelines: `51.0.0-0.1.beta1`.

## Arch Linux

```
cd packaging/arch && makepkg -p PKGBUILD
```

Installs with `pacman -U arbor-51.0.0beta1-1-x86_64.pkg.tar.zst`.
`arch=('x86_64' 'aarch64')`. Replace `sha256sums=('SKIP')` with the real
checksum when cutting the release tarball.

## Flatpak

```
flatpak-builder --user --force-clean --install-deps-from=flathub \
    build-dir packaging/flatpak/io.github.hakergeniusz.Arbor.json
flatpak build-bundle build-dir \
    arbor-51.0.0-beta.1-x86_64.flatpak io.github.hakergeniusz.Arbor
```

Uses `org.gnome.Platform//51` + the rust SDK extension. Pin the manifest's
`commit` field to the released commit hash. The sandbox grants
`--filesystem=host` because the scanner reads arbitrary paths and moves
items to the host XDG Trash; the trash confirmation dialog (docs/06) is
enforced in-process and cannot be bypassed by the sandbox.

## Portable

`arbor-51.0.0-beta.1-portable-x86_64.tar.zst` contains the stripped binary
(built in a debian:trixie chroot for the widest glibc compatibility), the
license, the desktop file and icon for optional manual installation, and a
README. Extract and run `./arbor` — needs GTK 4 ≥ 4.10 and libadwaita ≥ 1.8
at runtime.
