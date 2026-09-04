---
id: ROADMAP-0001
status: active
project: fast
supersedes: none
review: .project/review/REVIEW-0008-file-visibility-version-0.0.5.md
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
- The manual release workflow and platform packaging steps are present and
  were exercised in practice as part of the Phase 3 verification.
- Built-in simple filtering was implemented in `0ed7208`, and the in-process
  fuzzy matcher was implemented in `df00d64`. The binary source was reorganized
  into dedicated application, CLI, and terminal modules in `a27a411`, with the
  entrypoint reduced to top-level orchestration; that approved refactor is
  recorded in `REVIEW-0002`.
- Explicit `..` parent and `.` current-directory navigation were implemented in
  `3578296`; the test suite passes 34 tests and navigation entries remain out of
  cached child-directory data. The Phase 3 implementation and practical
  wrapper/release verification are accepted in `REVIEW-0003`.
- Navigator UX refinements are implemented and accepted in the code candidate
  recorded by `REVIEW-0004`; the plan is
  `.project/plan/PLAN-0005-phase-3-navigation-ux-refinements.md`, and the
  behavior is recorded in
  `.project/decision/DECISION-0004-navigation-defaults-and-session-selection.md`.
- Bounded child-directory prefetch remains deferred. It is retained as the
  proposed `.project/plan/PLAN-0004-phase-4-bounded-prefetch.md` and will be
  revisited if measured workloads demonstrate a need beyond the current
  chunked scan behavior. The Phase 3 plan and accepted fuzzy-matching decision
  are recorded in
  `.project/plan/PLAN-0003-phase-3-prefetch-and-filter-backends.md` and
  `.project/decision/DECISION-0003-in-process-fuzzy-matching.md`.
- Initial selection refinement and the `0.0.4` package candidate are implemented
  and accepted in `REVIEW-0006`; their candidate tree and commit identifiers are
  pending staging and commit. The work is governed by
  `.project/decision/DECISION-0006-navigation-default-selection.md`.
- On-demand file visibility and browse-only confirmation are implemented and
  accepted in `REVIEW-0007`; directory-first grouping, a Files label, dimmed
  file rows, and the exact candidate tree and commit are recorded there. The
  synchronized `0.0.5` package metadata and governance follow-up are pending
  exact-tree review in `REVIEW-0008`.

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
- Status: Completed and accepted in `REVIEW-0004`; the code commit and
  candidate tree identifiers are pending recording.
- Dependency: phase-3-internal-filter-and-shell-integration

### Phase 3 Follow-up: Default Selection Refinement and 0.0.4

- Objective: Select the first actual child in directories without remembered
  selection, preserve explicit movement during asynchronous scans, and release
  the behavior as package version `0.0.4`.
- Gate: First-child defaults, cache and chunk timing, remembered-path fallback,
  manual movement priority, and synchronized version metadata pass automated
  checks.
- Status: Completed and accepted in `REVIEW-0006`; candidate tree and commit
  identifiers remain pending staging and commit.
- Dependency: phase-3-navigation-ux-refinements

### Phase 3 Follow-up: File Visibility

- Objective: Show direct non-directory entries on demand while keeping the
  navigator browse-only, shallow, chunked, and compatible with the directory
  cache and shell selection protocol.
- Gate: Runtime visibility toggling, mixed file/directory chunking, safe
  non-directory actions, directory-first grouping, Files labeling, cache
  isolation, and documentation pass automated checks.
- Status: Runtime implementation completed and accepted in `REVIEW-0007`;
  synchronized `0.0.5` package metadata is pending exact-tree review in
  `REVIEW-0008`.
- Dependency: phase-3-navigation-ux-default-selection

### Phase 4: Bounded Child-Directory Prefetch

- Objective: Add bounded, cancellable prefetch for direct child-directory
  listings while preserving shallow foreground navigation and the existing
  cache contract.
- Gate: Prefetch queue, concurrency, work bounds, cancellation, cache
  interaction, and the absence of recursive indexing pass automated checks.
- Status: Deferred; `PLAN-0004` remains proposed pending demonstrated need.
- Dependency: phase-3-internal-filter-and-shell-integration

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
