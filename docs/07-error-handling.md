# 07 — Error Handling

Catalogue of every error surface observed, plus the explicit gaps. All items
**Observed** unless labeled otherwise.

## 1. Invalid scan target (toast)

Entering a non-existent / non-folder location in the chooser’s location
entry closes the chooser and shows a toast in the main window:

> “/tmp/ai-agent/baobab-tests/fixture-basicent” is not a valid folder

with an embedded ✕ dismiss button. The quoted string is the exact typed
path. (`screenshots/B-scan-dialog-invalid-path.png`, S09c)

## 2. Unreadable subtree (in-row presentation)

A directory that cannot be read (chmod 000) during a scan:

- still appears as a row;
- its name is rendered in **red**;
- its **Size** and **Contents** cells are **blank** (not zero, not “?”);
- it still contributes its own node to the parent’s item count and byte
  total (parent-side metadata was readable);
- **no dialog, toast, or banner** reports the error;
- the rest of the scan completes normally.
  (`screenshots/B-perm-error-scan.png`, S17)

**Arbor requirement**: identical in-row presentation. *Design decision*: in
addition, Arbor logs skipped paths to a session log accessible from the UI
(optional), but never interrupts the scan for per-item errors.

## 3. Open Externally failure (toast)

With no default handler available, a toast **“Failed to open file”**
(role=alert, ✕ dismiss, auto-dismiss ~3–4 s) appears; the app continues
normally. (`screenshots/D-failed-to-open-infobar2.png`, S13c)

## 4. Trash failure (toast + untouched item)

With the Trash made read-only, Move to Trash shows a toast **“Failed to
trash file”** (same style), leaves the item in place, and never falls back
to permanent deletion. (`screenshots/E-trash-fail-toast2.png`, S15)
Arbor raises this to a MANDATORY requirement — see `06` R5/R6.

## 5. Informational banner (not an error)

“Files may take more space than shown” — static infobar in every result
view (`04` §6). Not dismissible, not an error state.

## 6. What was deliberately not observed

- Scanning a location that disappears between selection and scan start —
  **Unknown**.
- Moving to Trash an item that was deleted externally between scan and
  action — **Unknown** for the observed app; **Arbor requirement**: treat as
  failure with a toast (“Item no longer exists”) and refresh the view; never
  delete anything else.
- Multiple simultaneous failures (e.g. many unreadable subtrees) — only
  single-error cases observed; per-item presentation is expected to compose.

## 7. Error-presentation principles distilled from observation

1. **Never block the scan** for per-item problems; encode them in the row.
2. **Transient, dismissible toasts** for action failures (bottom center,
   ~3–4 s, role=alert).
3. **No modal error dialogs** were observed anywhere in the application.
4. **No success dialogs**; success is communicated by state change (row
   disappears) — Arbor strengthens this for destructive actions (`06` R6).
