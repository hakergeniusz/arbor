# Architecture

Arbor is a GTK4/libadwaita desktop application in Rust. It implements the
behavioral specification in `docs/` — a black-box description of GNOME
Baobab 50.0 — without containing any upstream code (see `PROVENANCE.md`).

## Module map

```
src/
├── main.rs             binary entry: glib logging, app.run()
├── lib.rs              library crate so tests can exercise internals
├── app.rs              AdwApplication: actions, menu model, accelerators,
│                       single-instance `arbor <folder>` handling
├── scanner.rs          async directory walk → DisplayNode snapshot
│                       (single GTask-style thread, progress events,
│                       cancellation, error capture, hidden files,
│                       symlinks excluded/cycle-safe)
├── models.rs           DisplayNode tree (size/items/mtime aggregates,
│                       error flags, detach for trash-undo bookkeeping)
├── mounts.rs           "This Device" list: mounts via GUnixMounts,
│                       dedup of devices, home folder pinned first
├── settings.rs         JSON persistence: recents, scan options, window
│                       state ($XDG_DATA_HOME/arbor/settings.json)
├── format.rs           size formatting (decimal units, 3 significant
│                       digits, <1 kB → bytes) and relative day strings
├── squarify.rs         squarified-treemap strip layout (pure function,
│                       unit-tested invariants)
├── trash.rs            gio File::trash + undo: locate the newest matching
│                       .trashinfo across candidate topdirs, conservative
│                       restore (never overwrites, rename only)
└── ui/
    ├── window.rs       ArborUi: headerbar, view stack, actions, row/chart
    │                   wiring, keyboard map, toasts, item menus
    ├── start_page.rs   Devices & Locations list (S01–S03)
    ├── result_view.rs  breadcrumb, infobar banner, ColumnView rows and
    │                   sorters, chart pages, row context-menu gestures
    ├── charts.rs       RingsChart + TreemapChart (custom cairo draw,
    │                   keyboard walk, accessible summaries)
    ├── scan_row.rs     ScanRow GObject: one row's model + accessible text
    ├── confirm.rs      MANDATORY trash confirmation dialog (docs/06)
    ├── chooser.rs      Scan Folder… dialog + recursive-mounts checkbox
    ├── preferences.rs  scan options dialog
    ├── shortcuts.rs    keyboard shortcuts window
    ├── about.rs        about dialog
    └── palette.rs      deterministic color assignment (golden-angle hue
                        per name-sorted sibling, depth-varied HSL)
```

## Data flow

1. **Scan**: `window.open_location(path)` → `Scanner::scan(path)` on a
   background thread, emitting progress; the UI shows a spinner and
   progressively fills the list from the partial snapshot (docs/03).
2. **Snapshot semantics**: the finished scan hands over one immutable
   `DisplayNode` tree; mid-scan filesystem changes are not reflected until
   rescan (S14). Hidden files are included, symlinks are excluded and
   cycle-safe, permission errors become per-node error rows.
3. **Rendering**: the result view flattens the tree to `ScanRow`s
   (`build_rows`), expanding folders in place (location row pinned first),
   sorting siblings app-side by the ColumnView's mirrored sort key — a
   flat model sorter would interleave expanded children.
4. **Visualization**: `to_chart_node` projects the tree to chart nodes;
   the treemap re-roots to the selected folder's subtree. Ring segments
   subdivide the parent's arc (angle span threaded through recursion);
   the treemap uses `squarify` with an inset per level.
5. **Actions**: context-menu actions receive the row's absolute path.
   `trash-item` → `confirm_trash` (mandatory; Enter/Escape/Cancel do
   nothing) → `trash::trash` (gio; no deletion fallback) → snapshot
   `detach` + toast with Undo; `undo-trash` → `trash::restore` (finds the
   newest matching `.trashinfo`, refuses to overwrite or escape, removes
   the `.trashinfo` only after a successful rename).
6. **Errors** surface as toasts (AT-SPI role alert); there are no modal
   error dialogs (docs/07).

## Invariants

- **Trash safety (docs/06)**: no item ever moves to the Trash without the
  user explicitly activating "Move to Trash" in the confirmation dialog
  naming the exact item; permanent deletion is never offered.
- **Size arithmetic (docs/04)**: size = sum of `stat` sizes (blocks when
  available, apparent-size fallback), decimal units with 3 significant
  digits; item counts include the folder itself and exclude symlinks.
- **Single source of truth**: the `DisplayNode` snapshot is the only data
  structure; list, charts and totals all derive from it.

## Testing

- `src/` unit tests + `tests/scanner_tests.rs`: formatting, sorting,
  treemap invariants, settings round-trip, scanner fixtures
  (nested/hidden/symlink/loop/perm/unicode/large/disappear/cancel),
  trash record/restore logic — destructive tests only on self-created
  fixtures in fresh temp dirs with redirected `XDG_DATA_HOME`.
- GUI validation happens in a hermetic Xvfb session (`env -i`, private
  bus, fake HOME); see `IMPLEMENTATION_NOTES.md` for the harness notes.
