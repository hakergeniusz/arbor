#!/usr/bin/env bash
# Build the Debian .deb inside a ubuntu:25.10 container (needs root docker,
# the cleanest way to get real dpkg-deb without touching the host).
# Usage: build-deb.sh v51.0.0-beta.1
set -euo pipefail
tag="${1:?usage: build-deb.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
sudo docker run --rm -v "$root:/src:ro" -v "$root/dist:/out" \
    ubuntu:25.10 bash /src/packaging/scripts/in-deb.sh "$tag"
