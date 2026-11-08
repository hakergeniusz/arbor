#!/usr/bin/env bash
# Cut the release source tarball from a tag: arbor-<tag>.tar.gz, contents
# under arbor-<tag>/ — the layout packaging/{arch,rpm,debian} expect.
# Usage: make-tarball.sh v51.0.0-beta.2
set -euo pipefail
tag="${1:?usage: make-tarball.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$root/dist"
git -C "$root" archive --prefix="arbor-${tag}/" --format=tar.gz \
    -o "$root/dist/arbor-${tag}.tar.gz" "$tag"
sha256sum "$root/dist/arbor-${tag}.tar.gz"
