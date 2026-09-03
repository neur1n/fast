---
id: PLAN-0006
status: completed
roadmap: ROADMAP-0001
phase: phase-3-navigation-ux-default-selection
issue: []
review: .project/review/REVIEW-0006-navigation-default-selection-and-version-0.0.4.md
---

# Plan: Navigator Default Selection and 0.0.4

## Objective

Refine the navigator's initial selection so a directory with no remembered
selection opens on its first actual child directory, while preserving path-based
restoration and parent navigation. Release the change as package version
`0.0.4` without changing the cache contract or shell selection protocol.

## Scope

- Included: selecting the first non-navigation entry after `..` and `.` when
  the current directory has no remembered selection.
- Included: applying the default during cache hits and when the first result of
  a cold scan arrives in a chunk.
- Included: preserving remembered paths, parent-return restoration, and the
  first-child fallback when a remembered path disappears.
- Included: selecting the current-directory navigation entry as the fallback
  when no child directory exists.
- Included: making explicit selection movement take priority over a pending
  automatic selection or restoration target.
- Included: regression coverage for cold scans, cache hits, empty directories,
  missing remembered entries, and manual movement during scanning.
- Included: updating user-facing navigation documentation and package version
  metadata to `0.0.4`.
- Excluded: persistent selection state, cache format changes, recursive scans,
  bounded prefetch, and new runtime dependencies.

## Acceptance Criteria

- In a non-root list shaped as `..`, `.`, `alpha`, `beta`, a new directory
  selects `alpha` by default.
- In a root list shaped as `.`, `alpha`, `beta`, a new directory selects
  `alpha` by default.
- A cache hit and the first non-empty cold-scan chunk apply the same default.
- A remembered entry path overrides the first-child default, including when the
  remembered entry arrives in a later scan chunk.
- A missing remembered entry falls back to the first available child and then
  to the current-directory navigation entry when there are no children.
- `Up`, `Down`, `Home`, and `End` cancel a pending automatic target and retain
  the user's explicit selection as later scan chunks arrive.
- The selection map remains process-local and does not alter cached records or
  shell output.
- The package manifest and lockfile identify the root package as `0.0.4`.

## Steps

1. Record the revised initial-selection and pending-target priority rules.
2. Represent remembered and first-child pending targets separately so an
   asynchronous scan can distinguish restoration from a new-directory default.
3. Select the first available child after navigation entries, with a safe
   navigation fallback, across cold scans and cache hits.
4. Cancel pending automatic selection when the user explicitly moves the
   selection and add regression coverage for the timing-sensitive behavior.
5. Update navigation documentation and synchronize package version metadata.
6. Run the implementation checks and prepare the exact-tree review candidate.

## Affected File Or Interface

- `src/app.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
- `.project/project.json`
- `.project/STATE.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/decision/DECISION-0006-navigation-default-selection.md`
- `.project/review/REVIEW-0006-navigation-default-selection-and-version-0.0.4.md`

## Risk And Reversibility

- A first-child target can become available before or after the foreground
  scan finishes; keeping it distinct from a remembered path prevents a stale
  navigation fallback from becoming permanent.
- Explicit movement during a scan must not be overwritten by a later chunk;
  clearing the pending target on movement preserves user intent.
- Path-based restoration remains stable across sorting and filtering, and the
  change is reversible without a cache migration because selection state is not
  persisted.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- Verify first-child selection for non-root and root navigation layouts.
- Verify cache-hit and incremental cold-scan selection timing.
- Verify remembered-path priority, missing-entry fallback, and manual movement
  priority.
- Verify the package version and default `--version` output report `0.0.4`.

## Completion Evidence

- Implemented separate remembered-path and first-child pending selection
  targets in `src/app.rs`.
- New directories select the first child after navigation entries during cold
  scan chunks and cache hits; empty directories fall back to `.`.
- Existing remembered-path restoration, parent-return restoration, and missing
  entry fallback remain path-based and process-local.
- Explicit `Up`, `Down`, `Home`, and `End` movement cancels a pending automatic
  target.
- Added regression coverage for non-root and root defaults, empty directories,
  cache hits, incremental scans, missing entries, and manual movement.
- The package manifest and lockfile were synchronized to `0.0.4`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --
  -D warnings`, `cargo test --locked`, `cargo check --locked`, and
  `git diff --check` passed; the test suite contains 45 tests.
- `cargo run --quiet -- --version` reported `fast 0.0.4`.
- Human exact-tree review remains pending in `REVIEW-0006`.
