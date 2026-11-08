#!/usr/bin/env bash
# Build the Fedora .rpm inside a fedora:latest container (needs root docker).
# Usage: build-rpm.sh v51.0.0-beta.2
set -euo pipefail
tag="${1:?usage: build-rpm.sh <tag>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
sudo docker run --rm -v "$root:/src:ro" -v "$root/dist:/out" \
    fedora:latest bash /src/packaging/scripts/in-rpm.sh "$tag"
