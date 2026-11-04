# Implementation Notes

Field notes from implementing Arbor against Team A's specification — the
decisions that are not visible in the architecture, the harness that made
GUI verification possible, and what remains untested.

## Key implementation decisions

- **Snapshot + app-side sibling sorting.** The list is a flat
  `gio::ListStore` of `ScanRow`s rebuilt from the `DisplayNode` snapshot;
  expanded folders are interleaved by the rebuild, and siblings are sorted
  by the `ColumnView`'s mirrored sort key (`SortKey` enum). A model-level
  sorter cannot express "children follow their parent", so the header
  clicks drive app-side rebuilds instead. `GtkColumnViewSorter` mutates in
  place, so the sorter's own `changed` signal (not `notify::sorter`) is
  the trigger.
- **Rings geometry**: children subdivide the *parent's arc* — `start`/`span`
  are threaded through the recursion and mapped proportionally. Segment
  angles use 12 o'clock as origin, clockwise; hover hit-testing converts
  the pointer angle with `atan2(-dy, dx) + 90°`.
- **Treemap**: one custom `DrawingArea`, not a widget-per-rect tree —
  layout produces a `Vec<TreeRect>` (parents first, children inset by
  3 px), drawn and hit-tested in reverse. Double-click navigates into the
  deepest *folder* region under the pointer.
- **Trash undo without trash-daemon assumptions**: undo records the
  original path, then scans same-device candidate trash directories
  (`.Trash-$uid`, `.Trash/$uid`, XDG data dir) for the newest `.trashinfo`
  whose percent-decoded `Path=` matches. Restore is conservative: it
  renames only, never overwrites, and removes the `.trashinfo` only after
  success. `gio::File::trash` is used with **no deletion fallback** — if
  the Trash refuses (read-only, foreign topdir), the item stays untouched
  and a toast explains the failure (docs/06 R5/R7).
- **Toast buttons**: libadwaita's `AdwToast` in the bound Rust version has
  no `add_button`; Undo is attached with `set_button_label` +
  `set_action_name("win.undo-trash")`.
- **Context menus** are one `PopoverMenu` with
  `set_action_and_target_value` entries (`win.open-externally`,
  `win.copy-path`, `win.trash-item` — all STRING parameters), opened
  either by right-click gestures (button 3, capture phase) or by the
  `Menu`/`Shift+F10` keyboard action anchored at the focused row cell.
- **Charts and accessibility**: a `GtkDrawingArea`'s default accessible
  role is *none* — the charts are invisible to assistive technology
  unless an explicit role is set. Both charts set `AccessibleRole::Img`
  with a `Label` and a live `Description` summary (rings: total + top
  items; treemap: shown folder, then the keyboard-focused rectangle).
  The row percentage `ProgressBar` has role *none* so its raw fraction
  ("0.985563") does not pollute the row's accessible name; the visible
  "98.56%" label carries the value, matching the spec's observed row
  string format.

## GTK4-rs pitfalls that cost debugging time

- `GtkGestureSingle` defaults to **button 1**; a plain
  `GtkGestureClick` never sees right-clicks — `set_button(3)` for context,
  `set_button(0)` for all-buttons.
- `ListItem` is not a `Widget`; per-row gestures must capture the
  `ListItem` from the factory setup directly, `ancestor()` cannot reach it.
- Weak-obj pattern (`downgrade()` → `upgrade()`) everywhere; imp structs
  are not `Clone`.
- `gtk::Ordering` vs `std::cmp::Ordering` in custom sorter closures
  (`.into()`).
- `AdwMessageDialog` is deprecated (1.6+); `AdwAlertDialog` with
  `set_default_response`/`set_close_response` implements docs/06 R3
  (Cancel on Enter and Escape) cleanly.
- `WidgetExt::allocation()` is deprecated (4.12+) — `#[allow(deprecated)]`
  with a comment, used once for anchoring the keyboard context menu.

## Hermetic GUI harness notes

All GUI validation ran inside an isolated Xvfb display (`env -i` with
private dbus, fake `HOME`, redirected XDG dirs) so nothing could reach the
real desktop or the real Trash. Practical notes:

- No window manager is available in the sandbox (mutter 51 dropped X11 WM
  mode), and `xdotool search --class | head -1` can pick a GTK helper
  window — enumerate `--onlyvisible` windows and resize the real toplevel
  by id, or resize tests silently do nothing.
- `pkill` is unreliable in the sandbox; kill by explicit pid loop.
- The AppImage wrapper on this machine shadows some tools (`convert`,
  `--help` output mangling); use absolute paths (`/usr/bin/convert`).
- AT-SPI verification works: start `at-spi-bus-launcher
  --launch-immediately` and `/usr/lib/at-spi2-registryd` (with
  `AT_SPI_BUS_ADDRESS` pointing at the a11y bus socket) **before** the
  app, launch the app with `GTK_A11Y=atspi`, then walk the tree with
  python3-Atspi. Backgrounded daemons need `setsid` to survive the
  launching shell.

## Deviations from the plan

- The plan sketched `fsops.rs` and separate `treemap.rs`; the final layout
  keeps trash/undo in `trash.rs`, charts together in `ui/charts.rs`, and
  adds `ui/confirm.rs` for the mandatory dialog — a cleaner split along
  the safety boundary.
- `libc` is a regular dependency (trash candidate-dir logic uses
  `geteuid`), not a dev-only one as first assumed.

## Untested / lower confidence

See `docs/FINAL_REPORT.md` for the full implemented / tested / untested
matrix — notably the distribution packages (no dpkg/rpm toolchain in the
build environment) and aarch64 builds.
