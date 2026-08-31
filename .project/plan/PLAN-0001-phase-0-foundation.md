---
id: PLAN-0001
status: implemented
roadmap: ROADMAP-0001
phase: phase-0-foundation
issue: []
review: none
---

# Plan: Project Foundation and Initial Contract

## Objective

Create the minimum documented foundation for `fast` so implementation can
start from a reviewed scope, consistent formatting rules, and the
phased-development workflow.

## Scope

- Included: governance records, repository instructions, README, Git ignore
  rules, EditorConfig, and Rustfmt configuration.
- Excluded: application code, TUI dependencies, SQLite or custom cache code,
  shell wrappers, and release automation.

## Step

1. The repository was initialized with `main` and the foundation was committed
   in `e754b31`.
2. The foundation files were committed; a formal exact-tree review is not
   recorded in the governance records.
3. Before Phase 2 implementation, confirm the cache storage backend and cache
   contract through a separate reviewed plan or decision record. Phase 1 uses
   `crossterm`, but that backend choice has not been recorded as a reviewed
   decision.

## Affected File Or Interface

- `AGENTS.md`
- `.editorconfig`
- `rustfmt.toml`
- `README.md`
- `.gitignore`
- `.gitattributes`
- `.project/project.json`
- `.project/.gitignore`
- `.project/STATE.md`
- `.project/decision/DECISION-0001-v1-scope-and-language.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/plan/PLAN-0001-phase-0-foundation.md`

## Risk And Reversibility

- The foundation contains no application behavior and can be revised before
  implementation.
- Rust is recorded as a proposed initial choice; the storage backend remains
  open until a small prototype and size/maintenance review.
- The two-space Rustfmt choice is deliberate and can be changed through a
  reviewed configuration update if project conventions change.

## Verification

- Parse `.project/project.json` as JSON.
- Confirm all text files use LF line endings and end with a newline.
- Confirm Markdown keeps `trim_trailing_whitespace = false`.
- Confirm no application implementation or external runtime dependency was
  introduced.
- Human performs Git initialization, staging, commit, and exact-tree review.

## Completion Evidence

- Foundation files are present in commit `e754b31` from 2026-08-30.
- The subsequent Phase 1 implementation commit `07fdac4` confirms that
  application work has started; that work is outside this plan's scope.
