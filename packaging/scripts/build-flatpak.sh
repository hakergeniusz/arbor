#!/usr/bin/env bash
# Build the single-file Flatpak bundle with flatpak-builder (host; requires
# the org.gnome.Sdk//51 runtime + rust-stable extension installed).
# Usage: build-flatpak.sh v51.0.0-beta.1
set -euo pipefail
tag="${1:?usage: build-flatpak.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
rm -rf dist/flatpak-build dist/flatpak-repo
flatpak-builder --user --force-clean --repo=dist/flatpak-repo \
    dist/flatpak-build packaging/flatpak/io.github.hakergeniusz.Arbor.json
flatpak build-bundle dist/flatpak-repo "dist/arbor-${tag}-x86_64.flatpak" \
    io.github.hakergeniusz.Arbor
flatpak info "dist/arbor-${tag}-x86_64.flatpak" | head -20
