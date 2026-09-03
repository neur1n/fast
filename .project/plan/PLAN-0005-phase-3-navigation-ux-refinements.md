---
id: PLAN-0005
status: completed
roadmap: ROADMAP-0001
phase: phase-3-navigation-ux-refinements
issue: []
review: .project/review/REVIEW-0004-phase-3-navigation-ux.md
---

# Plan: Navigator UX Refinements

## Objective

Make the built-in fuzzy matcher the default interactive filter and preserve
the user's directory selection during navigation within one process. Keep the
existing shallow scan, cache contract, substring fallback, and shell selection
protocol unchanged.

## Scope

- Included: using `FilterKind::Fuzzy` as the initial filter mode and as the
  default after navigation or rescan.
- Included: retaining the existing in-process substring matcher as the
  `Tab`-selectable alternative.
- Included: an in-memory per-directory selection map keyed by directory path
  and selected entry path.
- Included: restoring selection after cache hits and asynchronous scan chunks.
- Included: selecting the child just left when returning to its parent.
- Included: nearest-visible fallback when a remembered entry is unavailable.
- Included: automated coverage for the default mode, toggle behavior,
  navigation restoration, scan timing, cache hits, and missing-entry fallback.
- Included: documentation of the new default filter behavior.
- Excluded: persistent selection state across process launches.
- Excluded: selection state in the directory cache or any cache format change.
- Excluded: child-directory prefetch, recursive indexing, external fuzzy-finder
  processes, and new runtime dependencies.

## Acceptance Criteria

- A new application starts with fuzzy filtering selected.
- Opening a directory or rescanning starts with fuzzy filtering selected while
  preserving the existing filter query reset behavior.
- `Tab` switches between fuzzy and substring matching in both directions.
- A directory selected from a parent can be opened and then returned from with
  that same parent entry selected when it still exists.
- A previously visited directory restores its remembered entry path after a
  cache hit and after a cold scan whose results arrive in chunks.
- If the remembered entry disappears or is no longer visible, selection falls
  back to a valid nearest visible entry without selecting an invalid path.
- Selection state is held only in process memory and does not alter cache
  records or shell selection output.
- Existing fuzzy scoring, substring matching, shallow scanning, and navigation
  entries remain behaviorally compatible apart from the documented default
  mode change.

## Steps

1. Implement the default fuzzy mode at application initialization and every
   scan-context reset, then update the user-facing filter documentation.
2. Add process-local path-based selection state and a pending restoration path
   so cache results and asynchronous scan chunks can restore the right entry.
3. Define navigation transitions so entering a new directory keeps the
   `..`-first behavior while returning to a parent targets the child just left.
4. Add regression tests for default mode, filter toggling, parent restoration,
   cached and chunked scans, and nearest-visible fallback.
5. Run the phase checks, inspect the candidate tree, and prepare the
   implementation gate review.

## Affected File Or Interface

- `src/app.rs`
- `README.md`
- `.project/decision/DECISION-0004-navigation-defaults-and-session-selection.md`

## Risk And Reversibility

- A selection target can arrive before or after the foreground scan, so
  restoration must be tied to the active directory and scan context.
- Fuzzy filtering can reorder visible entries; path-based state avoids treating
  a stale numeric index as the user's intended selection.
- A removed or filtered entry must never produce an invalid selection; the
  nearest-visible fallback keeps navigation safe.
- The change is reversible without a cache migration because selection state is
  not persisted and the existing substring matcher remains available.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `git diff --check`
- Verify default fuzzy behavior at startup, after navigation, and after rescan.
- Verify selection restoration across parent navigation, cache hits, and
  incremental cold scans.
- Verify missing-entry fallback and unchanged cache contents.

## Completion Evidence

- Implemented fuzzy-default filtering and process-local path-based selection
  restoration in `src/app.rs`.
- Navigation now restores the child just left, cache hits and asynchronous scan
  chunks restore remembered entries, and missing entries fall back safely.
- `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo test --locked` passed; the test suite contains 38 tests.
- `git diff --cached --check` passed for the accepted code candidate.
- The human reviewer accepted the code candidate. Its commit and candidate tree
  identifiers remain pending until the staged code is committed.
- Bounded child-directory prefetch remains deferred to `PLAN-0004`.
