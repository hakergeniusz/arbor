# 11 — Provenance and Observation Log

This document records how every statement in `docs/01`–`docs/09` was
established, so that another person can reproduce or audit the work.

## 1. Clean-room attestation

- **No upstream source code, repository, tests, build scripts, internal
  documentation, or implementation notes of GNOME Baobab was read,
  cloned, downloaded, decompiled, disassembled, or string-inspected.**
- The binary was verified only via non-invasive metadata
  (`file /usr/bin/baobab` → ELF executable; package metadata
  `pacman -Q baobab` → 50.0-1).
- All behavioral knowledge comes from: (a) driving the GUI with mouse and
  keyboard, (b) screenshots of the virtual display, (c) reading the
  application’s AT-SPI accessibility tree (the same information surface a
  screen reader uses), (d) comparing displayed numbers against
  ground-truth filesystem measurements (`du`, `stat`) of **test fixtures
  created by the team**, and (e) the application’s user-facing CLI argument
  (`baobab <folder>`) and shortcuts window.
- Generic framework/desktop knowledge (GTK file-chooser anatomy, XDG Trash
  specification layout) comes from standard public documentation, not from
  the application.

## 2. Environment

| Item | Value |
|------|-------|
| Application under test | GNOME Baobab **50.0** (package `baobab 50.0-1`); version also confirmed in the About dialog badge (S08) |
| OS | Arch Linux (kernel `7.2.11-zen1-1-zen`), GTK 4.24.1, libadwaita 1.10.0 |
| Display | Xvfb `:98`, 1440×900×24 (`-nolisten tcp -noreset`) |
| Window manager | mutter (X11 backend; launched bare — mutter 50 has no `--replace` flag) |
| Session bus | private `dbus-run-session` per observation session |
| Accessibility | `at-spi-bus-launcher` + manually started `at-spi2-registryd`; `gsettings set org.gnome.desktop.interface toolkit-accessibility true` required for GTK4 clients to register |
| Isolation | `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR` all redirected under `/tmp/ai-agent/` — settings (dconf) and Trash never touched the real user profile |
| Locale | `C.UTF-8` |
| Session launcher | `tools/start-baobab-session.sh` (every process launched with `env -i`) |
| Fixtures | `tools/make-baobab-fixtures.sh`; ground truth via `du -sb`, `du -sB1`, `stat` |
| Screenshots | `ffmpeg -f x11grab` via `tools/shot.sh` (ImageMagick `import` was unusable — see §5) |
| A11y dumps | `tools/a11y.py`, `tools/a11y-focus.py` (PyGObject AT-SPI) |
| Input | `xdotool` (mouse, keys, window size) |

Observation dates: **2026-11-04**, sessions starting ~18:30 and ending
~20:20 local time (UTC+1). Timestamped highlights below; every screenshot
file in `screenshots/` is an unmodified capture from these sessions (the ✕
marks in some captures are the X11 default cursor, not UI).

## 3. Chronological log (condensed)

| Time (approx.) | Event | Result |
|---|---|---|
| 18:27 | recon: tooling present (Xvfb, xdotool, ffmpeg, AT-SPI bindings, mutter) | — |
| 18:30–18:37 | first session attempts; AppImage shim interference discovered (see §5); hermetic launcher written | session stable on `:98` |
| 18:37 | first successful boot screenshot | `boot-first-run.png` (960×600, title, devices) |
| 18:39–18:41 | a11y registration fixed (toolkit-accessibility + manual registryd) | tree dumpable |
| 18:45 | Main Menu contents recorded | `A-main-menu-open.png` |
| 18:46 | Keyboard Shortcuts window (a11y-verified list) | `A-keyboard-shortcuts*.png` |
| 18:48 | About dialog — version 50.0; badge copies to clipboard with toast | `A-about-dialog.png` |
| 18:50 | Preferences + ignore chooser | `A-preferences*.png` |
| 18:52 | resize checks (start page 1200×800, 400×300) | `A-resized-*.png` |
| 18:54 | start-page focus cycle via Tab (a11y) | 3-stop cycle |
| 18:55 | first scan (root device): spinner, partial list, live totals, completed rings | `B-after-home-scan-enter.png`, `B-root-scan-rings.png` |
| 18:58 | Scan Folder chooser: extras + invalid-path toast | `B-chooser-*.png`, `B-scan-dialog-invalid-path.png` |
| 19:00 | fixture-basic scan; size/count arithmetic cracked against `du`/`stat` ground truth | `B-fixture-basic-view.png` |
| 19:05–19:15 | navigation: folder-click, file-click, expander, breadcrumb, up-arrow, ring hover/click, treemap dblclick | `C-*.png` |
| 19:15–19:18 | sorting: name/asc/desc/ties | `C-sort-*.png` |
| 19:20 | context menu; Escape cancel; Copy Path verified by paste; Open Externally failure toast (a11y + pixels) | `D-*.png` |
| 19:25–19:40 | trash tests on `trash-target` + note file: **no confirmation**, XDG trashinfo, stale totals; failure test (read-only Trash): “Failed to trash file”, item survives | `E-*.png` |
| 19:45 | big-tree 25 000 files: mid-scan mutations → snapshot semantics | `B-bigtree-after-midscan.png` |
| 19:50 | root re-scan: chart-mode persistence; Escape mid-scan → start page | `B-cancel-test-after-esc.png` |
| 19:55–20:05 | empty-dir scan; permission-error scan; home scan; recents + Clear Recent List | `B-empty-dir-scan.png`, `B-perm-error-scan.png`, `A-startpage-with-recents.png`, `A-after-clear-recents.png` |
| 20:10 | result-view resize checks | `C-result-*.png` |

## 4. Destructive-action discipline

- Every trash/open-externally test ran against **files and folders created
  by the team** inside `/tmp/ai-agent/baobab-tests/` (fixtures). No real
  user file was ever the target of an action.
- The user’s real Trash was never used: `XDG_DATA_HOME` pointed at the
  sandbox profile, so all trashed items landed in
  `/tmp/ai-agent/fakehome/.local/share/Trash/`.
- The read-only-Trash failure test changed permissions only inside the
  sandbox profile and was restored immediately afterwards.

## 5. Observation-environment incidents (transparency)

1. **Host AppImage shim**: the ZCode host environment intercepts some
   commands (`pkill`, `import`, `--version`-style flags), which produced
   misleading output early on (e.g. `baobab --version` printing an
   unrelated banner). Worked around by using `kill <pid>`, `ffmpeg`, and
   GUI-only version discovery. These are *observation tooling* artifacts,
   not application behavior.
2. **x11grab over ImageMagick**: the system `import` binary was shadowed;
   screenshots were taken with `ffmpeg -f x11grab` instead.
3. **Typing races**: two early chooser attempts had keystrokes split between
   widgets, producing the garbled path in the invalid-path toast
   (“fixture-basicent”). The toast itself is genuine application behavior;
   the garbled text came from the automation, not the app.
4. **AT-SPI registration**: GTK4 clients only register when
   `org.gnome.desktop.interface toolkit-accessibility` is true and the
   registry is reachable; solved inside the sandbox (documented for
   reproducibility).
5. **Window count assumption**: the team briefly believed two windows
   existed; a11y showed the app is single-window and the CLI argument
   *navigates* the existing window. All conclusions use the corrected
   model. The first result window was closed at one point by automation;
   the exact stray keystroke was not identified (no impact on findings).
6. **Screenshot timing**: toasts auto-dismiss in ~3–4 s; two toasts
   (“Failed to open file”, “Failed to trash file”) were captured both by
   pixels and a11y; “Copied to clipboard” was captured once (About badge).

## 6. Uncertainties and open questions (also marked inline as **Unknown**)

1. Effect of the “Recursively analyze mount points” checkbox (never toggled).
2. Recents persistence across application restarts (persisted profile
   observed, restart not separately re-verified).
3. Window size/position persistence across restarts.
4. Whether Escape mid-scan cancels the internal scan or only leaves the view.
5. Ring-segment click semantics (no effect at three tested radii; deeper
   interaction model unknowable black-box).
6. Sort tie-break rule (stable but non-alphabetical).
7. Whether the infobar is conditional or static (present in 100 % of
   observed result views).
8. Keyboard path to the context menu; Help (F1) destination; Credits/Legal
   sub-page contents.
9. Clicking a recent row (not separately exercised; same row pattern as
   devices).
10. Exact minimum window size (no wall hit at 400×300; smaller not tested).

## 7. Handoff

Team B receives: `docs/` (this specification), `screenshots/` (original
captures), `tools/` (observation scripts, useful for acceptance testing).
Nothing derived from upstream sources is included, because none was used.
