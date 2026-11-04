# 03 — Directory Scanning

All items are **Observed** unless labeled otherwise. Scenario references
(S##) resolve to [09 — Test Scenarios](09-test-scenarios.md).

## 1. Entry points for a scan

1. **Start-page rows** — Home Folder, each mounted filesystem, and each
   recent folder row. (S05, S10)
2. **Main Menu → Scan Folder… / Ctrl+O** — opens a **“Select Folder”**
   chooser dialog. (S09)
3. **Command line** — `baobab <folder>`: if the app is not running it starts
   scanning `<folder>`; if it is running, the existing window navigates to
   and scans `<folder>` (single-instance behavior). (S06)
4. **Rescan** — the header button (↻) and Ctrl+R re-scan the current
   location. (S23)

### The “Select Folder” chooser (Observed)

- Standard GTK folder chooser: sidebar (Recent, Home, Desktop, bookmarks,
  “Other Locations…”), file list with Name/Size/Type/Modified columns,
  **Cancel** and **Open** buttons.
- **Baobab-specific extra**: a checkbox **“Recursively analyze mount
  points”** at the bottom of the dialog, unchecked by default.
  (S09b, `screenshots/B-chooser-typed.png`)
- Ctrl+L opens a location entry with path completion; typing a full folder
  path and confirming navigates into it, and confirming again (or pressing
  **Open**) starts the scan.
- If the entered location is not a valid folder, the dialog closes and the
  main window shows a toast: **“\<path\>” is not a valid folder**” with an
  embedded dismiss (✕) button. (S09c,
  `screenshots/B-scan-dialog-invalid-path.png`)

**Arbor requirement**: provide a folder chooser with Cancel/Open, a
“Recursively analyze mount points” checkbox, and the same invalid-path toast.
*Design decision*: “Recursively analyze mount points” checked = crossed
filesystem boundaries during the scan (mounts included as ordinary folders
instead of being excluded); the checkbox state is remembered per user.

## 2. Scan progress presentation (Observed)

From scan start to completion:

- The chart pane shows a **spinner** (an animated arc) instead of a chart.
  (S05, `screenshots/B-after-home-scan-enter.png`)
- The item list is **populated progressively**: rows appear as subtrees are
  measured; sizes/counts update in place; the root row’s total changes while
  scanning (observed: root row “4.1 GB / 30744 items” → “39.5 GB /
  516810 items” as the scan progressed).
- The **Modified** column shows **“Unknown”** for rows whose mtime is not yet
  determined during the scan.
- No progress percentage, progress bar, or cancel button is shown.
- On completion the spinner is replaced by the active chart (rings or
  treemap) with no completion toast or notification.

**Arbor requirement**: show a spinner in the chart pane, fill the list
progressively, show “Unknown” in undetermined Modified cells, and swap the
chart in on completion. No cancel control is required by observation, but
see §5.

## 3. Scan semantics (Observed)

- **Snapshot semantics**: the result reflects the tree as it was read. A file
  created while a scan was running did **not** appear in the results
  (a 25 004-item scan stayed at 25 004 items with the mid-scan file absent);
  a file deleted mid-scan was still included if it had already been read.
  Changes require a **Rescan**. (S21, `screenshots/B-bigtree-after-midscan.png`)
- **Hidden files are included**: dot-prefixed files and folders appear as
  rows and are counted in totals and item counts (`.hidden-file`,
  `.hidden-dir` observed with correct values). No “show hidden” toggle
  exists.
- **Symbolic links are excluded**: symlinks are not followed, not listed as
  rows, and not counted. Verified three ways: (a) a symlink-to-folder did not
  double-count the target (fixture total matches non-following arithmetic),
  (b) no `link-to-file`/`link-to-dir`/`broken-link` rows appear, (c) a
  symlink cycle (`inner/loop-up -> ..`) did not hang or recurse — the cycle
  fixture scanned without incident. (S20)
- **Empty directories** scan fine: a folder with no children shows a single
  row for itself, “40 bytes / 1 item”, and a chart of one flat tile.
  (S18, `screenshots/B-empty-dir-scan.png`)
- **Permission errors** (unreadable subdirectory, chmod 000): the scan does
  not fail; the unreadable folder appears as a row with its name shown in
  **red** and **empty Size and Contents cells**; it still counts as an item
  and its own (parent-side) contribution is included in totals. No error
  dialog is shown. (S17, `screenshots/B-perm-error-scan.png`)
- Item counts include the folder itself and exclude symlinks (see `04` §4
  for the exact arithmetic verified against fixtures).

**Arbor requirement**: implement the snapshot semantics, hidden inclusion,
symlink exclusion (including cycle safety), and the permission-error row
presentation above. Never follow symlinks when measuring.

## 4. Error surfaces related to scanning

- Invalid folder path → toast in main window (§1).
- Unreadable subtree → red name + blank cells (§3).
- No other scan-error surfaces were observed. Behavior for a scan target that
  itself is unreadable or was deleted between selection and scan start:
  **Unknown**.

## 5. Cancellation (Observed / Unknown)

- **No cancel control exists**: no button, menu item, or shortcut labeled
  cancel; the shortcuts window lists none.
- The only observed way to leave a running scan is **Escape / Back**, which
  navigates away from the result view (verified mid-scan). Whether the scan
  continues internally afterwards is **Unknown** (unobservable).

**Arbor requirement**: Escape and Back must leave a running scan view
immediately. *Design decision*: Arbor additionally stops the background scan
when its view is closed or replaced, and frees the partial data.

## 6. Recents update (Observed)

Every completed scan of a folder adds/updates the folder’s entry at the top
of the start-page recents list (§2 of `01`). Device and Home rows are fixed
and never enter the recents list.

## 7. Not observed

- Scanning network locations or trash:///-style URIs — **Unknown**.
- Behavior when the scanned tree is modified during rescan — same snapshot
  semantics assumed; not separately re-verified.
- The practical effect of the “Recursively analyze mount points” checkbox
  (see §1) — **Unknown**.
