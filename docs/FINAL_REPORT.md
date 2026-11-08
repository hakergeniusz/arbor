# Final Report — Team B, Arbor implementation

Implementation of the Team A behavioral specification (`docs/00`–`docs/11`)
as an independent Rust/GTK4 application. This report distinguishes what is
**implemented and tested**, what is **implemented but not fully verified**,
and what is **not done** — no claims beyond that.

- Head: this commit (default branch `main`, pushed to
  https://github.com/hakergeniusz/arbor — private), version 51.0.0-beta.1,
  first release: https://github.com/hakergeniusz/arbor/releases/tag/v51.0.0-beta.1
- Test suite: **38 tests, 0 failures** (24 unit + 14 scanner-integration)
- GUI evidence: ~200 screenshots from the hermetic Xvfb session
  (`/tmp/arbor-gui/shots/01…224`, referenced below as S###/A#–A# from
  `docs/09-test-scenarios.md`)
- Clean-room rules honored throughout: no Baobab source, no `tools/`
  inspection/execution. `/usr/bin/baobab` was launched exactly once per
  benchmark run as a **black box** (timing-only observation of its own
  window, §2.14) — user-authorized for that comparison, still no source.

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
- **Scan progress bar** (user request beyond the spec): a pulsing bar with
  live counters ("N items · X scanned") slides in under the infobar while a
  scan runs — visible regardless of the chart setting — and hides itself on
  finish/failure/cancel (shots 191, 194–197, 205).
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
  without confirmation (S25, screenshots 69/70). Mount-root scans display
  correctly (fixed in the fourth round): a recent for `/` used to render a
  degenerate row titled "/" with the same "/" subtitle (a scan root has no
  basename); recents now use the volume/device name for mount roots, so the
  row reads "nvme0n1p2 — /" (shot 190).

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
- **Chart pane can be disabled** (user request beyond the spec):
  Preferences → "Show chart" (persisted, default on) hides the whole right
  pane — list takes the full width, no projection or redraws happen — and
  re-enabling restores it live, mid-result (shots 143–146).
- **Large-scan chart performance** (third review round): per-node ring
  segment caches, sub-pixel arc pruning, hover repaints only when the
  hovered *segment* changes (binary-search hit-test), treemap layout cached
  against (root, size) with flat rect data instead of subtree clones,
  depth-capped projection, projection cache keyed on a tree-generation
  counter (row selection no longer re-projects anything), O(1) palette
  lookup with interned colors. On a 160k-entry fixture the UI stays at
  ~0.1 s AT-SPI round-trips through navigation, selection, chart hovers and
  treemap interaction (shots 134–142); the same view used to stall per
  mouse-move.

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
- **Undo**: toast with Undo button → newest matching `.trashinfo` located,
  item restored, `.trashinfo` removed, automatic rescan shows the item
  again (R6). Restore refuses to overwrite or escape the original parent
  (unit-tested).
- **Re-verified end-to-end in a fresh Xvfb session (2026-11-05, shots
  107–131)**: baseline → context menu (R1) → file *and* folder confirmation
  wording (R9) with full wrapped path (R2/A6) → Escape and Enter both
  cancel with zero filesystem change (A2/A3, safe default R3) → folder
  trash: `.trashinfo` (`Path=` + `DeletionDate=`), row removed, totals
  118.9 kB/5 → 106.5 kB/3 with recomputed percentages and rings (A4/R8) →
  Undo button restores + rescans (R6) → read-only Trash: "Failed to move
  file to Trash" toast, item and Trash untouched (A5/R5/R7). The Undo
  button's `win.undo-trash` action was additionally exercised directly
  (same handler the button activates). Harness note: in the WM-less
  session a toast's auto-dismiss countdown can stay paused (libadwaita
  inhibits it on focus/motion enter, and without a WM the matching leave
  never arrives) — an environment artifact, not app behavior; on a real
  desktop the countdown runs (and pauses only while hovered, as
  documented upstream).

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
("Chart — Show chart" switch + "Locations to Ignore" + Add Location),
Keyboard Shortcuts (General + Scanning groups matching docs/08 §2), About
(logo, developer, version badge with click-to-copy toast, Website/Report an
Issue/Credits/Legal), Scan Folder chooser (with "Recursively analyze mount
points" checkbox, S09).

### 2.9 Third real-desktop review round (2026-11-05, shots 132–189)
User: the UI is "clearly weird" and "the graph on right … causes immense
lag" — test all UI elements. A full control sweep in the hermetic Xvfb
session (start page incl. pixel-level usage-bar measurement, main menu,
Preferences, Keyboard Shortcuts, About incl. the version-badge clipboard
toast, chooser, result-view sorting/breadcrumb/expander/up-arrow, chart
toggles and both charts at fixture scale, context menus on rows and treemap
tiles, keyboard map, resize, Clear Recent List, Ctrl+Q, plus the full trash
A-cycle re-run) found and fixed four real defects:
- **Treemap collapsed into full-width strips**: the squarified worst-ratio
  test ignored the row total (Bruls needs `max(a·L²/A², A²/(a·L²))`), so
  rows of equal weights never broke. Now a proper grid (pinned by
  `equal_weights_break_into_a_grid_not_one_strip`,
  `dominant_item_gets_its_own_row`; shots 141 → 142).
- **Dead expander arrows**: the row ▶/▼ buttons had no click handler at all
  and `win.toggle-expand` had no activator — nothing expanded, and the
  location row's `^` did not navigate. Both wired now; `^` navigates to the
  parent via the new `win.navigate-up` (docs/02 §3, S13b/S15; shots
  149–150 vs 148).
- **Keyboard item menu on the start page**: Shift+F10/Menu with a stale
  selection from a previous scan resurrected the item menu outside any
  result view; the action now no-ops there (docs/08 §4.2; shots 183 vs
  184/185).
- **Missing About Credits row** (docs/01 §8) — added a credit section.
The usage-bar suspicion from the user's screenshot was measured and
**cleared**: fill = 413/526 px = 78.5% for a 78.7%-used disk — the bar, its
label and the spec all agree; the start page matches the reference
structure exactly (docs/01 §1–2, docs/10 state 7).

### 2.10 Fourth round (2026-11-05, shots 190–206): the "/" row, progress bar, scan cache
User report (screenshot): after scanning the root filesystem the start page
showed a degenerate recent row **"/" with subtitle "/"** — fixed via
mount-name display names (above). Two user requests implemented:
- **Scan progress bar** — pulsing bar + live "N items · X scanned" counters
  under the infobar while scanning, hidden on finish/fail/cancel. Verified
  on a real root-filesystem scan (1,099,953 items / 129.1 GB): counters
  climbed live (3567 · 51.6 MB → 1 090 212 · 121.8 GB → done), the bar hid
  itself at Finished, and AT-SPI pings stayed at 0.13–0.21 s throughout.
- **Scan cache** (Preferences switch "Cache scan results", persisted,
  default on): every finished scan streams one flat record per entry to
  `$XDG_CACHE_HOME/arbor/scans/` **from the worker thread** (no main-loop
  cost, no tree clone); re-opening a folder restores the snapshot instantly
  (files ≤ 250k nodes decode inline, bigger ones on a worker behind the
  "Loading cached scan…" spinner) and a live rescan immediately refreshes
  it — a `DirListed` for an already-measured (cache-loaded) directory
  replaces its stale children, so entries deleted since the snapshot
  disappear, and pruned folders show the honest amber "unmeasured" state
  until their fresh aggregates arrive. Verified end-to-end on the root
  scan: re-open restored the view instantly from the 57 MB snapshot,
  the refresh converged back to exactly 129.1 GB / 1,099,953 items with
  rings redrawn and the bar auto-hidden (shots 198–206), including across
  an application restart. Unit-tested: record/decode roundtrip with
  finalized aggregates, garbage/foreign-path rejection, `collect_paths`,
  and a real cached-scan roundtrip through `spawn_ext`.
  *(Superseded by §2.13: the automatic refresh-after-restore was removed —
  a restore no longer rescans; ↻/Ctrl+R do, bypassing the cache read.)*

**Follow-up polish (same day, shots 207–209)**: two user-flagged
mismatches fixed — (1) scanning a mount root recorded a **recent row
duplicating the permanent device row** ("nvme0n1p2" twice: drive icon
with stats vs bare folder row); mount roots and the home folder no
longer enter the recents list, and state written earlier is migrated
cleaned on the next launch (shot 207). (2) The treemap's per-level
3 px insets + 1.5 px strokes stacked into pale moats between tile
groups ("gaps are too big") — insets halved to 1.5 px and strokes to
1 px, tiles now separate by ~2–3 px hairlines (shot 209 vs the user's
screenshot).

### 2.11 Fifth round (2026-11-05, shots 210–214): disc glyphs and list density
Both items of the user's "neither were done" report, this time with
numeric pixel evidence instead of eyeballed crops:
- **Disc icons now sit centered in their circles.** Root cause found in
  two layers: (a) the disc was a GtkBox, which start-packs a
  non-expanding child — the 18px glyph kept an 18px allocation pinned
  to the box origin, where halign Center is a no-op; (b) the first fix
  attempt (Overlay) was never actually linked into the tested binary
  (a 0.04 s "Finished" no-op build), which is why two rounds rendered
  identically. The disc is now a GtkOverlay carrying the circle as its
  own background with the glyph as an *overlay* child, which
  libadwaita positions strictly by halign/valign. Pixel measurement
  across all six start-page rows: glyph center == circle center,
  dx=0.0/dy=0.0, discs exactly 40×40 (shot 212 + zoom crop).
- **Denser result-list rows.** Measured folder-list row pitch was
  50 px, driven by theme min-heights and expander-button padding.
  Columnview rows and the row-expander buttons now have zero
  min-height/vertical padding; pitch is uniformly 34 px (−32 %,
  shot 214 vs shot 200, same measurement script on both).

### 2.12 Release 51.0.0-beta.1 (2026-11-05, shots 215–218)
Default branch switched to `main`; tag `v51.0.0-beta.1`; GitHub
pre-release with five artifacts, all built by committed scripts
(`packaging/scripts/`):
- **Arch** `.pkg.tar.zst` (makepkg from the tag tarball) — contents
  verified via `pacman -Qlp`, and the packaged binary was run from an
  extracted staged root in the Xvfb session (shot 215/216: full result
  view, CLI path open).
- **Debian** `.deb` — real `dpkg-deb` inside an Ubuntu 25.10 chroot
  (trixie's libadwaita 1.7.6 is below the crate's ≥ 1.8 floor);
  computed `Depends` floors: libc6 ≥ 2.35, libgtk-4-1 ≥ 4.20.1,
  libadwaita-1-0 ≥ 1.8.0.
- **Fedora** `.rpm` — `rpmbuild` inside fedora:latest (`51.0.0-0.1.beta1`
  pre-release versioning); `%check` (cargo test) ran **in the chroot and
  caught a real test flake**: two independent walks of a live ext4
  disagree while delalloc is pending, so `settle_fs()` now fsyncs
  fixtures before size comparisons (btrfs inline extents had masked
  this on the host).
- **Flatpak** `.flatpak` single-file bundle (flatpak-builder,
  org.gnome.Sdk//51) — installed `--user` and **run under the Xvfb
  session from inside the sandbox** (shot 217: scans the passed host
  path, renders list + rings; `--share=network` was required for the
  crates.io fetch).
- **Portable** `.tar.zst` (Ubuntu 25.10-built stripped binary +
  license/desktop/icon) — extracted and run under Xvfb (shot 218).

### 2.13 Sixth round (2026-11-08, shots 220–224): cache restore no longer rescans
User report: **"The caching does not work. It always recalculates it."**
Root cause in `open_location`: a cache hit installed the snapshot and then
unconditionally called `start_scan(path, true)` — a full live rescan with a
"Rescanning…" bar on every open, so the cache only ever *seeded* the rescan
and never saved work. The Rescan button and Ctrl+R (`app.rescan`) also
routed through `open_location`, i.e. through the cache read. Fix:
- a cache hit (sync or worker-thread decode) now installs the snapshot and
  finalizes the view exactly like a completed scan (rows, charts, recents
  bump, largest-child selection) and **stops there** — no automatic rescan;
- new `Window::rescan_current()` for the Rescan button and Ctrl+R: a true
  from-disk rescan that never reads the cache;
- `install_cached` clears stale expansion state, bumps recents and toasts
  "Restored scan from cache (HH:MM)" so a stale snapshot is never mistaken
  for a fresh scan; `start_scan` lost its now-unused `refresh` branch.
Verified in Xvfb (hermetic session): first launch scans and writes the
cache; relaunch restores the full result in ~1 s with the toast, no
spinner, and the cache file's mtime **unchanged** (no scan, no rewrite);
after adding a file to the fixture the restore still shows the stale
snapshot while Ctrl+R picks the new file up (28→29 items) and rewrites the
cache (981→1020 B). 38 tests green (commit `9a2cd01`).

### 2.14 Benchmark: Baobab 50.0 vs Arbor, cold and warm (2026-11-08)
User request: *"So compare speed of baobab and arbor, both on cold and warm
measuring."* Identical setup for both apps: hermetic Xvfb session (`env -i`,
private dbus, fake HOME/XDG, no window manager), the same CLI argument (a
fixture of 1250 dirs × 80 files = **101,251 items / 645.8 MB** on ext4),
`GSK_RENDERER=cairo` (the Vulkan renderer stalls frame presents on
Xvfb/llvmpipe — a harness artifact that froze *either* app mid-scan), a11y
bridge disabled and no AT-SPI polling (16 Hz accessibility tree walks
congest Baobab's main thread mid-scan — another harness artifact). Cold =
page cache dropped via `sudo sync; echo 3 > /proc/sys/vm/drop_caches` plus
Arbor's scan cache cleared; warm = immediately re-opening the same folder
with everything hot. Timing from 4 fps x11grab recordings of the window
region with a pairwise frame detector: the last frame pair 2 s apart (8
frames at 4 fps) with a pixel diff > 8/255 among frames carrying real
content (std > 15), wall = rec time + the 0.25 s launch calibration offset.
An earlier frame-stability detector (4-level-quantized hash stable for 3 s,
input-seeking probes against the final frame) over-reported every cell by
~1–2 s — mp4 P-frame noise of ±4–6/255 defeats 4-level quantization, frames
pushed after SIGKILL are black, and the final frame carries encoder-flush
blips — so every number below (and in §2.15) is re-measured with the
corrected detector on the same recordings. Arbor-warm is additionally
cross-checked by a dark-rect probe for the restore toast. Baobab strictly
black box. Three runs per cell.

| Scenario | Baobab 50.0 | Arbor (before §2.15) | Arbor detail |
|---|---|---|---|
| **Cold open** (fresh scan) | 2.25 s (2.25 / 2.0 / 2.25) | 5.25 s (5.25 / 5.75 / 5.0) | scanner + cache write **done at 1.3 s** (1.27–1.36) |
| **Warm open** (same folder again) | 1.0 s ×3 — full rescan from disk | **≈0.55–0.85 s** | cache restore, **no rescan** (cache mtime untouched in all 3 runs) |

Reading:
- Arbor's scanner alone beats Baobab's whole cold path — all 101k items are
  measured and cached at 1.3 s vs 2.25 s — but the UI then ingests the result
  incrementally (bounded 6 ms/tick drain, throttled repaints, chart
  projection of 101k nodes) and only settles at ~5.25 s. The cold tail is
  UI ingest, not scanning; §2.15 profiles and removes it.
- Warm, the §2.13 fix pays off: Arbor skips the scan entirely and shows the
  restored snapshot in ~0.7 s, faster than Baobab, whose warm open is the
  same full scan, merely page-cache-hot (1.0 vs 2.25 s).
- Caveats: software rendering (llvmpipe) — absolute numbers are not
  native-GPU numbers, the *ratios* are the comparable part; single-fixture
  workload; "UI-complete" is strict (rows, rings and treemap all settled,
  spinner gone). Recordings `rec-{fc,fw,gc,gw}{1..3}.mp4`.

### 2.15 Seventh round (2026-11-08): ingest optimization — cold settle 5.25 s → 1.75 s

User request: *"So optimize it."* Target the §2.14 finding that Arbor's
scanner finishes at ~1.3 s but the UI settles 4 s later. Temporary env-gated
instrumentation (`ARBOR_DRAIN_DEBUG`, removed before commit) attributed the
tail on the 101k fixture to ≈2.4 s of main-thread work delivered at a 37.5%
duty cycle (6 ms of work per 16 ms timeout tick, running *above* the redraw
priority):

| Sink | Measured |
|---|---|
| Streaming repaint `set_rows` (28 × 52.5 ms) | **1470 ms** — dominant |
| Drain merge work (2503 scan messages) | 561 ms |
| Streaming repaint `build_rows` (28 × ~14 ms) | 393 ms |

Two commits, no behavior change (120 ms repaint throttle, progressive fill,
spinner, amber unmeasured rows, autoselect-all all preserved):

- `perf(models)` (`5301080`): a per-`DisplayNode` child name index (built once
  a folder crosses 32 children) makes path descents O(depth) instead of
  O(siblings) — the fixture's 1250 root children made every `DirMeasured`
  descent and every `merge_entries`/prune parent lookup walk ~625 siblings;
  `merge_entries` consumes `EntryMeta` by value; the scanner's dead
  per-directory `DisplayNode` build removed.
- `perf(ui)` (`b163724`): the drain moved to `glib::idle_add_local`
  (DEFAULT_IDLE sits below redraw/input, so it consumes only spare main-loop
  time) with a 10 ms per-dispatch budget and a 16 ms one-shot re-arm timeout
  for when data reappears; `set_rows` via `ListStore::splice` (one
  `items_changed` per rebuild instead of 1252); one `now_local()` per rebuild
  and decorate-sort-undecorate with cached keys in `emit_children`; settle
  housekeeping (`list_mounts`, recents, start-page rebuild, the synchronous
  settings write) deferred off the `Finished`/`install_cached` critical path
  into a one-shot idle after the final paint.

Result (same harness/fixture as §2.14, cold page cache, 3 runs per cell):

| Scenario | Baobab 50.0 | Arbor before | Arbor after |
|---|---|---|---|
| **Cold open** | 2.25 s | 5.25 s | **1.75 s ×3** — now ahead of Baobab |
| **Warm open** | 1.0 s ×3 | ≈0.55–0.85 s | **≤0.55 s ×3** (toast already present in the first probeable frame; negative `cache_done` deltas prove no rescan) |

The scanner is unchanged (1.20–1.25 s cold) — the whole win is ingest.
Mid-scan frames confirm progressive fill still streams (71,090 items visible
at ~1.25 s vs 19,730 at the same point before the change) with the spinner
animating, i.e. the idle drain does not starve rendering. The cancel path was
re-verified on the new drain: Escape at 0.6 s on a cold run exits the scanner
thread (1 → 0) with no panic, and the `.tree.tmp` leftover matches the
documented cancel invariant (the writer is never `finish()`ed on cancel).
39 tests green (24 unit + 15 integration), including a new
`child_index_survives_prune_and_remerge` that replays the exact measure →
prune → re-merge sequence the refresh path exercises on an indexed folder.

### 2.16 Release 51.0.0-beta.2 (2026-11-08)
Tag `v51.0.0-beta.2`; GitHub pre-release with the same five artifacts +
`SHA256SUMS.txt`, all rebuilt by the committed `packaging/scripts/`
pipeline. Ships both perf rounds (`5301080`/`b163724`, §2.15 — cold open
settle 5.25 → 1.75 s) and the sixth-round cache fix (`9a2cd01`).
- **Arch** `.pkg.tar.zst` — makepkg from the tag tarball; the packaged
  binary is run from an extracted staged root in the Xvfb session.
- **Debian** `.deb` — real `dpkg-deb` in the Ubuntu 25.10 chroot;
  `Depends` floors unchanged (libc6 ≥ 2.35, libgtk-4-1 ≥ 4.20.1,
  libadwaita-1-0 ≥ 1.8.0); package contents/depends inspected in-chroot.
- **Fedora** `.rpm` — `rpmbuild` in fedora:latest (`51.0.0-0.2.beta2`
  pre-release versioning); `%check` (cargo test) green in the chroot.
- **Flatpak** `.flatpak` (flatpak-builder, org.gnome.Sdk//51) — the
  manifest's git source now pins the released commit hash; installed
  `--user` under a throwaway `XDG_DATA_HOME` and run inside the sandbox
  in the Xvfb session.
- **Portable** `.tar.zst` — extracted and run under Xvfb.

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

- **Packaging** — **built and released as of 51.0.0-beta.2** (see §2.12, §2.16):
  all five targets were compiled and their artifacts exercised; what
  remains untested is *installation on the target distributions
  themselves* (the .deb was never installed into a real Debian/Ubuntu
  system, the .rpm into real Fedora — only built and inspected).
- **aarch64**: all targets declare it (`arch: any`,
  `%{rust_tier1_arches}`, `arch=('x86_64' 'aarch64')`), but only x86_64
  was compiled and run locally.

## 5. Not implemented / deliberate deviations

- Nothing from the specification was consciously omitted. Deviations from
  the *plan* (not the spec): `fsops.rs` folded into `trash.rs` + UI
  actions; `treemap.rs` merged into `ui/charts.rs`; `ui/confirm.rs` added
  for the mandatory dialog; `libc` is a regular dependency (`geteuid`).
- **Chart pane optional** (user request beyond the spec): Preferences
  "Show chart" (default on, persisted) hides the rings/treemap pane; when
  off, the result view is the full-width list per the same anatomy.
- **Recents exclude mount roots and the home folder** (user request beyond
  the spec's letter): both have permanent rows in "This Device"; recording
  them produced duplicated, differently-styled rows.
- About identity: the dialog and menu say **Arbor** (clean-room app,
  developer hakergeniusz) rather than the reference's GNOME branding —
  same row structure as docs/01 §8.
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
