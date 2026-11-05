#!/usr/bin/env bash
# Build the portable .tar.zst inside a ubuntu:25.10 container (needs root
# docker) so the binary links the oldest glibc of our targets.
# Usage: build-portable.sh v51.0.0-beta.1
set -euo pipefail
tag="${1:?usage: build-portable.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
sudo docker run --rm -v "$root:/src:ro" -v "$root/dist:/out" \
    ubuntu:25.10 bash /src/packaging/scripts/in-portable.sh "$tag"
