# 09 — Test Scenarios

Part A sets up the environment, Part B lists the black-box observation
scenarios (S01–S33) performed against GNOME Baobab 50.0, Part C lists
acceptance scenarios for Arbor (including the MANDATORY trash-safety checks
from [06](06-trash-safety-requirements.md)).

## Part A — Reproducing the observation environment

1. Build fixtures (all disposable, under `/tmp/ai-agent/baobab-tests/`):
   `tools/make-baobab-fixtures.sh` — creates deterministic structures:
   `fixture-basic` (known sizes, hidden files, symlinks incl. a broken link
   and a directory symlink, sparse file, unusual/unicode/long/emoji
   filenames, empty dir, nested levels), `fixture-loop` (symlink cycle),
   `fixture-perm` (chmod-000 dir), `fixture-trash` (items destined for
   trash testing), `scan-target-simple`.
2. Start the isolated session: `tools/start-baobab-session.sh` — Xvfb `:98`
   1440×900×24, private D-Bus, private `XDG_RUNTIME_DIR`, redirected
   `HOME`/`XDG_DATA_HOME` (so Trash and settings are sandboxed), mutter WM,
   AT-SPI registry, then baobab. It writes the session environment to
   `/tmp/ai-agent/baobab-env.sh`.
3. In a shell that sourced that env file: screenshots via
   `tools/shot.sh <name> [WxH+X+Y]`; interaction via `xdotool`;
   accessibility dumps via `tools/a11y.py` / `tools/a11y-focus.py`.

Ground-truth arithmetic used for verification: `du -sb` (apparent),
`du -sB1` (allocated), `stat -c '%s %b'` per node.

## Part B — Observation scenarios (performed against Baobab 50.0)

| ID | Initial state | Action | Observed result | Evidence |
|----|----------------|--------|-----------------|----------|
| S01 | fresh profile, app started | observe window | 960×600, title “Devices & Locations”, This Device list (Home, root disk w/ available/total + usage bar), CSD header | `boot-first-run.png` |
| S02 | start page | click Main Menu (☰) | popover: Scan Folder… (Ctrl+O), Clear Recent List, Preferences (Ctrl+,), Keyboard Shortcuts (Ctrl+?), Help (F1), About Disk Usage Analyzer | `A-main-menu-open.png` |
| S03 | main menu open | click Keyboard Shortcuts | shortcuts window: General (F1, F10, Ctrl+Q, Ctrl+?, Ctrl+,) + Scanning (Ctrl+O, Ctrl+R), search entry | `A-keyboard-shortcuts.png`, `A-shortcuts-bottom.png` |
| S04 | main menu open | Preferences → Add Location | “Select Location to Ignore” folder chooser; Cancel/Open | `A-preferences.png`, `A-preferences-add-location.png` |
| S05 | start page | activate root device row | result view opens; spinner in chart pane; list fills progressively; totals update live (4.1 GB/30744 → 39.5 GB/516810); Modified “Unknown” during scan | `B-after-home-scan-enter.png`, `B-root-scan-rings.png` |
| S06 | app running | `baobab <folder>` on CLI | same window navigates and scans `<folder>` (single instance) | a11y frame title change |
| S08 | main menu open | About Disk Usage Analyzer | About dialog: name, The GNOME Project, version badge 50.0; badge click → “Copied to clipboard” toast; Website/Report an Issue/Credits/Legal | `A-about-dialog.png` |
| S09 | main menu open | Scan Folder… (Ctrl+O) | “Select Folder” chooser; sidebar; Cancel/Open | `B-scan-folder-dialog.png` |
| S09b | chooser open | Ctrl+L, type path | location entry w/ completion; checkbox “Recursively analyze mount points” (unchecked) | `B-chooser-typed.png` |
| S09c | chooser location entry | confirm invalid path | chooser closes; main-window toast “… is not a valid folder” with ✕ | `B-scan-dialog-invalid-path.png` |
| S10 | start page with recents | observe rows | recents listed newest-first, folder icon + name + path | `A-startpage-with-recents.png` |
| S11 | result view | click Back (‹) | return to start page | `B-cancel-test-after-esc.png` (start page restored) |
| S12 | fixture-basic result view | single-click folder row | navigates into it; breadcrumb `fixture-basic / docs`; largest child auto-selected | `C-docs-navigated.png` |
| S13 | result view | single-click file row | selects the row (highlight), no navigation | `C-click-file-row2.png` |
| S13a | result view | right-click row | context menu: Open Externally / Copy Path to Clipboard / Move to Trash | `D-context-menu-folder.png`, `E-trash-menu.png` |
| S13b | result view | click expander ▶ | children expand inline, indented; arrow flips | `C-expander-l1.png` |
| S13c | no handler available | context menu → Open Externally | toast “Failed to open file” (role=alert, ✕, ~3–4 s) | `D-failed-to-open-infobar2.png` |
| S13d | result view | Copy Path to Clipboard; paste in chooser entry | clipboard = absolute path of item; no toast | `D-clipboard-pasted.png` |
| S14 | fixture-trash scan | context menu → Move to Trash (folder `trash-target`, then file) | **no confirmation**; row disappears; XDG trashinfo written; no success feedback | `E-trash-menu.png`, `E-after-trash-settled.png` |
| S14b | after S14 | observe pinned row | totals stale (78.0 kB / 5 items) until rescan | `E-after-trash-settled.png` |
| S15 | Trash made read-only (chmod 500) | Move to Trash | toast “Failed to trash file”; item untouched; no permanent deletion | `E-trash-fail-toast2.png` |
| S16 | `fixture-basic / docs` view | click breadcrumb segment `fixture-basic` | navigates back to fixture-basic root | `C-ring-click-dir.png` era state |
| S16b | rings view | hover / click ring segment | hover brightens; click (3 radii) and double-click: no effect | `C-ring-click-dir.png` |
| S17 | fixture-perm scan | observe locked dir | row with red name, blank Size/Contents; scan completes; no dialog | `B-perm-error-scan.png` |
| S18 | empty-dir scan | observe | single row “40 bytes / 1 item”; single flat chart tile | `B-empty-dir-scan.png` |
| S19 | result view | ↑/↓ keys | selection moves without activating (folder selected w/o navigation); treemap follows selection | `C-nested-l1-selected.png` |
| S20 | fixture-basic (symlinks) / fixture-loop (cycle) | scan | symlinks absent from rows and counts; broken link harmless; cycle no hang | a11y row dump; counts verified |
| S20b | treemap active | select items | treemap always shows selected item’s subtree | `C-nested-l1-selected.png` |
| S20c | treemap active | double-click folder rect | navigates into it (breadcrumb …/l1/l2); file rect: no effect | `C-treemap-folder-dclick.png` |
| S21 | 25 000-file tree scan | create file + delete file mid-scan | snapshot semantics: addition absent, deletion included; totals stable | `B-bigtree-after-midscan.png` |
| S22 | slow scan running | press Escape | leaves result view to start page immediately; no cancel control exists | `B-cancel-test-after-esc.png` |
| S23 | result view | click Rescan (↻) | re-scan; totals recomputed (8.3 kB / 2 items incl. new file) | `E-rescan-check.png` |
| S24 | several scans done | start page | recents newest-first below devices | `A-startpage-with-recents.png` |
| S25 | recents present | Main Menu → Clear Recent List | recents gone immediately, no confirmation | `A-after-clear-recents.png` |
| S26 | start page | Tab ×N | focus cycle: row1 → row2 → Main Menu → wrap (a11y-verified) | a11y focus dump |
| S27 | result view | click “Folder” header | name sort asc, case-insensitive, dotfiles first; header highlighted | `C-sort-folder.png` |
| S28 | result view | click “Size” header ×2 | asc then desc; ▲/▼ chevron; ties stable but non-alphabetical; pinned root row unaffected | `C-sort-size-asc.png`, `C-sort-size-desc.png` |
| S29 | treemap active in window | navigate to another location | chart mode persists per window | `B-root-scan-rings.png` |
| S30/S31 | result view | resize 1200×800 / 600×400 | list:chart ≈50/50 at all sizes; chart redraws | `C-result-1200x800.png`, `C-result-600x400.png` |
| S32/S33 | start page | resize 1200×800 / 400×300 | centered max-width card; fully usable at 400×300 | `A-resized-1200x800.png`, `A-resized-400x300.png` |

## Part C — Arbor acceptance scenarios

Functional checks derived from this specification (Team B implements; every
check must pass before hand-off):

| ID | Requirement | Check |
|----|-------------|-------|
| A1 | `06` R1/R2 | Move-to-Trash on a **test folder created for the test** opens the confirmation dialog with title “Move folder to Trash?”, exact folder name, full path, explanation, Cancel + Move to Trash; no filesystem change before confirmation. |
| A2 | `06` R3/R4 | Cancel, Escape, close ✕, Enter (default) → folder untouched; Trash unchanged. |
| A3 | `06` R4 | Only explicit Move to Trash click performs the operation; folder moves under `$XDG_DATA_HOME/Trash/files/` with valid `.trashinfo`. |
| A4 | `06` R5 | With read-only Trash: failure feedback, item untouched, **no deletion** anywhere. |
| A5 | `06` R6 | Success shows feedback (toast w/ Undo where feasible); totals refresh (R8). |
| A6 | `06` R7 | No privilege-escalation attempt during trash (no polkit/sudo). |
| A7 | `06` R9 | Same confirmation for a single test file (“Move file to Trash?”). |
| A8 | `04` §4 | Fixture arithmetic: 256 B file → “4.1 kB”; sparse 10 MiB → “10.5 MB”; item counts exclude symlinks, include self. |
| A9 | `03` §3 | Hidden files included; symlink cycle scans without hang. |
| A10 | `07` | Invalid path toast; unreadable dir → red name + blank cells; Open Externally failure toast. |
| A11 | `02` | Folder-click navigates, file-click selects, breadcrumb navigates, Escape returns. |
| A12 | `08` | Full keyboard map works; toasts role=alert; rows fully readable by screen reader. |

Rule: acceptance scenarios run against **Arbor**; observation scenarios
(Part B) were run against **Baobab 50.0** and are recorded for provenance.
