---
id: DECISION-0003
status: proposed
date: 2026-08-31
supersedes: none
review: none
---

# Decision: In-Process Fuzzy Matching

## Context

The built-in case-insensitive substring filter is implemented in `0ed7208`.
Phase 3 also needs fuzzy matching, but the implementation must remain easy to
maintain and must not make the standalone TUI depend on an external executable.
An interactive `fzf` process would need to take over the terminal currently
owned by `fast`, while a non-interactive `fzf` process would add process and
data-transfer overhead without providing its interactive UI.

## Option

1. Add a small in-process fuzzy matcher to the existing filter module and keep
   the current TUI responsible for input and rendering.
2. Launch interactive `fzf` as an external backend, temporarily handing it the
   terminal and restoring `fast` afterward.
3. Launch non-interactive `fzf` as an external scorer, passing the query and
   candidate list through process I/O.

## Decision

Use a small in-process fuzzy matcher in the existing TUI. Keep the built-in
substring matcher as the baseline and do not add an `fzf` backend to the current
Phase 3 implementation.

The matcher will use a deliberately limited contract: query characters must
appear in order, matching is case-insensitive under the existing UTF-8 scope,
and explicit score bonuses may prefer contiguous, word-boundary, or earlier
matches. The scoring rules must be documented and covered by tests; matching
`fzf` exactly is not a goal.

## Rationale

The in-process approach reuses the current query input, visible-index model,
selection restoration, and rendering lifecycle. A small pure matcher is easier
to test and maintain than terminal handoff or an external process protocol. It
also preserves the standalone distribution goal and avoids a new mandatory
runtime dependency.

## Consequence

- Fuzzy matching remains available without `fzf`, a shell command, or a second
  terminal screen.
- The full `entries` collection and cache remain unfiltered; fuzzy scores only
  affect visible ordering and selection behavior.
- Stable tie-breaking, parent navigation, empty results, and incremental scan
  updates are part of the matcher integration contract.
- The initial implementation should use the standard library only. Adding a
  fuzzy-matching dependency requires a separate reviewed decision.
- An `fzf` integration remains a possible future explicit backend, but it must
  be proposed and reviewed separately with terminal lifecycle and fallback
  behavior specified.

## Affected Record Or Consumer

- `.project/plan/PLAN-0003-phase-3-prefetch-and-filter-backends.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/STATE.md`
- `src/filter.rs`
- `src/main.rs`
