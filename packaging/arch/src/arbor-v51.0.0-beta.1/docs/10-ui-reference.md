# 10 — UI Reference

Visual reference for Team B: what each screen looks like and where its
controls are. Screenshots are **original captures** from the observation
session (1440×900 Xvfb; window at 0,0 unless stated). The small ✕ marks
visible in some captures are the X11 default cursor arrow rendered by the
virtual display — they are not UI elements.

## 1. Screens and states

| # | State | Screenshot | Notes |
|---|-------|------------|-------|
| 1 | First-run start page | `boot-first-run.png` | 960×600; This Device card: Home Folder + root disk row (available/total + blue usage bar); header: title, ☰, min/max/close |
| 2 | Main Menu popover | `A-main-menu-open.png` | anchored under ☰; 7 entries + separator; shortcuts right-aligned |
| 3 | Keyboard Shortcuts window | `A-keyboard-shortcuts.png`, `A-shortcuts-bottom.png` | search entry on top; General + Scanning groups |
| 4 | About dialog | `A-about-dialog.png` | icon, name, vendor, 50.0 badge, Website / Report an Issue / Credits / Legal |
| 5 | Preferences dialog | `A-preferences.png` | “Locations to Ignore” + “+ Add Location” |
| 6 | Preferences → ignore chooser | `A-preferences-add-location.png` | “Select Location to Ignore”, GTK chooser |
| 7 | Start page with recents | `A-startpage-with-recents.png` | device rows + recent folder rows (icon, name, path) |
| 8 | Start page after Clear Recent List | `A-after-clear-recents.png` | only Home + devices remain |
| 9 | Chooser (Scan Folder…) | `B-scan-folder-dialog.png`, `B-chooser-typed.png` | location entry + “Recursively analyze mount points” checkbox visible in the latter |
| 10 | Chooser invalid path | `B-scan-dialog-invalid-path.png` | main-window toast “… is not a valid folder” |
| 11 | Chooser navigated into folder | `B-chooser-inside-fixture.png` | Open button active |
| 12 | Scan in progress | `B-after-home-scan-enter.png` | spinner in chart pane; list partially filled; Modified “Unknown” |
| 13 | Result view — rings, root filesystem | `B-root-scan-rings.png` | infobar; colored names + percent badges; center total; exploded selected segment; chart toggles |
| 14 | Result view — rings, fixture | `B-fixture-basic-view.png` | 12 rows incl. hidden + empty-dir “40 bytes” |
| 15 | Result view — treemap | `C-treemap-view2.png` | selected item’s subtree; labeled rects |
| 16 | Navigation into folder | `C-docs-navigated.png` | breadcrumb `fixture-basic / docs`; up-arrow row |
| 17 | File selection | `C-click-file-row2.png` | highlight moved to clicked file row |
| 18 | Expander open | `C-expander-l1.png` | inline children, indent |
| 19 | Ring hover | `C-ring-click-dir.png` | teal segment brightened; selection unchanged |
| 20 | Treemap folder navigation | `C-treemap-folder-dclick.png` | breadcrumb `fixture-basic / nested / l1 / l2` |
| 21 | Sorting | `C-sort-folder.png`, `C-sort-size-asc.png`, `C-sort-size-desc.png` | header highlight + ▲/▼ chevron |
| 22 | Context menu | `D-context-menu-folder.png`, `E-trash-menu.png` | Open Externally / Copy Path to Clipboard / Move to Trash |
| 23 | Clipboard verification | `D-clipboard-pasted.png` | absolute path pasted into chooser entry |
| 24 | Failure toast | `D-failed-to-open-infobar2.png`, `E-trash-fail-toast2.png` | “Failed to open file” / “Failed to trash file”, ✕ dismiss |
| 25 | After trash | `E-after-trash-settled.png`, `E-after-file-trash.png` | row gone; pinned totals stale |
| 26 | Rescan result | `E-rescan-check.png` | refreshed totals (8.3 kB / 2 items) |
| 27 | Empty-dir result | `B-empty-dir-scan.png` | single row; one flat gray tile |
| 28 | Permission-error result | `B-perm-error-scan.png` | red name, blank Size/Contents |
| 29 | Mid-scan snapshot | `B-bigtree-after-midscan.png` | 25 004 items; mid-scan addition absent |
| 30 | Escape from scan | `B-cancel-test-after-esc.png` | start page restored mid-scan |
| 31 | Result view resized | `C-result-1200x800.png`, `C-result-600x400.png` | ≈50/50 panes at both sizes |
| 32 | Start page resized | `A-resized-1200x800.png`, `A-resized-400x300.png` | fluid, usable at 400×300 |

## 2. Control inventory (result view header)

```
[‹ Back] [ (icon) breadcrumb segment / segment / segment          ] [↻] [–] [□] [✕]
```

- Back ‹ — far left, only in result views.
- Location pill — centered, wide; segments clickable; drive/home icon at its
  start.
- Rescan ↻ — right of pill.
- Window controls — far right (minimize, maximize, close).

## 3. Item list row anatomy

```
[▶] [ 90.46% ▁▁▁▁ ] large          110.7 MB   4 items   Today
```

expander · percent badge (blue pill + mini bar) · color-coded name · size ·
item count · relative date. The location row replaces the expander with an
up-arrow and is pinned on top.

## 4. Toast anatomy

Dark rounded pill, bottom center of the window: message text + ✕ dismiss;
role=alert; auto-dismisses in ~3–4 s.

## 5. Colors

The chart palette assigns distinct hues to top-level children (blues, reds,
greens, purples, teals observed); descendants inherit the parent hue with
variation. List name colors match segment colors; permission-error names are
red; selection highlight is blue.
