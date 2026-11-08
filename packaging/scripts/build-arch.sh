#!/usr/bin/env bash
# Build the Arch package (pacman -U installable .pkg.tar.zst) from the
# release tarball. Run after make-tarball.sh.
# Usage: build-arch.sh v51.0.0-beta.2
set -euo pipefail
tag="${1:?usage: build-arch.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
archdir="$root/packaging/arch"
cp "$root/dist/arbor-${tag}.tar.gz" "$archdir/"
(cd "$archdir" && makepkg -p PKGBUILD -f --noconfirm)
mv "$archdir"/arbor-*.pkg.tar.zst "$root/dist/"
rm -f "$archdir/arbor-${tag}.tar.gz"
