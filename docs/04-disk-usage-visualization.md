# 04 — Disk Usage Visualization

All items are **Observed** unless labeled otherwise. Scenario references
(S##) resolve to [09 — Test Scenarios](09-test-scenarios.md).

## 1. Layout of the result view

A result view has two panes splitting the window ≈50/50:

- **Left: item list** (a table) with columns **Folder | Size | Contents |
  Modified**. Column headers are sort controls. A header row above the table
  carries the optional infobar (§6).
- **Right: chart** — *Rings Chart* or *Treemap Chart*, chosen with two toggle
  buttons anchored at the bottom-right of the chart pane.

(S05, `screenshots/B-fixture-basic-view.png`; resize: S30/S31)

## 2. The item list

- The **first row is always the scanned location itself** (pinned; not
  affected by sorting). It shows the location name, the subtree total size,
  the total item count and its modification date.
- Every child of the current folder follows, one row each, in sort order.
- Each row shows:
  - **percent badge**: a small blue pill with a mini horizontal progress bar
    and the share of the location total, two decimals (e.g. `90.46%`;
    displayed with a leading space in the a11y tree).
  - the **name**, color-coded to match the item’s chart segment color
    (verified on the root filesystem scan: `var`, `usr`, `opt`, `etc`,
    `root`, `srv`, `mnt` each matched their ring colors). The location row
    itself appears neutral/dark.
  - **Size** and **Contents** (items) per §4, and **Modified** per §5.
  - an expander arrow (▶) for folders with children; the location row shows
    an up-arrow (^) for navigating to the parent (see `02` §3).
- Names are **ellipsized** to fit the column (e.g. `sparse-10MiB…`); the full
  name is exposed to assistive technologies.
- Rows with permission errors show the name in **red** and empty Size and
  Contents cells (see `03` §3).
- Selection: exactly one row selected at a time (blue highlight). After
  navigating into a folder, the **largest child is auto-selected**. (S12/S19)

## 3. Sorting (Observed)

- Initial order: **Size descending** (▼ on the Size header).
- Clicking a header sorts by that column; clicking the same header again
  toggles ascending/descending; the header is highlighted and shows a ▲/▼
  chevron.
- **Folder** sorts by name, case-insensitively, dot-prefixed names first
  (`.hidden-dir` < `.hidden-file` < `apostrophe's…` < `docs` < …).
- Ties (e.g. many 4.1 kB files under size sort) keep a stable but
  non-alphabetical order — tie-break rule **Unknown**; Arbor *design
  decision*: break ties case-insensitively by name.
- The location row stays pinned at the top regardless of sort.
  (S27/S28, `screenshots/C-sort-folder.png`, `C-sort-size-asc.png`,
  `C-sort-size-desc.png`)

## 4. Size and count semantics (Observed, fixture-verified)

**Displayed size of an item = the larger of (file content length, disk space
used by the file’s blocks), summed over the item’s subtree.** Concretely,
per node: `contribution = max(st_size, st_blocks × 512)`; a folder row sums
its own and all descendants’ contributions.

Verified against controlled fixtures (decimal-unit display):

| Fixture entry | content length | block usage | displayed |
|---|---|---|---|
| 256 B text file | 256 B | 4 096 B | `4.1 kB` |
| sparse 10 MiB file | 10 485 760 B | 0 B | `10.5 MB` |
| empty file (0 B, 0 blocks) | 0 | 0 | ~0 (row absent at root granularity) |
| empty dir | 40 B (dir node) | 0 B | `40 bytes` |
| folder w/ 5 120 B file | 5 120 B | 8 192 B | `8.3 kB` (= 8 192 + 60 dir node) |
| 15 000 empty files in dir | ~0 (files) | 0 B | `300.0 kB` (the directory node itself) |
| whole fixture (117 MiB apparent) | 122 297 731 B | mixed | `122.3 MB` ✓ |

**Item count ("Contents") = number of nodes in the subtree, including the
folder itself, excluding symlinks.** Verified: fixture root = 28 items
(31 real nodes − 3 symlinks); 25 000-file tree = 25 004 items (25 000 files
+ 4 folders, self included); 150 000-file tree = 150 003 items; a lone file
= “1 item”.

**Number formatting (Observed):** decimal units — `kB` = 1 000 B, `MB` =
1 000 000 B — with three significant digits (`122.3 MB`, `33.1 kB`,
`1.0 MB`, `102.4 kB`); below 1 kB the format switches to `<N> bytes`
(`40 bytes`, `14 bytes`). Negative/zero: an empty folder shows `0 bytes`–
`40 bytes` style values from its own node size.

**Arbor requirement**: implement exactly this arithmetic and formatting.
*Design decision*: compute from `stat` metadata only (no st_blocks on
platforms without it → fall back to content length there) and document the
deviation.

## 5. Modified column (Observed)

Relative day formatting: **“Today”**, **“1 day”**, **“13 days”**,
**“17 days”** were all observed. During a scan, undetermined values show
**“Unknown”**. Exact timestamps/hours were never displayed.

## 6. The infobar (Observed)

Every observed result view shows a full-width gray banner directly under the
header: **“Files may take more space than shown”**. It has no dismiss button
and appeared unconditionally in all observed result views (sparse files,
many-small-file trees, real filesystem, empty dir). Whether it is strictly
unconditional or conditional on an apparent-vs-allocated divergence is
**Unknown**; no view without it was ever observed.

**Arbor requirement**: show this exact banner in every result view, without
a dismiss control.

## 7. Rings Chart (Observed)

- Concentric rings around a **center circle that displays the location’s
  total size** (e.g. “122.3 MB”, “39.5 GB”, “33.1 kB”).
- Ring *n* shows the depth-*n* descendants; angular span of a segment is
  proportional to its share.
- Each top-level child gets a distinct color; descendants reuse the parent
  hue with variation. Segment colors match the list name colors.
- The **selected item’s segment is exploded** (translated radially outward).
- **Hover** brightens a segment.
- **Clicks on segments produced no visible effect** at three tested radii
  (file segment, folder segment, inner band): no selection change, no
  navigation. Double-click also had no effect. Interaction beyond hover is
  therefore **Unknown**; Arbor *design decision*: rings are display + hover
  only, with all interaction on the list.
  (S16b, `screenshots/C-ring-click-dir.png`)

## 8. Treemap Chart (Observed)

- Shows the **selected item’s subtree** (not the current folder): selecting
  `large` displayed its three files; selecting `l1` displayed `l2` and
  `level1-4096B.txt` nested recursively. (S19b/S20b)
- Rectangles are **area-proportional** to subtree sizes, children inset
  inside parent rectangles, distinct colors, **labels rendered when they
  fit** (`big-64MiB.bin`, `level3-16384B.txt`, `index`, …), unlabeled when
  too small.
- **Double-click on a folder rectangle navigates into it** (verified:
  breadcrumb `fixture-basic / nested / l1 / l2`). **Double-click on a file
  rectangle has no effect.** (S20c,
  `screenshots/C-treemap-folder-dclick.png`)
- Redraws on selection change, navigation, and window resize.

**Arbor requirement**: implement the treemap with the interaction rules
above; single-click on a rectangle selects the item (design decision —
single-click behavior was not separately established).

## 9. Empty and degenerate states (Observed)

- Folder with no children: one pinned row (`40 bytes / 1 item`), chart shows
  a single flat gray tile labeled with the folder name.
- After trashing a folder’s only children, the view keeps the (now stale)
  pinned row and shows the remaining rows; the chart pane can be empty or
  show the remaining tiles (see `05` §3 and S-screens).

## 10. Not observed

- Keyboard navigation inside the charts themselves — **Unknown**.
- Any accessibility names on individual chart segments — none were exposed
  in the a11y tree beyond the pane labels “Rings Chart” / “Treemap Chart”;
  chart data is effectively invisible to screen readers (Arbor requirement
  in `08`).
