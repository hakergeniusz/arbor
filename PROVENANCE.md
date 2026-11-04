# Provenance

This document records what information the Arbor implementation was built
from, and — equally important — what was **not** consulted.

## Sources used (allowed)

- `docs/00` – `docs/11`: Team A's black-box behavioral specification of
  GNOME Baobab 50.0, including the requirements checklist
  (`docs/IMPLEMENTATION_PLAN.md`) and the trash-safety specification
  (`docs/06-trash-safety-requirements.md`).
- `screenshots/`: Team A's UI reference screenshots, used only as visual
  layout references, never as source-derived material.
- Public, general documentation: the gtk4-rs / libadwaita-rs API
  references, GTK4/libadwaita developer documentation, and standard Linux
  trash-specification knowledge (freedesktop.org Trash spec) needed to
  implement Undo generically.

## Not consulted (clean-room rules)

- GNOME Baobab source code, in any form: not read, not cloned, not
  decompiled, not grepped for symbols or strings.
- The `tools/` scripts shipped with Team A's observation harness — they
  were neither inspected nor executed by the implementation effort.
- The `/usr/bin/baobab` binary was never launched during implementation,
  and no strings, resources, or GSettings schemas were extracted from it.
- No source-derived artifacts (Doxygen output, Debian source packages,
  GitLab snippets, blog posts embedding upstream code).

Everything in `src/`, `packaging/`, and the project documentation was
written from the specification documents and from the implementer's own
observation of **Arbor itself** running in an isolated Xvfb session.

## Relationship to the specification

Where the specification records *Unknown* or environment-dependent
behavior, Arbor makes an explicit design decision (numbered D1–D20 in
`docs/IMPLEMENTATION_PLAN.md`); deviations from the plan are listed in
`IMPLEMENTATION_NOTES.md`.

## License

Arbor is MIT-licensed (see `LICENSE`); third-party components are audited
in `THIRD_PARTY_LICENSES.md`. The specification documents and screenshots
in `docs/` and `screenshots/` are part of this repository and describe
behavior only — they contain no upstream code or assets.
