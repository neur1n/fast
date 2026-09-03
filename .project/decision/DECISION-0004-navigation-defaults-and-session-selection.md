---
id: DECISION-0004
status: proposed
date: 2026-09-02
supersedes: none
review: none
---

# Decision: Navigation Defaults and Session Selection

## Context

The built-in fuzzy matcher is already available alongside the case-insensitive
substring matcher. The current application defaults to substring matching and
resets that mode when a directory is opened or rescanned. The current
selection is an index into the visible list, so starting a new directory scan
also loses the selection from the directory being left.

The navigator should favor fuzzy matching for interactive use while retaining
substring matching as an explicit predictable alternative. It should also
remember directory positions during one navigation session without adding
state to the persistent directory cache.

## Options

1. Keep substring matching as the default and leave selection state transient.
2. Make the built-in fuzzy matcher the default, keep substring matching behind
   the existing toggle, and remember selections by numeric list position.
3. Make the built-in fuzzy matcher the default, keep substring matching behind
   the existing toggle, and remember selections by directory and entry path in
   process memory.

## Decision

Use the built-in fuzzy matcher as the default filter mode. Keep the built-in
substring matcher available through the existing `Tab` toggle in filter mode.
The fuzzy default applies when the application starts and whenever a new scan
context is started by navigation or rescan; the existing filter query reset
behavior remains unchanged.

Remember selection state only for the lifetime of the current process. Key the
state by directory path and store the selected entry path rather than a
numeric index. Restore the selected path after a cache hit or after the
corresponding asynchronous scan result becomes available. When the saved
entry is no longer visible, use the nearest valid visible position.

When entering a never-visited child directory, retain the current behavior of
initially selecting `..`. When returning to a parent directory, select the
child directory that was just left. Do not persist this state in the cache or
in a separate file.

## Rationale

Fuzzy matching is already implemented in-process and does not add an external
dependency. Keeping substring matching as a toggle preserves a predictable
literal search mode without requiring a second backend.

Paths are more stable than numeric positions because directory entries can be
rescanned, sorted, filtered, or reordered by fuzzy scores. Process-local state
avoids cache format changes and prevents navigation history from becoming
persistent user data.

## Consequence

- A non-empty filter query uses fuzzy matching by default.
- Users can switch to substring matching with `Tab` and switch back without
  changing the authoritative unfiltered entries or cache contents.
- Returning from a child directory restores the corresponding parent entry
  when it still exists.
- The selection map is discarded when `fast` exits; cross-process restoration is
  out of scope.
- Prefetch remains governed separately by the proposed `PLAN-0004` and is not
  part of this decision.

## Affected Record Or Consumer

- `.project/plan/PLAN-0005-phase-3-navigation-ux-refinements.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/STATE.md`
- `src/app.rs`
- `README.md`
