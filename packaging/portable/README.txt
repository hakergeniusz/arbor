Arbor 51.0.0-beta.1 — portable build (x86_64)
=============================================

Run:  ./arbor [path-to-scan]

Requirements (runtime, already present on any mainstream desktop distro):
  GTK 4 >= 4.10, libadwaita >= 1.8, glibc >= the version of the Debian
  release the binary was built in, and a graphics session (X11 or Wayland).

The binary is self-contained apart from those system libraries. If your
desktop's icon theme does not resolve the app icon, install the bundled
share/applications/io.github.hakergeniusz.Arbor.desktop into
~/.local/share/applications/ and the SVG into
~/.local/share/icons/hicolor/scalable/apps/ (both optional).

License: MIT — see LICENSE. This is an independent clean-room
implementation of the behavior of GNOME Baobab 50.0; it contains no
upstream code.
