#!/usr/bin/env bash
# Runs INSIDE debian:trixie (driven by build-deb.sh, /src read-only repo
# checkout, /out artifact dir): install build deps, compile the release
# binary, stage the package tree and build the .deb with dpkg-deb.
set -euxo pipefail
tag="$1"
ver="${tag#v}"; debver="${ver//-/~}"          # 51.0.0-beta.1 -> 51.0.0~beta.1
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends \
    ca-certificates curl gcc libgtk-4-dev libadwaita-1-dev pkg-config binutils
# trixie's rustc 1.85 predates the locked dependency tree — use the current
# stable toolchain via rustup instead of the distro rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
export PATH="$HOME/.cargo/bin:$PATH"

mkdir /work && tar xzf "/src/dist/arbor-${tag}.tar.gz" -C /work
cd "/work/arbor-${tag}"
cargo build --release --locked

stage=/pkg/debian/arbor
install -D -s -m 755 target/release/arbor "$stage/usr/bin/arbor"
install -D -m 644 data/io.github.hakergeniusz.Arbor.desktop.in \
    "$stage/usr/share/applications/io.github.hakergeniusz.Arbor.desktop"
install -D -m 644 data/io.github.hakergeniusz.Arbor.metainfo.xml \
    "$stage/usr/share/metainfo/io.github.hakergeniusz.Arbor.metainfo.xml"
install -D -m 644 data/icons/scalable/apps/io.github.hakergeniusz.Arbor.svg \
    "$stage/usr/share/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg"
install -D -m 644 LICENSE "$stage/usr/share/doc/arbor/copyright"

# Precise runtime floors: the glibc symbol floor actually referenced plus the
# gtk/adw versions the binary was just built against.
libc_min=$(objdump -T target/release/arbor | grep -o 'GLIBC_[0-9.]*' | \
    sort -Vu | tail -1 | cut -d_ -f2)
gtk_min=$(pkg-config --modversion gtk4)
adw_min=$(pkg-config --modversion libadwaita-1)
cat > "$stage/DEBIAN/control" <<EOF
Package: arbor
Version: ${debver}-1
Architecture: amd64
Maintainer: hakergeniusz <hakergeniusz@users.noreply.github.com>
Depends: libc6 (>= ${libc_min}), libgtk-4-1 (>= ${gtk_min}), libadwaita-1-0 (>= ${adw_min})
Section: utils
Priority: optional
Homepage: https://github.com/hakergeniusz/arbor
Description: see where your disk space grows
 Arbor is a disk usage analyzer: scans folders asynchronously and presents
 sizes, item counts and modification dates as a sortable tree plus rings and
 treemap visualizations. Items can be opened, their path copied, and moved
 to the Trash — always behind a mandatory confirmation dialog with an undo.
 .
 Independent clean-room implementation of the behavior of GNOME Baobab 50.0;
 contains no upstream code.
EOF
(cd "$stage" && find usr -type f -exec md5sum {} + | sort -k2 > DEBIAN/md5sums)

mkdir -p /out
dpkg-deb --root-owner-group --build "$stage" "/out/arbor_${debver}-1_amd64.deb"
dpkg-deb --info "/out/arbor_${debver}-1_amd64.deb"
