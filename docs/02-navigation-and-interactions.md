# 02 — Navigation and Interactions

All items are **Observed** unless labeled otherwise. Scenario references
(S##) resolve to [09 — Test Scenarios](09-test-scenarios.md).

## 1. Focus order on the start page (Observed)

Repeated Tab presses cycle focus through exactly three stops:

1. the first device/location row (Home Folder),
2. the second row (root filesystem),
3. the Main Menu button — then wrapping back to the first row.

With more rows (recents present) the same pattern extended row by row
(a11y focus tracking; row names are empty in the a11y tree but rows are
distinct focus stops). (S26)

**Arbor requirement**: all interactive elements are reachable with Tab in
reading order; the start page order is rows top-to-bottom, then Main Menu.

## 2. Activating locations

- **Clicking (or keyboard-activating) a start-page row** opens the result
  view for that location and starts a scan (see `03`). (S05/S10)
- Clicking a **recent** row re-scans and re-opens that folder the same way.

## 3. Row interaction semantics in the result view (Observed)

The item list behaves asymmetrically for folders and files:

| Interaction | Folder row | File row |
|---|---|---|
| Single click on the row body | **Navigates into** the folder (list and chart re-center on it; breadcrumb extends) | **Selects** the row (highlight; ring segment explodes; treemap switches to show its parent selection content — see `04`) |
| Single click on the expander (▶) | Expands/collapses children inline (indented sub-rows, without navigation) | n/a |
| Double click (list) | Navigates into the folder | no effect observed |

(S12–S14, `screenshots/C-docs-navigated.png`, `C-click-file-row2.png`,
`C-expander-l1.png`)

- The **first row** of a result view is the scanned location itself; its
  expander area shows an **up-arrow (^)** which navigates back to the parent
  folder. (S15)
- The **Back button** (‹) in the header returns to the start page
  (Devices & Locations) from any result view. (S11)
- The **breadcrumb in the location pill**: every ancestor segment is
  clickable and navigates to it (verified: `fixture-basic / docs` → click
  `fixture-basic` → back at the fixture-basic root). (S16)

**Arbor requirement**: implement exactly this asymmetry (folder click =
navigate, file click = select, expander = inline expand, up-arrow row,
Back → start page, clickable breadcrumb segments).

## 4. Keyboard interaction in the result view (Observed)

- **Arrow Up/Down** move the selection between rows **without activating**
  them (verified: selection moved from a file row onto the folder row `l1`
  without navigating into it; the treemap followed the selection). (S19)
- **Tab** moves focus among interactive elements (rows are focusable).
- **Enter/Space** on a row activates it with the same semantics as a click
  (Enter was used to activate the start-page device row; in-list Enter was
  not separately exercised — **Unknown**, expected to mirror clicks).
- **Escape** in a result view navigates **back** (verified mid-scan: Escape
  left the running scan view and returned to the start page). (S22)

## 5. Chart mode toggle (Observed)

- Two toggle buttons at the bottom of the chart pane: **Rings Chart** and
  **Treemap Chart** (with icons). The active one is visually emphasized.
- Switching is instant, no re-scan.
- The chosen mode is **per-window state, not per-location**: after switching
  to Treemap in one location, navigating to another location (root
  filesystem) kept Treemap active. (S20)
- The mode was not observed to persist across application restarts —
  **Unknown**.

## 6. Hover behavior (Observed)

- Ring-chart segments brighten on pointer hover (teal → bright green).
- List rows show hover highlight.
- No hover tooltips were observed anywhere (ring segments, rows, header) —
  **Unknown** whether tooltips exist on longer hover.

## 7. Menus and dialogs

- Context menu on list rows: see `05`.
- The Main Menu popover: see `01` §4.
- Dialogs (Preferences, About, chooser) are modal to the window and close on
  their ✕ button or Escape.

## 8. Window title behavior (Observed)

The window title (WM title / a11y frame name) always mirrors the current
view: “Devices & Locations” on the start page; the location display name in a
result view (e.g. “fixture-basic”, “acer-archlinux”, “big-tree2”).

**Arbor requirement**: keep the window title in sync with the current
location; use the location’s display name (basename for folders, label for
devices, “Home Folder” for home).
