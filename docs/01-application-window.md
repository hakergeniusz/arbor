# 01 — Application Window

Observation environment: GNOME Baobab 50.0 on a 1440×900 Xvfb display, fresh
profile (first run). Every statement below is classified per
[00 — Overview](00-overview.md). Scenario references point into
[09 — Test Scenarios](09-test-scenarios.md).

## 1. Startup and first-run state (Observed)

On first launch the application opens a single window:

- Default size **960 × 600** pixels, positioned by the window manager
  (at 0,0 in the observation session). (S01, `screenshots/boot-first-run.png`)
- Window title: **“Devices & Locations”**.
- The window uses client-side decorations (CSD). The header bar contains,
  left to right: the centered title; at the right edge a **Main Menu** button
  (hamburger, ☰), **Minimize**, **Maximize**, **Close** buttons.
- Content area (start page): a **“This Device”** section heading followed by a
  rounded card list with one row per scannable location:
  - **Home Folder** — subtitle: the user’s home path; house icon; chevron (›).
  - One row per mounted filesystem, e.g. **acer-archlinux** — subtitle `/`,
    drive icon, right-aligned “**177.6 GB Available**” over
    “**511.0 GB Total**”, a horizontal blue usage bar, chevron (›).
  - The available/total figures come from the filesystem; they changed
    between observations (177.4 → 175.5 GB) as the real disk usage changed —
    they are live values, not cached.
- There is **no** separate splash, wizard, or first-run dialog.

**Arbor requirement**: on first start Arbor shows a single window titled
“Devices & Locations” with a start page listing the user’s home folder and
mounted filesystems with live available/total space and a usage bar. Minimum
window content must remain usable down to at least 400 × 300 (see §5).

## 2. Recents on the start page (Observed)

After folders have been scanned, the start page lists them **below the device
rows**, in the same card list, with a folder icon, the folder’s display name,
its full path as subtitle, and a chevron. No size information is shown for
recent rows.

- Ordering: **most recently scanned first** (verified: after scanning
  fixture-perm, empty-dir, big-tree2, fixture-trash, big-tree in that order,
  the list showed exactly that order, newest at top). (S24,
  `screenshots/A-startpage-with-recents.png`)
- **Main Menu → Clear Recent List** removes all recent entries immediately —
  no confirmation, no toast. The list returns to just Home Folder + devices.
  (S25, `screenshots/A-after-clear-recents.png`)
- Recents survive an application restart (they are persisted per user
  profile); *Observed within one session via the persisted profile directory*;
  restart persistence was not separately re-verified — **Unknown** across
  restarts.

**Arbor requirement**: maintain a recents list of scanned folders shown
newest-first on the start page, and a “Clear Recent List” action in the main
menu that empties it without confirmation.

## 3. Result view window state (Observed)

When a scan result is shown, the same window changes in place:

- The **Back button** (‹) appears at the far left of the header bar.
- The title area becomes the **location pill**: a wide, rounded field showing
  a drive/home icon and the location name. In a result view it renders the
  breadcrumb path (e.g. `fixture-basic / docs`); each segment is clickable
  and navigates to that ancestor (see `02`).
- A **Rescan** button (circular-arrows icon) sits right of the pill.
- The window title (as reported to the window manager and the accessibility
  tree) becomes the **location’s display name** (e.g. “fixture-basic”,
  “acer-archlinux”, “Home Folder” shows as the location name in the pill but
  the window/frame title was observed as the scanned folder name).
  (S05, `screenshots/B-fixture-basic-view.png`)

## 4. Main Menu (Observed)

The Main Menu button opens a popover menu anchored at the button:

| Item | Shortcut | Notes |
|---|---|---|
| Scan Folder… | Ctrl+O | opens the folder chooser (see `03`) |
| Clear Recent List | — | clears recents (§2) |
| — separator — | | |
| Preferences | Ctrl+, | opens the Preferences dialog (§6) |
| Keyboard Shortcuts | Ctrl+? | opens the shortcuts window (§7) |
| Help | F1 | not exercised in the isolated session — **Unknown** |
| About Disk Usage Analyzer | — | opens the About dialog (§8) |

(S02, `screenshots/A-main-menu-open.png`)

The menu closes on Escape, on selecting an item, and on clicking elsewhere.
No other menus or toolbars exist.

## 5. Window resizing (Observed)

- Resizing is fluid; content re-lays out live. (S32/S33,
  `screenshots/A-resized-1200x800.png`, `screenshots/A-resized-400x300.png`)
- At 1200 × 800 the start-page card list keeps its maximum content width
  (~580 px) and stays horizontally centered; the “This Device” heading
  aligns to the card’s left edge.
- At 400 × 300 everything remains visible and operable: the title shifts
  left next to the Main Menu button, the usage bar truncates, the
  available/total labels compress. No minimum-size wall was hit at 400 × 300.
- Result-view resizing: the list and chart panes split **≈50/50** at every
  tested size (960×600, 1200×800, 600×400); the chart redraws to fill.
  (S30/S31, `screenshots/C-result-1200x800.png`, `screenshots/C-result-600x400.png`)

**Arbor requirement**: support live resize from ~400×300 upward with the
behaviors above; result view splits list/chart 50/50.

## 6. Preferences dialog (Observed)

Main Menu → Preferences opens a modal dialog:

- Title: **Preferences**; close (✕) button top-right.
- Single section: **“Locations to Ignore”** with one control:
  **“+ Add Location”**.
- Clicking it opens a **“Select Location to Ignore”** folder chooser (sidebar:
  Recent, Home, …; Cancel / Open buttons). (S04,
  `screenshots/A-preferences-add-location.png`)
- Ignored locations were not added during observation (to keep the
  environment pristine), so the effect of an ignore entry on scans is
  **Unknown** by direct observation.

**Arbor requirement**: provide a Preferences dialog with an “Locations to
Ignore” list; adding an entry uses a folder chooser. *Design decision*: an
ignored location and everything under it is excluded from scan results and
from all size/item totals, and ignore entries are persisted per user.

## 7. Keyboard Shortcuts window (Observed)

Main Menu → Keyboard Shortcuts (or Ctrl+?) opens a separate shortcuts window:
a **“Search shortcuts”** entry at top, then grouped, scrollable lists
(a11y-verified complete list):

**General**: Show Help — F1 · Open Main Menu — F10 · Quit — Ctrl+Q ·
Show Keyboard Shortcuts — Ctrl+? · Show Preferences — Ctrl+,

**Scanning**: Scan Folder — Ctrl+O · Rescan Current Location — Ctrl+R

(S03, `screenshots/A-keyboard-shortcuts.png`, `screenshots/A-shortcuts-bottom.png`)

## 8. About dialog (Observed)

Main Menu → About Disk Usage Analyzer opens a centered dialog:

- Application icon (pie-chart style), name **Disk Usage Analyzer**,
  vendor **The GNOME Project**.
- A version badge **“50.0”**. Clicking the badge copies the version string to
  the clipboard and shows a **“Copied to clipboard”** toast (bottom center).
- Rows: **Website** (external-link icon), **Report an Issue** (external-link
  icon), **Credits** (chevron, sub-page), **Legal** (chevron, sub-page).
  (S08, `screenshots/A-about-dialog.png`)
- Credits/Legal sub-pages were not opened — content **Unknown**.

**Arbor requirement**: About dialog with app identity, a version badge that
copies the version on click with a “Copied to clipboard” toast, and
Website / Report an Issue / Credits / Legal entries.

## 9. Application identity and instances (Observed)

- The application is single-instance: launching `baobab <folder>` while the
  app is running opens/updates a window scanning `<folder>` instead of
  starting a second process window stack (observed: the running window
  navigated to the new location; see `03` §2).
- Quit shortcut Ctrl+Q is registered (shortcuts window); not exercised to
  destruction in observation.

## 10. What was not observed

- Help (F1) behavior (no help browser in the isolated session) — **Unknown**.
- Window size/position persistence across restarts — **Unknown**.
- Maximize/fullscreen behavior was available as a header control but not
  exercised in detail — **Unknown** beyond control existence.
