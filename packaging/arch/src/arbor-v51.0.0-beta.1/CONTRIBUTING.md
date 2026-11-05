# Contributing to Arbor

Thank you for considering a contribution. Arbor is a clean-room
implementation of a behavioral specification — please keep that property
intact.

## Clean-room rules (hard constraints)

- Never read, clone, or decompile GNOME Baobab's source code, and never
  paste source-derived snippets, symbol names, or internal details into
  issues, code, or documentation.
- The `tools/` directory (Team A's observation harness) must not be
  inspected or executed as part of implementation work.
- Do not launch `/usr/bin/baobab` as part of implementing or debugging.
- If you find behavior that the spec marks *Unknown*, observe **Arbor's**
  behavior, decide explicitly, and record the decision.

## Development

```
cargo build          # debug build
cargo test           # unit + scanner tests (no GUI required)
```

GUI checks run in an isolated Xvfb session with a private bus and a fake
`HOME` — never on the developer's real display, and any destructive test
must operate **only on files the test created** inside disposable
temporary directories (redirect `XDG_DATA_HOME` for anything touching the
Trash). See `IMPLEMENTATION_NOTES.md` for harness details.

## Code style

- Rust 2024 edition; keep `cargo build` warning-free.
- Follow the gtk4-rs idioms already in the tree: weak-obj closures, imp
  modules, `#[allow(deprecated)]` with a comment where unavoidable.
- Comments cite the spec: `docs/04 §3`, `docs/06 R5`, scenario ids (S19,
  A1–A8) — every non-obvious behavior should trace back to a document.

## Commits

- Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:`,
  `build:`), small and single-purpose.
- Never force-push shared branches; never `git add .` blindly — review
  what you stage.
- Team A's files (`docs/`, `screenshots/`, `tools/`) are inputs, not
  implementation surfaces; propose spec changes in an issue first.

## Reporting bugs

Include the Arbor version, the exact dialog/toast text, and — for trash
issues — the state of the `.trashinfo` files involved. Trash-safety
regressions (docs/06) are release blockers.
