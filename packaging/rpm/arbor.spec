# Spec file for arbor — build with: rpmbuild -ba packaging/rpm/arbor.spec
# (or packaging/scripts/build-rpm.sh, which drives a fedora container)
Name:           arbor
Version:        51.0.0
Release:        0.1.beta1%{?dist}
Summary:        See where your disk space grows — a disk usage analyzer
License:        MIT
URL:            https://github.com/hakergeniusz/arbor
Source0:        %{url}/archive/refs/tags/v51.0.0-beta.1.tar.gz

# rust tier-1: covers x86_64 and aarch64; the macro needs rust-srpm-macros,
# fall back to the explicit list where it is not installed
ExclusiveArch:  %{?rust_tier1_arches}%{!?rust_tier1_arches:x86_64 aarch64}

BuildRequires:  rust >= 1.85
BuildRequires:  cargo
BuildRequires:  pkgconfig(gtk4) >= 4.10
BuildRequires:  pkgconfig(libadwaita-1) >= 1.8

Requires:       hicolor-icon-theme

%global _description %{expand:
Arbor is a disk usage analyzer for Linux: it scans folders asynchronously and
presents sizes, item counts and modification dates as a sortable tree plus
rings and treemap visualizations. Items can be opened with the default
handler, their path copied, and moved to the Trash — always behind a
mandatory confirmation dialog with an undo. An independent clean-room
implementation of the behavior of GNOME Baobab 50.0; it contains no upstream
code.}

%description
%{_description}

%prep
%autosetup -n arbor-v51.0.0-beta.1

%build
cargo build --release --locked

%install
install -D -m 755 target/release/arbor %{buildroot}%{_bindir}/arbor
install -D -m 644 data/io.github.hakergeniusz.Arbor.desktop.in \
    %{buildroot}%{_datadir}/applications/io.github.hakergeniusz.Arbor.desktop
install -D -m 644 data/io.github.hakergeniusz.Arbor.metainfo.xml \
    %{buildroot}%{_metainfodir}/io.github.hakergeniusz.Arbor.metainfo.xml
install -D -m 644 data/icons/scalable/apps/io.github.hakergeniusz.Arbor.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg

%check
cargo test --locked

%files
%license LICENSE
%doc README.md ARCHITECTURE.md IMPLEMENTATION_NOTES.md PROVENANCE.md THIRD_PARTY_LICENSES.md
%{_bindir}/arbor
%{_datadir}/applications/io.github.hakergeniusz.Arbor.desktop
%{_metainfodir}/io.github.hakergeniusz.Arbor.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.hakergeniusz.Arbor.svg

%changelog
* Thu Nov 05 2026 hakergeniusz <hakergeniusz@users.noreply.github.com> - 51.0.0-0.1.beta1
- First public beta (clean-room implementation of the Arbor behavioral
  specification; see docs/ for the specification and provenance).
* Wed Nov 04 2026 hakergeniusz <hakergeniusz@users.noreply.github.com> - 1.0.0-1
- Initial package
