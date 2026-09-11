---
id: ROADMAP-0001
status: active
project: fast
supersedes: none
review: .project/review/REVIEW-0009-logical-path-preservation-and-version-0.0.6.md
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

- The foundation and Phase 1 implementation are committed, but formal
  exact-tree review records for those early transitions are not recorded.
- Phase 2, Phase 3, and the accepted follow-up work through `0.0.6` are complete;
  canonical implementation evidence remains in the linked plans, decisions,
  and reviews.
- Phase 4 bounded child-directory prefetch is deferred and remains proposed in
  `.project/plan/PLAN-0004-phase-4-bounded-prefetch.md`; promotion requires
  measured workload evidence.

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

### Phase 3: Internal Filter and Shell Integration

- Objective: Provide built-in simple and fuzzy directory filtering, explicit
  current-directory navigation, and verified shell/release integration without
  requiring `fzf`.
- Gate: Current-directory navigation, simple and fuzzy filter behavior, shell
  navigation, and platform release checks pass without a mandatory external
  database or fuzzy-finder executable.
- Status: Completed in `3578296` and accepted in `REVIEW-0003`.
- Dependency: phase-2-persistent-directory-cache

### Phase 3 Follow-up: Navigator UX Refinements

- Objective: Make fuzzy filtering the default and preserve directory selection
  within the current process without changing the cache contract.
- Gate: Default filter behavior, substring fallback, parent selection
  restoration, asynchronous scan restoration, and safe missing-entry fallback
  pass automated checks.
- Status: Completed and accepted in `REVIEW-0004`; commit
  `94b467348b30536b049a1f24565f129ee2f37359` has candidate tree
  `556be641f9a7d51fbce0acb4d67686593bb7dbc0`.
- Dependency: phase-3-internal-filter-and-shell-integration

### Phase 3 Follow-up: Default Selection Refinement and 0.0.4

- Objective: Select the first actual child in directories without remembered
  selection, preserve explicit movement during asynchronous scans, and release
  the behavior as package version `0.0.4`.
- Gate: First-child defaults, cache and chunk timing, remembered-path fallback,
  manual movement priority, and synchronized version metadata pass automated
  checks.
- Status: Completed and accepted in `REVIEW-0006`; candidate tree and commit
  identifiers are recorded as commit `0b9c725edb8f557a4d679523174b1a5954992fac`
  with tree `96076c882aadfc4653917812ecf49138d843c110`.
- Dependency: phase-3-navigation-ux-refinements

### Phase 3 Follow-up: File Visibility

- Objective: Show direct non-directory entries on demand while keeping the
  navigator browse-only, shallow, chunked, and compatible with the directory
  cache and shell selection protocol.
- Gate: Runtime visibility toggling, mixed file/directory chunking, safe
  non-directory actions, directory-first grouping, Files labeling, cache
  isolation, and documentation pass automated checks.
- Status: Runtime implementation and synchronized `0.0.5` package metadata are
  completed and accepted in `REVIEW-0007` and `REVIEW-0008`.
- Dependency: phase-3-navigation-ux-default-selection

### Phase 3 Follow-up: Logical Symlink Paths and 0.0.6

- Objective: Preserve valid shell logical paths through startup and keep parent
  navigation within the symlink path.
- Gate: Valid `PWD` recovery, safe fallback behavior, symlink regression
  coverage, and synchronized `0.0.6` package metadata pass automated checks.
- Status: Completed and accepted in `REVIEW-0009`; implementation commit
  `a824db7760426ebb908f4ef439ff0de0ed18ab33` has tree
  `cec199816f4e7c54631aaec45a156e35f57fd467`.
- Dependency: phase-3-file-visibility

### Phase 4: Bounded Child-Directory Prefetch

- Objective: Add bounded, cancellable prefetch for direct child-directory
  listings while preserving shallow foreground navigation and the existing
  cache contract.
- Gate: Prefetch queue, concurrency, work bounds, cancellation, cache
  interaction, and the absence of recursive indexing pass automated checks.
- Status: Deferred; `PLAN-0004` remains proposed pending demonstrated need.
- Dependency: phase-3-logical-path-preservation

## Assumption

- Rust is the proposed initial implementation language.
- The initial browser starts with directories only and does not support mouse
  input; direct non-directory entries can be shown with the runtime `F` toggle.
- Every foreground scan is shallow; the deferred prefetch is bounded and
  cancellable. Simple and fuzzy filtering are built in; external `fzf` remains
  out of scope.
- The initial supported character set is UTF-8 with core cross-platform path
  and link behavior only.
- SQLite remains an implementation option; if selected, it must be bundled so
  users do not need a system SQLite installation or the `sqlite3` CLI.
- No fixed latency or binary-size target has been approved yet; both will be
  measured before optimization decisions are finalized.
