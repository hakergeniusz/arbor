# 08 — Accessibility and Keyboard

Accessibility information was collected with the AT-SPI screen-reader API
against the running application (the same surface an assistive technology
sees); it is therefore legitimate black-box observation. Tooling:
`tools/a11y.py`, `tools/a11y-focus.py`.

## 1. Accessibility tree structure (Observed)

The application registers on the accessibility bus as application
**“baobab”**. Structure highlights:

- `frame` — one per window; its **name is the window title**
  (“Devices & Locations” / the location name). A hidden auxiliary frame
  named “baobab” also exists.
- Start page: `grouping "This Device"` → `list` → `list item`s, each
  containing `image`, `label "<name>"`, `label "<path>"`,
  `label "<availability>"` and a `level bar` (device usage).
- Header: `toggle button "Main Menu"`, `button "Minimize" | "Maximize" |
  "Close"`, `label` for the title, Back/Rescan buttons in result views.
- Result view: the list is exposed as a `table` with `table row`s; each row
  is fully readable as text, e.g.
  `table row "  0.92% docs 1.1 MB 5 items Today"` — the **complete row
  content is one accessible string**, so screen readers can read rows.
- Charts: the chart panes expose only the labels “Rings Chart” /
  “Treemap Chart”. **Individual segments/rectangles expose no data** —
  chart content is invisible to assistive technology.
- Dialogs: `dialog "Preferences"`, `dialog "Select Folder"`,
  `dialog "Select Location to Ignore"`; the shortcuts window exposes
  `label`s and shortcut key labels per entry.
- Toasts are exposed with role `alert` and their full text
  (“Failed to open file”, “Failed to trash file”, “Copied to clipboard”).
- Menus: `menu` → `menu item`s (item names were exposed empty in this
  environment; the visible labels are in the screenshots — a11y naming of
  popover menu items **Unknown**/environment-dependent).

**Arbor requirement**: expose at least the same fidelity: named frames,
fully readable table rows, named dialogs, role=alert toasts, and labelled
chart-mode toggles. **Design decision**: Arbor additionally exposes chart
segments with name+size to close the screen-reader gap observed here.

## 2. Keyboard map (Observed, from the shortcuts window + menus)

| Shortcut | Action |
|---|---|
| F1 | Show Help (not exercised — no help browser in session) |
| F10 | Open Main Menu |
| Ctrl+Q | Quit |
| Ctrl+? | Show Keyboard Shortcuts |
| Ctrl+, | Show Preferences |
| Ctrl+O | Scan Folder |
| Ctrl+R | Rescan Current Location |
| Escape | Close menu/dialog; **navigate back** from a result view (incl. abandoning a running scan view) |
| Tab | focus traversal (see `02` §1/§4) |
| ↑ / ↓ | move list selection without activating |
| Enter | activate focused row (start-page rows verified) |
| Right-click | context menu (mouse only; keyboard equivalent **Unknown**) |

## 3. Focus behavior (Observed)

- Focus is visibly indicated (blue outline on entries/buttons; row highlight
  for selection).
- The start-page Tab cycle and the list arrow-key selection were verified
  via a11y focus tracking (S26/S19).
- After opening a dialog, initial focus lands in the dialog (e.g. shortcuts
  window focuses its search entry; chooser focuses the file list).

**Arbor requirement**: visible focus everywhere, dialog modal focus trapping,
and the exact keyboard map above.

## 4. Gaps to close in Arbor (design decisions)

1. Chart data must be keyboard-reachable and screen-reader-reachable (the
   observed app exposes neither).
2. The context menu needs a keyboard path (Menu key / Shift+F10) and the
   trash confirmation dialog of `06` must be fully keyboard-operable with
   Cancel as default.
3. All toasts must be announced (role=alert) — matches observed behavior.
