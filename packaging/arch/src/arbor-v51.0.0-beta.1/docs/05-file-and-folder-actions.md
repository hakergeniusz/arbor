# 05 — File and Folder Actions

All items are **Observed** unless labeled otherwise. Scenario references
(S##) resolve to [09 — Test Scenarios](09-test-scenarios.md).

## 1. Where actions live (Observed)

The only action surface for individual items is a **context menu** opened by
right-clicking a row in the result-view list. It was observed in both chart
modes (rings and treemap) and for both folder and file rows. The same
three-item menu appears in every case:

1. **Open Externally**
2. **Copy Path to Clipboard**
3. **Move to Trash**

Right-clicking also selects the row under the pointer. No actions exist in a
toolbar, in the Main Menu (besides scan/recents/preferences), or via
keyboard shortcut. (S13a, `screenshots/D-context-menu-folder.png`,
`screenshots/E-trash-menu.png`)

A keyboard path to the context menu (Menu key / Shift+F10) was **not**
established — **Unknown**.

## 2. Open Externally (Observed)

- Launches the item with the system’s default handler (folders in a file
  manager, files with their default application).
- **Failure feedback**: when no handler is available (the isolated
  observation session had none), a dark toast appears at the bottom center of
  the window: **“Failed to open file”** with an embedded dismiss (✕) button.
  The toast auto-dismisses after roughly 3–4 seconds. (S13c,
  `screenshots/D-failed-to-open-infobar2.png`)
- The a11y role of the toast is `alert`; its full text is exactly
  “Failed to open file”.

**Arbor requirement**: open items via the platform default-handler
mechanism; on failure show a non-blocking toast with an explicit failure
message and dismiss control; never crash or fall back to another action.

## 3. Copy Path to Clipboard (Observed)

- Copies the item’s **absolute path** to the system clipboard. Verified by
  pasting into the chooser’s location entry: the pasted value was exactly
  `/tmp/ai-agent/baobab-tests/fixture-basic/sparse-10MiB.dat`.
- **No visible feedback** was observed (two attempts, screenshots taken
  ~0.4 s after activation; contrast: the About version badge shows a
  “Copied to clipboard” toast). (S13d, `screenshots/D-clipboard-pasted.png`)

**Arbor requirement**: copy the absolute path. *Design decision*: Arbor adds
a short “Copied to clipboard” toast, matching the pattern the original
application uses elsewhere.

## 4. Move to Trash (Observed — full details and safety implications in `06`)

- Performs the move to the XDG Trash **immediately — no confirmation dialog
  of any kind**. For both folders and files.
- The row disappears from the list right away.
- The pinned location row’s totals are **NOT refreshed** (stale size/items
  until a manual Rescan; observed: after trashing a folder holding 94.65 %
  of a fixture, the root row still showed the old 78.0 kB / 5 items).
  (S14b, `screenshots/E-after-trash-settled.png`)
- Success produces **no toast or other feedback** (absence of feedback is
  itself the observed behavior).
- Trash metadata was written per the XDG Trash specification (`.trashinfo`
  with absolute `Path=` and `DeletionDate=`), and the Trash directory
  (`$XDG_DATA_HOME/Trash/{files,info}`) is created on demand.
- **Failure case** (Trash made read-only): a toast **“Failed to trash file”**
  appears; **the item is left in place**; no deletion or fallback occurs;
  the row remains in the list. (S15, `screenshots/E-trash-fail-toast2.png`)
- Cancel path: pressing Escape with the context menu open closes the menu
  and performs nothing (verified; the item stays).

**Arbor requirement**: the Move to Trash action MUST follow the MANDATORY
safety specification in [06 — Trash Safety Requirements](06-trash-safety-requirements.md),
which deliberately differs from the observed no-confirmation behavior.

## 5. Post-action refresh (Observed)

- Move to Trash: row removed immediately; **totals stale** until Rescan.
- No other list/chart updates were triggered by actions.
- Rescan (↻ / Ctrl+R) recomputes everything including the pinned row’s
  totals (verified: after rescan the fixture root showed the refreshed
  8.3 kB / 2 items).

**Arbor requirement**: after a successful trash operation Arbor refreshes
the affected row and the location totals immediately (Arbor differs here on
purpose — a stale total after a destructive action invites wrong decisions;
see `06` R8).
