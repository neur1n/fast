---
id: ROADMAP-0001
status: active
project: fast
supersedes: none
review: none
---

# Roadmap: fast

## Outcome

Provide a small standalone TUI that lets a user browse directories and change
the parent shell's working directory without recursively scanning the file
system by default.

## Success Criterion

- A cache miss can render the first available directory entries before the
  complete scan finishes and shows that indexing is still in progress.
- A valid cache can be displayed after a directory fingerprint check without a
  complete directory read.
- A changed or invalid cache is rebuilt atomically without exposing partial
  index data as complete.
- Release artifacts target Linux x86_64/aarch64, macOS arm64, and Windows
  x86_64.

## Current Progress

- Phase 0 foundation is committed in `e754b31`; its formal exact-tree review
  is not recorded.
- Phase 1 navigation and cancellable chunked scanning are implemented in
  `07fdac4`.
- Bash, Zsh, and Nushell shell wrappers were delivered with the Phase 1
  implementation.
- Phase 2 persistent caching was committed in `882c4ac` with candidate tree
  `bd4b10ed717f89585ba24b54daef9f50afab51b5`; its gate was accepted in
  `REVIEW-0001`.
- The manual release workflow and platform packaging steps are present; their
  end-to-end release checks remain part of Phase 3.
- Bounded prefetch, built-in filtering, and an optional fuzzy-finder backend are
  not yet implemented. The Phase 3 plan is proposed in
  `.project/plan/PLAN-0003-phase-3-prefetch-and-filter-backends.md`.

## Phase

### Phase 0: Foundation and Contract

- Objective: Establish the repository governance, formatting, scope, and
  review workflow.
- Gate: Human initializes the repository with `main`, reviews the candidate
  tree, and commits the approved foundation.
- Status: The repository and foundation commit exist, but the formal review is
  not recorded in the governance records.
- Dependency: none

### Phase 1: Minimal Navigator

- Objective: Implement a directory-only, single-column TUI with chunked
  scanning, cancellation, and an indexing indicator.
- Gate: The first screen is available during a cold scan and the UI remains
  responsive while the scan continues.
- Status: Implemented in `07fdac4`; the phase gate review is not recorded.
- Dependency: phase-0-foundation

### Phase 2: Persistent Directory Cache

- Objective: Add a versioned, crash-safe cache for visited directories with
  fingerprint validation and bounded storage.
- Gate: Cache hits, invalidation, corruption, concurrent writers, and scan
  races have automated coverage.
- Status: Completed in `882c4ac`; the phase gate was accepted in `REVIEW-0001`.
- Dependency: phase-1-minimal-navigator

### Phase 3: Prefetch and Shell Integration

- Objective: Add bounded child-directory prefetch, an always-available
  built-in simple filter, and an optional fuzzy backend selected after baseline
  evaluation. Verify release packaging and Nushell/Bash/Zsh wrappers, which
  are already implemented.
- Gate: Shell navigation, bounded prefetch, filter behavior, and platform
  release checks pass without a mandatory external database or fuzzy-finder
  executable.
- Status: Phase 3 plan proposed in `PLAN-0003`; implementation awaits human
  review and approval.
- Dependency: phase-2-persistent-directory-cache

## Assumption

- Rust is the proposed initial implementation language.
- The initial browser lists directories only and does not support mouse input.
- The default scan is shallow; prefetch is bounded and cancellable. Simple
  filtering is built in; fuzzy matching remains optional.
- The initial supported character set is UTF-8 with core cross-platform path
  and link behavior only.
- SQLite remains an implementation option; if selected, it must be bundled so
  users do not need a system SQLite installation or the `sqlite3` CLI.
- No fixed latency or binary-size target has been approved yet; both will be
  measured before optimization decisions are finalized.
