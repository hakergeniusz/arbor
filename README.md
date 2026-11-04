# Arbor — Behavioral Specification

This repository contains the **Team A deliverable** for the Arbor project: a
clean-room, black-box behavioral specification of the user-facing behavior of
GNOME Baobab (Disk Usage Analyzer), written as requirements for **Arbor**, an
independent Rust implementation.

## For Team B (implementation agent)

Read the documents in `docs/` in order. Your contract is:

- `docs/00-overview.md` — scope, reading order, requirement classes
- `docs/01-…09-…` — feature specifications, each separating *Observed
  behavior*, *Arbor requirement*, *Design decision*, and *Unknown*
- `docs/06-trash-safety-requirements.md` — **mandatory safety specification**,
  read before implementing any destructive action
- `docs/09-test-scenarios.md` — reproducible acceptance scenarios
- `docs/10-ui-reference.md` — visual reference with screenshots
- `docs/11-provenance-and-observation-log.md` — how the knowledge in this
  repository was obtained

You must implement from these documents only. Do not seek or use GNOME
Baobab's source code, and do not treat the screenshots as more than visual
reference for layout intent.

## Provenance statement

All observations in `docs/` were made by interacting with the installed
`/usr/bin/baobab` (version 50.0) as an ordinary user inside an isolated Xvfb
display, recording visible behavior and screenshots. No upstream source code,
tests, or internal documentation was consulted. See
`docs/11-provenance-and-observation-log.md` for the full log.

## Repository layout

```
docs/           Behavioral specification (numbered reading order)
screenshots/    Original screenshots captured during observation
src/            (empty — reserved for Team B's Rust implementation)
```
