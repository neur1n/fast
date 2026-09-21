---
id: ROADMAP-0001
status: active
project: fast
supersedes: none
review: .project/review/REVIEW-0012-entry-modification-timestamps.md
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
- Phase 2, Phase 3, and the accepted follow-up work through `0.0.7` are complete;
  canonical implementation evidence remains in the linked plans, decisions,
  and reviews.
- Phase 4 bounded child-directory prefetch is deferred and remains proposed in
  `.project/plan/PLAN-0004-phase-4-bounded-prefetch.md`; promotion requires
  measured workload evidence.
- Entry modification timestamps are approved in
  `.project/plan/PLAN-0009-entry-modification-timestamps.md` and
  `DECISION-0010` and were implemented in commit
  `715028818636c6bd26e3fd1a3cb7680e7cc83492`; the implementation is accepted in
  `REVIEW-0012` and does not depend on the deferred Phase 4 prefetch.
- Filesystem scan and cache resilience for target version `0.0.9` is completed
  and accepted in `REVIEW-0013`; it preserves the accepted timestamp baseline
  and does not promote Phase 4 prefetch.
- Terminal clipboard copy through OSC 52 is approved in `PLAN-0011` and
  `DECISION-0012` for target version `0.0.10`; the implementation is completed
  and accepted in `REVIEW-0014`.

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

### Phase 3 Follow-up: Current-Directory Default Selection and 0.0.7

- Objective: Select the current-directory entry by default while preserving
  asynchronous remembered-path restoration and explicit movement priority.
- Gate: Current-directory defaults, late remembered-path restoration, missing
  path fallback, regression coverage, documentation, and synchronized version
  metadata pass automated checks.
- Status: Completed and accepted in `REVIEW-0011`; implementation commit
  `b6966577fba7ece307c1573fb2465debd537aea5` has tree
  `f38667916eb8f559ee220b0a9fc5b6f537f0e8ee`.
- Dependency: phase-3-logical-path-preservation

### Phase 4: Bounded Child-Directory Prefetch

- Objective: Add bounded, cancellable prefetch for direct child-directory
  listings while preserving shallow foreground navigation and the existing
  cache contract.
- Gate: Prefetch queue, concurrency, work bounds, cancellation, cache
  interaction, and the absence of recursive indexing pass automated checks.
- Status: Deferred; `PLAN-0004` remains proposed pending demonstrated need.
- Dependency: phase-3-current-directory-default-selection

### Phase 5: Entry Modification Timestamps

- Objective: Display direct-entry modification timestamps in both listing modes
  while preserving responsive shallow scanning and listing-cache behavior.
- Gate: Fresh scan metadata, cache-hit metadata refresh, cancellation and stale
  result isolation, width-aware rendering, error fallbacks, documentation, and
  synchronized `0.0.8` metadata pass automated checks.
- Status: Completed and accepted in `REVIEW-0012`; implementation commit
  `715028818636c6bd26e3fd1a3cb7680e7cc83492` has candidate tree
  `e03e97dfd18e5a2f74df5f8c2e6c7476729181e1`. This workstream was independent
  of deferred Phase 4 prefetch.
- Dependency: phase-3-current-directory-default-selection

### Phase 6: Filesystem Scan and Cache Resilience

- Objective: Make direct listings robust when filesystems omit entry type data,
  provide a cache-bypassing `r` rescan, and prevent incomplete scans from
  becoming persistent directory-listing authority. Release the work as package
  version `0.0.9`.
- Gate: Unknown/failed type fallback, complete-versus-incomplete scan handling,
  forced rescan cache bypass, valid empty-result caching, legacy cache
  invalidation, regression coverage, documentation, and synchronized `0.0.9`
  metadata pass automated checks.
- Status: Completed and accepted in `REVIEW-0013`; implementation commit
  `1f5634f8c5dc1dc5f6c090a088ebc67f3ce99cfa` has tree
  `c9201f402ad55500bc7de8a810aaf4406a4e411b`.
- Dependency: phase-5-entry-modification-timestamps

### Phase 7: Terminal Clipboard Copy

- Objective: Copy the highlighted logical path to the terminal host clipboard
  with a lowercase `y` shortcut without changing navigation or shell selection
  behavior. Release the work as package version `0.0.10`.
- Gate: OSC 52 emission, direct directory/file/navigation-entry path semantics,
  filter-mode isolation, terminal-support documentation, regression coverage,
  synchronized `0.0.10` package metadata, and locked dependency checks pass.
- Status: Completed and accepted in `REVIEW-0014`; implementation commit
  `cbcfef71e42f7c91f7479fd4ad34aa2908d2eb7f` has tree
  `aac58992078b91623677bf3e05b4c307b503c7e5`.
- Dependency: phase-6-filesystem-scan-and-cache-resilience

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
- Last-modified values use local time, describe each direct entry itself rather
  than recursive contents, and are refreshed per listing load rather than per
  redraw.
- Clipboard copy uses OSC 52 through the terminal host; native clipboard APIs,
  external clipboard executables, paste, and primary-selection support are out
  of scope unless a separately reviewed workstream promotes them.
