#!/usr/bin/env bash
# Runs INSIDE debian:trixie (driven by build-portable.sh): compile the
# release binary and pack the portable archive.
set -euxo pipefail
tag="$1"
ver="${tag#v}"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends \
    ca-certificates cargo rustc libgtk-4-dev libadwaita-1-dev pkg-config binutils zstd

mkdir /work && tar xzf "/src/dist/arbor-${tag}.tar.gz" -C /work
cd "/work/arbor-${tag}"
cargo build --release --locked

port=/pkg/portable/arbor-${ver}
install -D -s -m 755 target/release/arbor "$port/arbor"
install -D -m 644 LICENSE "$port/LICENSE"
install -D -m 644 /src/packaging/portable/README.txt "$port/README.txt"
install -D -m 644 data/io.github.hakergeniusz.Arbor.desktop.in \
    "$port/share/applications/io.github.hakergeniusz.Arbor.desktop"
install -D -m 644 data/icons/scalable/apps/io.github.hakergeniusz.Arbor.svg \
    "$port/share/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg"

mkdir -p /out
tar -C /pkg/portable --zstd -cf "/out/arbor-${ver}-portable-x86_64.tar.zst" "arbor-${ver}"
