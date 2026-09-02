---
id: PLAN-0004
status: proposed
roadmap: ROADMAP-0001
phase: phase-4-bounded-prefetch
issue: []
review: none
---

# Plan: Bounded Child-Directory Prefetch

## Objective

Add bounded, cancellable prefetch of direct child-directory listings while
keeping the normal scan shallow, responsive, and authoritative. Prefetch is an
optimization for the existing cache and must not turn navigation into a
recursive index.

## Scope

- Included: enqueueing direct child directories discovered by the active
  directory scan for background listing and cache population.
- Included: explicit limits for queue length, concurrent work, and total work
  started for one navigation context.
- Included: cancellation when the user navigates, rescans, exits, or replaces
  the active prefetch context.
- Included: reuse of the existing cache format and safe fallback when a
  prefetched listing cannot be read or stored.
- Included: automated coverage for bounds, cancellation, cache interaction, and
  the absence of recursive prefetch.
- Excluded: recursive indexing beyond one direct child level, file entries,
  previews, content search, and a mandatory external database or executable.
- Excluded: cache format changes unless a separately reviewed decision makes
  them necessary.

## Acceptance Criteria

- Prefetch work has explicit queue, concurrency, and per-context work bounds.
- The first screen and normal directory navigation do not wait for prefetch
  completion.
- Replacing the active directory or rescanning cancels obsolete prefetch work,
  and cancelled work cannot publish stale results as current navigation data.
- A prefetched directory is stored only after the existing fingerprint check;
  stale, corrupt, or failed cache work remains safe and falls back to a normal
  scan.
- Prefetch lists only direct children of queued directories and never schedules
  another prefetch level.
- The existing shallow scan, selection behavior, cache format, and shell
  selection protocol remain unchanged when prefetch is unavailable.

## Steps

1. Record queue, concurrency, work, and cancellation limits and define the
   ownership of a prefetch context.
2. Isolate cancellable child-directory work from the foreground scan and make
   its results publishable only through the existing cache contract.
3. Start prefetch only from the completed direct-child result of the active
   directory; enforce bounds before spawning or queueing work.
4. Cancel and replace the prefetch context on navigation, rescan, and exit;
   ensure late worker results cannot affect the active view.
5. Add tests for upper bounds, cancellation, cache hits and misses, scan races,
   and the absence of implicit recursive indexing.
6. Run the phase checks, inspect the candidate tree, and prepare the Phase 4
   implementation gate review.

## Affected File Or Interface

- `src/app.rs`
- `src/scan.rs` or a dedicated prefetch module
- `src/cache.rs`
- `.project/project.json`
- `.project/STATE.md`
- `.project/roadmap/ROADMAP-0001.md`

## Risk And Reversibility

- Background filesystem work can consume threads, descriptors, and I/O; small
  explicit limits and cancellation keep resource use bounded.
- A late result can associate data with the wrong directory; context ownership
  and generation checks must prevent stale publication.
- Prefetch must remain an optimization. Any worker or cache failure must leave
  foreground navigation able to perform its existing normal scan.
- Keeping the current cache format avoids a migration surface; a format change
  requires a separate reviewed decision.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `git diff --check`
- Verify queue, concurrency, and total-work bounds under more eligible child
  directories than the configured limits.
- Verify cancellation during queued and active work, including navigation and
  rescan replacement.
- Verify cache hit, stale cache, cache error, and fingerprint race behavior.
- Verify no worker schedules a directory beyond the configured direct-child
  prefetch level.

## Completion Evidence

- Pending Phase 4 plan review and implementation.
