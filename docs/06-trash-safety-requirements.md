# 06 — Trash Safety Requirements (Arbor-specific, MANDATORY)

This document is an **Arbor requirement**, not a description of the observed
application. Where it conflicts with observed behavior of GNOME Baobab 50.0,
**this document wins** for Arbor.

Provenance of the “observed baseline” below: every statement was established
by performing the operation on **disposable test files created by the
observation team** inside an isolated Xvfb session with a redirected
`XDG_DATA_HOME`, so no real user data was ever touched (see
[11 — Provenance](11-provenance-and-observation-log.md)).

## 1. Observed baseline (for the record)

The observed application (Baobab 50.0):

- offers **Move to Trash** in the row context menu for files and folders;
- performs it **immediately on menu click, with no confirmation dialog**
  (verified on a test folder `trash-target` and a test file
  `top-level-note-999B.txt`);
- gives **no success feedback**;
- leaves the location totals **stale** until a manual rescan;
- on failure (Trash made read-only) shows a **“Failed to trash file”** toast
  and leaves the item in place — it does **not** fall back to permanent
  deletion (verified);
- writes standard XDG Trash metadata (`Path=`, `DeletionDate=`) and creates
  `$XDG_DATA_HOME/Trash/{files,info}` on demand.

## 2. Arbor requirements — confirmation dialog (MANDATORY)

**R1 — Mandatory confirmation.** Moving a folder to Trash MUST always
require explicit user confirmation. There MUST be no code path that moves a
folder to Trash without the dialog of §3 having been confirmed in that
instance. This includes batch operations, keyboard accelerators, and any
future automation API.

**R2 — Dialog content.** The confirmation dialog MUST contain, verbatim in
structure:

- Title: **“Move folder to Trash?”**
- The **exact folder name** (display name, not escaped).
- The **full target path** of the folder.
- An explanation sentence: the folder **and everything inside it** will be
  moved to Trash.
- A **Cancel** button.
- A **Move to Trash** confirmation button.

**R3 — Default action is Cancel.** The dialog’s default (initial focus and
Enter key) MUST be **Cancel**. The destructive button must never be the
default and must never be triggered by Enter. Escape MUST map to Cancel.

**R4 — No side effects on open/dismiss.** Opening the dialog, closing it,
pressing Escape, clicking Cancel, or destroying the dialog in any way MUST
leave the folder exactly untouched on disk and in the Trash. Only an
explicit click (or explicit keyboard activation) of **Move to Trash** may
start the operation.

**R5 — Trash access failure must never cause deletion.** If the Trash is
unavailable or the move fails for any reason (permissions, full disk,
missing `$XDG_DATA_HOME`, cross-device issues), Arbor MUST leave the item in
place and report the failure. There MUST be **no** fallback to unlinking,
shredding, or any form of permanent deletion. (The observed application
already satisfies the no-fallback property; Arbor MUST keep it.)

**R6 — Feedback.** On success Arbor MUST give clear positive feedback (toast
“Moved to Trash” with an explicit **Undo** affordance where feasible) and on
failure a clear error toast naming the operation (“Failed to move folder to
Trash”) with a dismiss control. Silence is not acceptable feedback for a
destructive operation.

**R7 — No privilege escalation.** The operation MUST run with the user’s
ordinary privileges. Arbor MUST NOT prompt for or attempt elevated
permissions (no `pkexec`, `sudo`, admin portals) for trashing.

**R8 — Fresh state after the operation.** After a successful move, Arbor
MUST remove the item’s row and update the location’s displayed totals
immediately (no stale totals), or — if re-measurement would be slow — update
totals optimistically and reconcile on the next rescan, and say which it
does.

**R9 — Files too.** The task mandate is folders; Arbor applies the same
confirmation to **files** moved to Trash, with title “Move file to Trash?”
(*design decision*: consistent confirmation for every destructive action).

**R10 — Scope label accuracy.** The dialog MUST show the item exactly as it
will be trashed: for a folder, the folder itself (not its parent, not a
selection surrogate). If multiple items are ever selected in the future, the
dialog MUST list every affected path.

## 3. Dialog specification (normative layout)

```text
┌──────────────────────────────────────────────┐
│  Move folder to Trash?                    ✕  │
│                                              │
│  Folder: trash-target                        │
│  Path:   /home/user/tmp/trash-target         │
│                                              │
│  The folder and all of its contents will be  │
│  moved to the Trash. You can restore it from │
│  the Trash later.                            │
│                                              │
│              [ Cancel ]  [ Move to Trash ]   │
└──────────────────────────────────────────────┘
```

- Initial focus: **Cancel**. Enter → Cancel. Escape → Cancel (dialog closes,
  nothing happens).
- The destructive button uses the destructive visual style but is never
  default-focused.
- The dialog is modal to the window; the window behind it must not process
  other actions while it is open.

## 4. Acceptance criteria (testable)

| # | Criterion |
|---|-----------|
| A1 | Right-click folder → Move to Trash opens the dialog; no filesystem change occurs (verified by before/after listing). |
| A2 | Cancel, Escape, dialog close, clicking outside → folder still present at the original path; Trash unchanged. |
| A3 | Enter on a fresh dialog → Cancel behavior (folder untouched). |
| A4 | Click Move to Trash → folder gone from its path; present under `$XDG_DATA_HOME/Trash/files/` with a valid `.trashinfo`; row removed; totals refreshed. |
| A5 | Read-only Trash → failure toast; folder still present; no permanent deletion in any code path. |
| A6 | The dialog shows the exact folder name and full path. |
| A7 | No privileged-execution attempt occurs (no pkexec/sudo/polkit prompts). |
| A8 | Same guarantees hold for a single file. |

Reference scenarios: S14 (trash success), S15 (trash failure), S16 (dialog
cancel matrix) in [09 — Test Scenarios](09-test-scenarios.md).
