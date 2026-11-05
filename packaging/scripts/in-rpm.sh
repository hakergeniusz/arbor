#!/usr/bin/env bash
# Runs INSIDE fedora:latest (driven by build-rpm.sh): install build deps and
# rpmbuild the spec from the release tarball.
set -euxo pipefail
tag="$1"
# libadwaita-devel (current Fedora) with a fallback to the older
# soname-suffixed name
dnf install -y --setopt=install_weak_deps=False \
    rust cargo gtk4-devel libadwaita-devel pkgconf rpm-build || \
dnf install -y --setopt=install_weak_deps=False \
    rust cargo gtk4-devel libadwaita1-devel pkgconf rpm-build

mkdir /work && tar xzf "/src/dist/arbor-${tag}.tar.gz" -C /work
top=/rpmbuild
mkdir -p "$top"/{SOURCES,SPECS,RPMS,BUILD,SRPMS,BUILDROOT}
cp "/work/arbor-${tag}/packaging/rpm/arbor.spec" "$top/SPECS/"
cp "/src/dist/arbor-${tag}.tar.gz" "$top/SOURCES/${tag}.tar.gz"
rpmbuild -bb --define "_topdir $top" "$top/SPECS/arbor.spec"
cp "$top"/RPMS/x86_64/*.rpm /out/
rpm -qlp /out/arbor-*.rpm
