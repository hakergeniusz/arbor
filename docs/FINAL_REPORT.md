# Final Report — Team B, Arbor implementation

Implementation of the Team A behavioral specification (`docs/00`–`docs/11`)
as an independent Rust/GTK4 application. This report distinguishes what is
**implemented and tested**, what is **implemented but not fully verified**,
and what is **not done** — no claims beyond that.

- Head: `a08c2be` (branch `implementer/arbor`, pushed to
  https://github.com/hakergeniusz/arbor — private), version 1.0.0
- Test suite: **29 tests, 0 failures** (16 unit + 13 scanner-integration)
- GUI evidence: ~100 screenshots from the hermetic Xvfb session
  (`/tmp/arbor-gui/shots/01…106`, referenced below as S###/A#–A# from
  `docs/09-test-scenarios.md`)
- Clean-room rules honored throughout: no Baobab source, no `tools/`
  inspection/execution, `/usr/bin/baobab` never launched.

## 1. Method

- **Unit/integration (no GUI)**: `cargo test` — formatting arithmetic,
  sorting comparators, treemap layout invariants, settings round-trip,
  scanner fixtures (nested/hidden/symlink/loop/permission/empty/unicode/
  large/disappearing-file/cancellation), trash record matching and
  restore refusal logic, `DisplayNode::detach`.
- **GUI (Xvfb, hermetic)**: every launch ran with `env -i` — private
  dbus, fake `HOME`, redirected XDG dirs — so neither the real display nor
  the real Trash could ever be touched. Interactions via xdotool; frames
  via ffmpeg x11grab; accessibility via python3-Atspi with a manually
  started at-spi bus + registry.
- **Destructive tests**: exclusively on fixtures the test itself created
  inside `/tmp/arbor-gui/` (per the user's standing instruction); the
  real home directory was never a target. Trash state lived under the
  fake `XDG_DATA_HOME`.

## 2. Implemented and verified

### 2.1 Scanning (docs/03)
- Async scan on a worker thread with progress events; UI fills
  progressively mid-scan; totals grow live (observed in GUI, Phase 3).
- Snapshot semantics: the finished scan is one immutable tree; mid-scan
  changes appear only after rescan (S14).
- Hidden files included; **symlinks excluded, cycle-safe** (unit tests
  with loop fixtures); permission-error nodes render as error rows
  (ERROR_COLOR) — unit-tested.
- Cancellation: dropping the scan state cancels and joins the worker
  (unit-tested); triggered by navigation away and window close.
- Size arithmetic (docs/04): block-usage with apparent-size fallback,
  decimal units, 3 significant digits, `<1 kB` → bytes — unit-tested and
  GUI-spot-checked (999 B file shows `4.1 kB`).

### 2.2 Start page & navigation (docs/01/02)
- Devices & Locations list: Home Folder, mounts with usage bars
  (nvme0n1p2/nvme0n1p1), Recent list with thumbnails-less folder rows.
- Rows activate by mouse **and Enter**; **Tab cycle verified** (row1 →
  row2 → … → Main Menu → wrap, S26/S27).
- Back button and location row return to the start page / parent folder.
- Recents recorded on navigation; **Clear Recent List works immediately**
  without confirmation (S25, screenshots 69/70).

### 2.3 Result view (docs/04/10)
- Header per docs/10 §2: `[‹ Back] [wide centered breadcrumb pill] [↻]
  [window controls]` — **no Main Menu button on the result view** (F10 is
  a no-op there; the menu lives on the start-page header, where it
  returns on Back — verified in the final round, shots 102–106).
- Breadcrumb pill: wide, centered (hexpand + AdwClamp), icon at its start.
- "Files may take more space than shown" infobar, spinner column,
  sortable Folder/Size/Contents/Modified columns with **working
  header-click sorting** (Size desc default; mirrored app-side sibling
  sort; location row pinned first).
- Row anatomy per docs/10 §3: percentage **inside** a single dark
  fill-pill badge (full-size ProgressBar under a centered label), plain
  name color, permission-error names red, and folders rendered **amber
  while their subtree is still being measured** (observed reference
  behavior — visible during scans of the real home, shots 102/94; they
  settle to the plain look once measured). The accessible row name leads
  with the percentage ("37.05% Projects 79.6 GB …").
- **Rings chart**: 12-o'clock origin, clockwise, children subdividing the
  parent arc, center total, hover brightening (verified via pixel
  sampling), correct wedge geometry for the deep-nesting fixture; drawn
  for a 2.2M-item home scan (shot 103).
- **Treemap**: squarified layout with per-level inset, labels when they
  fit, selection highlight, **treemap follows list selection** (S20b),
  double-click navigates into the deepest folder region, right-click
  opens the item menu; correct squarified orientation at real-home scale
  (shot 105).
- Chart switcher buttons (Rings/Treemap) with correct active styling.

### 2.4 Actions (docs/05)
- Context menu (Open Externally / Copy Path to Clipboard / Move to
  Trash) from **list rows, the location row, and treemap rectangles**,
  plus the keyboard path (Menu / Shift+F10) anchored at the focused row.
- Open Externally launched the default handler (positive path observed);
  folder targets show the exact "Failed to open file" toast.
- Copy Path puts the path on the clipboard with its toast.
- **No stale-totals bug**: unlike upstream's observed S14b, Arbor
  refreshes totals — this was a deliberate spec-required difference
  (docs/05 §"Arbor requirement"), verified in the A-matrix.

### 2.5 Trash safety (docs/06) — the safety-critical core
Full **A1–A8 acceptance matrix** on self-created fixtures:
- A1 confirmation opens with **zero filesystem change** while open;
  dialog shows exact Folder/File + full Path (wrapped, never truncated —
  R2/A6), explanatory text, destructive red "Move to Trash".
- A2/A3 **Enter and Escape both cancel** (default response Cancel, R3);
  Cancel click cancels; no side effects on any dismissal (R4).
- A4 confirm → item in XDG Trash with valid `.trashinfo` (`Path=`,
  `DeletionDate`); row removed; **totals refreshed** (8.3 kB/4 items →
  4.1 kB/2 items, R8); rings and treemap update.
- A5 read-only Trash → "Failed to move file to Trash" toast, item
  intact, **no deletion fallback ever** (R5/R7).
- A6/A7 exact-item confirmation (trash A but confirm shows A, not a
  sibling); files use "Move file to Trash?" (R9).
- **Undo**: toast (timeout 5 s) with Undo button → newest matching
  `.trashinfo` located, item restored, `.trashinfo` removed, automatic
  rescan shows the item again (R6). Restore refuses to overwrite or
  escape the original parent (unit-tested).

### 2.6 Keyboard & accessibility (docs/08)
- Full keyboard map verified in GUI: F1 (accel registered), F10 main
  menu (S02 contents + accelerators shown), Ctrl+comma Preferences,
  Ctrl+? Shortcuts (via menu; see §4), Ctrl+O Scan Folder, Ctrl+R
  rescan (registered + action shared with the toolbar button), Escape
  closes menus/dialogs, Tab/arrows/Enter as above.
- **AT-SPI tree verified**: complete row names as one readable string
  (`98.56% note-999B.txt 4.1 kB 1 item Today` — spec format, §1);
  charts exposed as `[image]` with Label + live Description summaries;
  toast overlay exposes **role=alert**; header buttons named (Back,
  Main Menu, Rescan, window controls).
- Error toasts are alerts; no modal error dialogs anywhere (docs/07).

### 2.7 Window behavior (docs/01 §5)
- Live resize verified: result view splits **exactly 50/50** at 600×400
  and 400×300 with the chart redrawing; start page at 400×300 keeps the
  title next to the Main Menu button, truncates usage bars, remains
  fully operable — no minimum-size wall (S30–S33).
- Window size persists on close and restores on next launch (wired via
  close-request; persistence logic unit-tested).

### 2.8 Dialogs
All opened and screenshot-verified in the hermetic session: Preferences
("Locations to Ignore" + Add Location), Keyboard Shortcuts (General +
Scanning groups matching docs/08 §2), About (logo, developer, version,
Website/Report an Issue/Legal), Scan Folder chooser (with "Recursively
analyze mount points" checkbox, S09).

## 3. Implemented, partially verified

| Area | State | What is missing |
|---|---|---|
| Large-tree GUI behavior | **Verified at home scale**: a 2,206,156-item / 294.3 GB home scan completes with a live UI — AT-SPI pings stay at 0–45 ms throughout (the unfixed build locked up at second 16), rows/amber state stream in progressively, rings and treemap render at full scale (shots 102–105). Fixes: `8c9a923` (bounded drain, scanner-side coalescing, throttled repaints) and `a08c2be` (O(1) merge duplicate guard + parent grouping — the per-entry linear scan made flat directories O(n²) on the UI thread; single-descent measured-delta walk; chart projection skipped while hidden behind the spinner page). Merge behavior pinned by `coalesced_stream_replays_to_the_final_tree`, `flat_directory_merges_in_linear_time`, `merge_entries_is_idempotent_per_path` | Even larger trees (>10⁷ items) untested; peak RSS for the 2.2M-item tree was ~3 GB — no eviction/summarization exists (same as the reference for an immutable snapshot) |
| Preferences ignore-list effect | Dialog + persistence implemented; settings round-trip unit-tested; ignore filtering itself unit-tested (`ignored_subtrees_are_excluded`) | No GUI test of an ignored location being excluded from a scan (chooser-typing is unreliable in the harness) |
| Multi-device trash undo | Candidate-dir walk unit-tested (same-device topdirs, `.Trash-$uid`) | No real second-partition Trash exercised |
| Help failure toast | F1 **positively verified** in GUI: the platform default URL handler launches the help page (it did open the machine's handler in the sandbox) | The "Failed to open help" toast needs a handler-less machine — failure branch logic-reviewed only |

Moved to §2 (fully verified on `f53c520`): chooser end-to-end pick (sidebar →
select folder → Open → scan starts, S09) *and* its invalid-folder branch
(exact `… is not a valid folder` toast); Ctrl+? direct accel (Keyboard
Shortcuts window opens); single-instance `arbor <folder>` (one process, the
existing window navigates, S06).

## 4. Implemented but not runtime-verified

- **Packaging**: `packaging/debian/` (control/rules/changelog/copyright),
  `packaging/rpm/arbor.spec`, `packaging/arch/PKGBUILD`,
  `packaging/flatpak/io.github.hakergeniusz.Arbor.json`. No
  dpkg/rpmbuild/makepkg in the build environment and flatpak-builder
  needs network — manifests are written and reviewed, **not built**.
- **aarch64**: all targets declare it (`arch: any`,
  `%{rust_tier1_arches}`, `arch=('x86_64' 'aarch64')`), but only x86_64
  was compiled and run locally.

## 5. Not implemented / deliberate deviations

- Nothing from the specification was consciously omitted. Deviations from
  the *plan* (not the spec): `fsops.rs` folded into `trash.rs` + UI
  actions; `treemap.rs` merged into `ui/charts.rs`; `ui/confirm.rs` added
  for the mandatory dialog; `libc` is a regular dependency (`geteuid`).
- One deliberate deviation from the *spec text* (docs/10 §5 "List name
  colors match segment colors"): names render plain, matching the
  observed reference application itself (user-supplied real screenshot,
  second review round) — names are colored only for permission errors
  (red) and while a folder is still being measured (amber).
- Known environment caveat: with the Trash directory made read-only the
  failure is reported as a toast (R5/R7); behavior with a *foreign*
  topdir (different device, no `.Trash-$uid`) follows the same candidate
  logic and refuses gracefully, but was only unit-tested.

## 6. Phase commits

| Phase | Commit |
|---|---|
| 1+2 | `89b746a` docs: implementation plan; `2a9cd38` chore: initialize Rust application |
| 3 | `3214c8a` feat(scanner): add asynchronous directory scanning |
| 4 | `082be0f` feat(visualization): implement rings and treemap charts |
| 5 | `2fbc30e` feat(actions): add context menu and mandatory trash confirmation |
| 6 | `df1fa30` feat(a11y): keyboard menus, accessible rows and chart descriptions |
| 7 | `e507091` build: packaging, license audit and project documentation |
| fix | `8c9a923` fix(scan): keep the UI responsive on large scans (first real-desktop review) |
| fix | `a08c2be` fix(scan,ui): survive multi-million-entry scans; match reference result view (second real-desktop review) |

## 7. Safety statement

**No folder is moved to Trash unless the user explicitly activates "Move
to Trash" in the confirmation dialog naming the exact item.** Permanent
deletion is never offered anywhere in the UI or code paths (`gio::trash`
only, no unlink fallback). All destructive verification ran on
self-created fixtures inside disposable temporary directories.
