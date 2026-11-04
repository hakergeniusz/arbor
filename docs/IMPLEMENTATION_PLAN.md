# Arbor — Implementation Plan (Team B)

Status: living document, started 2026-11-04 on branch `implementer/arbor`.
Input: `docs/00`–`docs/11` + `screenshots/` (Team A, clean-room black-box
specification of GNOME Baobab 50.0). No upstream code or internal docs were
consulted; see `docs/PROVENANCE.md`.

## 1. Requirements checklist

Derived from the specification; every item is implemented + tested unless
marked otherwise. MANDATORY items are safety-critical.

### Window / start page (docs/01)

- [ ] Single window, 960×600 default, CSD header (title, ☰, min/max/close) (S01)
- [ ] Start page “This Device”: Home Folder row + one row per displayed mount
      with live available/total + usage bar, chevrons (S01)
- [ ] Recents rows below devices, newest first, folder icon + name + path (S24)
- [ ] Main menu: Scan Folder… Ctrl+O · Clear Recent List · sep · Preferences
      Ctrl+, · Keyboard Shortcuts Ctrl+? · Help F1 · About (S02)
- [ ] Clear Recent List empties recents immediately, no confirmation (S25)
- [ ] Result view: Back ‹ + location pill with clickable breadcrumb + Rescan ↻ (S05, S11, S16)
- [ ] Window title mirrors view: “Devices & Locations” / location display name (S, 02 §8)
- [ ] Resize: fluid from 400×300; start page card max-width ~580 px centered;
      result view 50/50 panes (S30–S33)
- [ ] Preferences: “Locations to Ignore” + “+ Add Location” (chooser) (S04)
- [ ] Shortcuts window: search + General (F1, F10, Ctrl+Q, Ctrl+?, Ctrl+,) +
      Scanning (Ctrl+O, Ctrl+R) (S03)
- [ ] About: identity, version badge copies version + “Copied to clipboard”
      toast, Website / Report an Issue / Credits / Legal (S08)
- [ ] Single instance; CLI `arbor <folder>` navigates the running window (S06)

### Navigation & interactions (docs/02)

- [ ] Tab order: rows top-to-bottom, then Main Menu (S26)
- [ ] Folder row click → navigate; file row click → select; expander → inline
      expand/collapse; double-click folder navigates (S12, S13, S13b)
- [ ] First row = pinned location row with up-arrow (^) → parent (S15)
- [ ] Back → start page (S11); breadcrumb segment click → that ancestor (S16)
- [ ] ↑/↓ move selection without activating; treemap follows selection (S19)
- [ ] Enter/Space activate row like click; Escape navigates back, even
      mid-scan (S22)
- [ ] Chart toggle: Rings/Treemap buttons bottom-right, per-window state (S29)
- [ ] Hover brightens ring segment; list row hover highlight (S16b)

### Scanning (docs/03)

- [ ] Entry points: start rows, chooser (Ctrl+O), CLI, Rescan ↻/Ctrl+R
- [ ] Chooser: folder chooser with Cancel/Open + “Recursively analyze mount
      points” checkbox (persisted) (S09b)
- [ ] Invalid typed path → chooser closes, toast `"<path>" is not a valid folder` (S09c)
- [ ] Spinner in chart pane while scanning; progressive list fill; root totals
      update live; Modified “Unknown” until known (S05)
- [ ] Snapshot semantics (no mid-scan updates; Rescan required) (S21)
- [ ] Hidden files included; symlinks excluded everywhere (no follow, no rows,
      not counted, cycle-safe) (S20)
- [ ] Empty dir → single row “40 bytes / 1 item”, single flat tile (S18)
- [ ] Unreadable dir → red name, blank Size/Contents, still counted (parent
      metadata), no dialog, scan continues (S17)
- [ ] Escape/Back leaves a running scan view immediately; Arbor additionally
      stops the background scan and frees partial data
- [ ] Completed folder scans are added to recents (top, newest first)

### Visualization (docs/04)

- [ ] Two-pane 50/50: list (Folder | Size | Contents | Modified) + chart
- [ ] Pinned location row unaffected by sorting; largest child auto-selected
      after navigation
- [ ] Percent badge (two decimals) with mini progress bar; name colored to
      match chart segment; ellipsized names, full name for a11y
- [ ] Sorting: initial Size ▼; header click sorts; re-click toggles; Folder
      name sort case-insensitive, dotfiles first; ties by name; pinned row
      unaffected (S27/S28)
- [ ] Size arithmetic: node contribution = max(st_size, st_blocks·512),
      folders sum subtree (S/A8 fixtures: 256 B → 4.1 kB, sparse 10 MiB →
      10.5 MB, 8.3 kB folder, 122.3 MB fixture total)
- [ ] Contents count = subtree nodes incl. self, excl. symlinks (28/25 004/1)
- [ ] Number format: decimal units, GLib `g_format_size` output
      (“122.3 MB”, “40 bytes”, …)
- [ ] Modified: “Today” / “N days” (calendar-day relative)
- [ ] Infobar “Files may take more space than shown”, always, no dismiss
- [ ] Rings: depth-n rings, proportional spans, colored center total, selected
      segment exploded, hover brightens, click = no-op (display-only decision)
- [ ] Treemap: selected item’s subtree, squarified area-proportional rects,
      children inset, labels when fit; single-click selects (decision);
      double-click folder navigates, file dblclick no-op (S20b/S20c)
- [ ] Empty dir → one gray tile labeled with folder name
- [ ] Design decision: treemap rects are individually focusable widgets with
      accessible name+size; rings expose an accessible summary + data via list

### Actions (docs/05) + Trash safety (docs/06, MANDATORY)

- [ ] Context menu on rows: Open Externally / Copy Path to Clipboard / Move to
      Trash; right-click selects row (S13a); keyboard path Shift+F10/Menu (a11y gap fix)
- [ ] Open Externally via GIO default handler; failure → toast “Failed to open
      file” (✕, ~4 s, role=alert) (S13c)
- [ ] Copy Path copies absolute path; Arbor adds “Copied to clipboard” toast (S13d)
- [ ] **R1** Confirmation dialog for EVERY Move to Trash (folder and file, R9)
- [ ] **R2** Dialog: title “Move folder to Trash?” (file: “Move file to
      Trash?”), exact name, full path, explanation, Cancel + Move to Trash
- [ ] **R3** Default = Cancel (initial focus + Enter); Escape = Cancel;
      destructive button never default
- [ ] **R4** Open/Escape/Cancel/close/dismiss = zero side effects
- [ ] **R5** Trash failure (read-only, missing XDG_DATA_HOME, cross-device…) →
      item untouched, error toast, NEVER permanent deletion, no fallback
- [ ] **R6** Success toast “Moved to Trash” with Undo; failure toast “Failed to
      move folder/file to Trash”; externally-deleted item → “Item no longer exists”
- [ ] **R7** No privilege escalation (no pkexec/sudo/polkit anywhere)
- [ ] **R8** Row removed and totals updated immediately after success (exact
      subtraction of subtree totals, no stale numbers); Undo restores
- [ ] **R10** Dialog names the exact item; single-item only today, multi-select
      would list all paths
- [ ] Rescan recomputes everything (S23)

### Error handling (docs/07)

- [ ] Per-item scan errors never interrupt the scan; encoded in rows
- [ ] Toasts transient/dismissible, bottom center, role=alert
- [ ] No modal error dialogs; no success dialogs
- [ ] Deleted-between-scan-and-action target → failure toast, refresh view

### Accessibility & keyboard (docs/08)

- [ ] Named frames; rows exposed as one readable string each; named dialogs
- [ ] Toasts role=alert; chart toggles labelled
- [ ] Full keyboard map (F1, F10, Ctrl+Q, Ctrl+?, Ctrl+,, Ctrl+O, Ctrl+R,
      Escape, Tab, ↑/↓, Enter/Space, Shift+F10 context menu)
- [ ] Visible focus everywhere; modal dialog focus trapping
- [ ] Chart data keyboard/screen-reader reachable (see decision above)

## 2. Decisions resolving Unknowns / ambiguities

| # | Topic | Decision |
|---|-------|----------|
| D1 | “Recursively analyze mount points” | Checked = cross filesystem boundaries (mounts become ordinary rows); unchecked = other mounts are excluded from listing and totals. State persisted per user. |
| D2 | Recents persistence | Persisted across restarts as JSON in `$XDG_STATE_HOME/arbor/`. |
| D3 | Window size/position | Persisted across restarts; default 960×600. |
| D4 | Chart mode | In-memory per window; defaults to Rings each run (not persisted). |
| D5 | Escape mid-scan | Leaves the view AND stops the background scan; partial data freed. |
| D6 | Ring click | Display + hover only; all interaction on the list/treemap. |
| D7 | Sort ties | Case-insensitive name (then path). |
| D8 | Infobar | Always shown in every result view. |
| D9 | Recent row click | Opens and scans that folder (same as device rows). |
| D10 | Help (F1) | Opens the project website in the default handler; failure → toast “Failed to open help”. |
| D11 | Context menu keyboard | Shift+F10 / Menu key opens context menu for selected row. |
| D12 | Size formatting | GLib `g_format_size` (decimal, 1 decimal place, “N bytes” < 1 kB) — matches every observed value. |
| D13 | Modified days | Calendar-day difference (mtime date vs today); “Today”, “1 day”, “N days”; future mtime clamps to “Today”. |
| D14 | Hardlinks | Counted per path (spec counts “nodes”); no inode de-duplication; documented deviation risk. |
| D15 | Ignore list | Ignored location and everything under it excluded from scans/totals; rows removable from Preferences; persisted. |
| D16 | Scan target unreadable/deleted | Toast “Item no longer exists” / “\<path\> is not a valid folder”, stay on start page. |
| D17 | Charts a11y | Treemap rects = focusable widgets with name+size labels; rings expose accessible summary; list is the canonical accessible data. |
| D18 | App identity | Arbor, app id `io.github.hakergeniusz.Arbor`, MIT, original ring-motif icon; About shows “Arbor”. |
| D19 | Percent badges | Two decimals, mini progress bar fill = share of location total. |
| D20 | Scan engine | std::fs + unix metadata on a worker thread, updates over a GLib channel; cancellation token; no async runtime dependency. |

Note: `docs/04` §8 cites “(S19b/S20b)” — there is no S19b in `docs/09`
(dangling reference, Team A). The behavior is fully described in text; no
specification content is lost. Flagged here per clean-room rules; Team A docs
are not modified.

## 3. Architecture

```
src/
├── main.rs            entry: gtk::Application, CLI folder handling
├── app.rs             actions, menus, accelerators, about/prefs/shortcuts wiring
├── models.rs          TreeNode (path, name, size, items, mtime, flags, children)
├── scanner.rs         walk thread, cancellation, incremental updates, ignores, mounts
├── mounts.rs          mounted-filesystem enumeration + statvfs usage
├── fsops.rs           trash (confirm-independent ops), open-externally, clipboard, undo
├── settings.rs        JSON persistence (recents, ignores, mount flag, window size)
├── format.rs          size/day formatting helpers (wrapping glib::format_size)
├── ui/
│   ├── window.rs      headerbar, view switch, toasts overlay, window state
│   ├── start_page.rs  This Device + recents lists
│   ├── result_view.rs two panes, list+rows, breadcrumb, infobar
│   ├── charts.rs      rings chart (custom draw)
│   ├── treemap.rs     squarified treemap (widget-per-rect)
│   ├── palette.rs     original hue assignment
│   ├── dialogs.rs     trash confirmation (R2/R3), preferences
│   └── chooser.rs     Select Folder dialog + mount checkbox + validation
└── resources.rs       embedded assets (icon)
```

Dependencies (audited in `THIRD_PARTY_LICENSES.md`): `gtk4`, `adw` (GTK4 +
libadwaita bindings, MIT), `gio`/`glib` via gtk4-rs, `serde`+`serde_json`
(persistence, MIT/Apache-2.0). No async runtime; unsafe forbidden.

## 4. Phase → commit plan

1. `chore: initialize Rust application` — Cargo, entry, window, Xvfb smoke.
2. `feat(scanner): …` — tree model + walker + fixtures tests.
3. `feat(ui): start page / result view …` — window shell, lists, breadcrumb.
4. `feat(visualization): rings/treemap` + sorting.
5. `feat(trash): confirmation dialog + safe trash + tests` (safety-critical).
6. `feat(a11y)/polish` — shortcuts, focus, a11y labels, resize.
7. `build: packaging` + docs (ARCHITECTURE, IMPLEMENTATION_NOTES, PROVENANCE).

## 5. Test plan

- Unit (no GUI): size/count arithmetic + formatting, sorting, relative days,
  scanner fixtures (nested/hidden/symlink/loop/perm/empty/unicode/large/
  disappearing/cancel), treemap layout invariants, persistence round-trip.
  Destructive trash tests operate **only on fixtures created by the test**
  inside fresh temp dirs with redirected `XDG_DATA_HOME`.
- GUI (Xvfb, `env -i` hermetic session): startup/shutdown, view switching,
  scans + spinner + progressive fill, navigation, sorting, context menu,
  **trash confirmation matrix (A1–A8)**, toasts, dialogs, resize, screenshots
  vs `screenshots/` reference.

Primary invariant: **no folder is moved to Trash unless the user explicitly
confirms the exact target in the dialog.**
