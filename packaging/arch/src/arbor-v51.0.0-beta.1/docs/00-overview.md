# 00 — Overview

This repository is the **behavioral specification of Arbor**, an independent
Rust implementation of a disk-usage analyzer. It was written by observing the
user-facing behavior of GNOME Baobab 50.0 as an ordinary user through a GUI,
in an isolated Xvfb session (see [11 — Provenance](11-provenance-and-observation-log.md)).
No upstream source code, tests, or internal documentation was consulted.

## How to read this specification

Every behavioral statement in `docs/01`–`docs/09` is classified with one of
four labels:

| Label | Meaning |
|---|---|
| **Observed** | Directly verified by interacting with the observed application and recording the visible outcome (screenshot and/or accessibility-tree record; see the referenced scenario in `docs/09`). |
| **Arbor requirement** | A behavior Arbor MUST implement. Requirements marked **MANDATORY** are non-negotiable (see [06 — Trash Safety](06-trash-safety-requirements.md)). |
| **Design decision** | An independently chosen Arbor behavior where observation was insufficient, ambiguous, or where Arbor deliberately differs from the observed application. |
| **Unknown** | Behavior that could not be established by black-box observation. Arbor is free to choose, but must document its choice. |

Rule for Team B: where an **Arbor requirement** conflicts with **Observed**
behavior of the original application, the requirement wins. This happens
exactly once, deliberately, in [06 — Trash Safety](06-trash-safety-requirements.md).

## Reading order

1. `01-application-window.md` — windows, start page, header bar, menus, About, Preferences
2. `02-navigation-and-interactions.md` — how the user moves around
3. `03-directory-scanning.md` — scan entry points, progress, semantics
4. `04-disk-usage-visualization.md` — list + rings/treemap charts, sizes
5. `05-file-and-folder-actions.md` — context-menu actions
6. `06-trash-safety-requirements.md` — **MANDATORY** safety specification
7. `07-error-handling.md` — all observed error surfaces
8. `08-accessibility-and-keyboard.md` — a11y tree, keyboard map
9. `09-test-scenarios.md` — reproducible observation + acceptance scenarios
10. `10-ui-reference.md` — screenshot index and window inventory
11. `11-provenance-and-observation-log.md` — how everything above was learned

## Core concepts and vocabulary

- **Start page** — the initial view, titled *Devices & Locations*. Lists
  scannable devices/locations and recently scanned folders.
- **Result view** — the two-pane view shown for a scanned location: item list
  (left) and chart (right).
- **Location pill** — the wide header field showing the current location;
  in a result view it contains a clickable breadcrumb.
- **Infobar** — the full-width gray banner under the header
  (“Files may take more space than shown”).
- **Toast** — a transient dark pill at the bottom center of the window with an
  embedded dismiss (✕) button (e.g. “Failed to open file”).
- **Chart modes** — *Rings Chart* and *Treemap Chart*, switched by two toggle
  buttons at the bottom of the chart pane.

## Headline facts (details in dedicated documents)

- Single-window application; window titled after the current location
  (“Devices & Locations” on the start page, the location’s display name in a
  result view).
- Sizes are displayed in **decimal units** (kB / MB / GB) with 3 significant
  digits; each node contributes the **larger of its file length and its disk
  block usage**, and a folder row sums this over its subtree (Observed,
  see `04`).
- Item counts include the folder itself and **exclude symbolic links**;
  symlinks are neither followed nor listed (Observed, see `03`/`04`).
- Hidden (dot-prefixed) files and folders **are** included in scans and lists
  (Observed).
- The observed application moves items to Trash **without any confirmation**;
  Arbor differs — see the MANDATORY specification in `06`.
