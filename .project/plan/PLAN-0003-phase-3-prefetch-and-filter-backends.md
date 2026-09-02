---
id: PLAN-0003
status: completed
roadmap: ROADMAP-0001
phase: phase-3-internal-filter-and-shell-integration
issue: []
review: .project/review/REVIEW-0003-phase-3-internal-filter.md
---

# Plan: Internal Filter and Shell Integration

## Objective

Complete Phase 3 with an always-available built-in simple filter, a small
in-process fuzzy matcher, explicit current-directory navigation, and verified
shell/release integration. The standalone binary, current cache contract, and
shallow scan behavior remain the defaults. Bounded child-directory prefetch is
deferred to Phase 4.

## Scope

- Included: an interactive, case-insensitive literal substring filter over
  directory names, with a clear input mode and safe behavior when no entries
  match.
- Included: a visible-entry mapping that keeps the complete scan result in
  memory and in the cache; filtering must not mutate the authoritative
  `entries` collection.
- Included: a small in-process fuzzy matcher in the existing filter module and
  TUI, reusing the current query input and visible-entry model without an
  external process or mandatory runtime dependency.
- Included: explicit `..` parent and `.` current-directory navigation entries,
  kept separate from cached child-directory data.
- Included: Bash, Zsh, and Nushell wrapper verification and practical checks of
  the existing platform release workflow.
- Deferred: bounded, cancellable child-directory prefetch to the Phase 4 plan.
- Excluded: a mandatory `fzf` executable, a mandatory external database,
  recursive indexing, file browsing, previews, mouse input, and content search.
- Excluded: interactive or non-interactive `fzf` integration and `fzf`-specific
  process or terminal handling.
- Excluded: cache format changes unless a separately reviewed decision makes
  them necessary.

## Acceptance Criteria

- The simple filter works without any external executable or new mandatory
  runtime dependency.
- Fuzzy matching runs in the existing TUI, requires query characters to appear
  in order, uses documented case-insensitive matching and score rules, and has
  stable tie-breaking covered by tests.
- The `..` entry remains available when a parent exists, and the `.` entry
  represents the current directory; confirming either entry selects its path.
- Selection, scrolling, incremental scan chunks, cache hits, and directory
  changes preserve the selected path or apply a defined nearest-visible-entry
  fallback.
- Completed scans persist the complete unfiltered child-directory set, and
  cached data is unaffected by the active query.
- Fuzzy matching does not require `fzf`, and the built-in simple filter remains
  available as the predictable baseline.
- Shell navigation and the supported release targets have been verified in
  practice without requiring an external database or fuzzy-finder executable.

## Step

1. Record the fuzzy matching contract; keep the simple filter as the mandatory
   baseline and cap fuzzy scoring at a small, testable in-process algorithm.
2. Keep `entries` authoritative, add a small filter module and visible-index
   state, and update input handling, selection, scrolling, rendering, and
   incremental scan restoration. Add unit tests for matching, query editing,
   empty results, parent navigation, and selection stability.
3. Extend the filter module with the in-process fuzzy matcher after the simple
   baseline is stable. Preserve stable score ties, selected paths, parent
   navigation, and the complete unfiltered cache data. Do not add `fzf` process
   handling in this phase.
4. Add explicit current-directory navigation without including navigation
   entries in cached child-directory data or scan counts.
5. Verify the Bash, Zsh, and Nushell selection protocol, practical platform
   release packaging, and documented installation paths. Keep release checks
   aligned with the existing manual workflow.
6. Run the phase checks, inspect the complete candidate tree, and complete the
   Phase 3 gate review.

## Affected File Or Interface

- `src/main.rs`
- `src/app.rs`
- `src/cli.rs`
- `src/terminal.rs`
- `src/filter.rs`
- `shell/fast.bash`
- `shell/fast.zsh`
- `shell/fast.nu`
- `README.md`
- `.github/workflows/manual-release.yml`
- `.project/decision/DECISION-0003-in-process-fuzzy-matching.md`

## Risk And Reversibility

- A separate visible-index layer prevents filtering from corrupting cache
  persistence, but selection bugs can still cause an unintended directory to
  be selected; empty-result and path-restoration tests are required.
- A custom fuzzy scorer can become difficult to maintain if it tries to match a
  mature external tool feature-for-feature. A deliberately small contract,
  stable tie-breaking, and focused scoring tests keep the behavior reversible.
- Unicode and platform path behavior remain subject to the initial UTF-8 scope;
  backend-specific path transport must not weaken the existing NUL-terminated
  selection protocol.
- The cache is an optimization. Feature failures must fall back to scanning
  and must not change the directory-navigation contract.
- Prefetch is intentionally deferred; its queue, concurrency, cancellation, and
  cache interaction will be reviewed separately in Phase 4.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `git diff --check`
- Verify fuzzy score ordering, stable ties, case handling, empty results, and
  incremental scan updates without spawning an external process.
- Record the human-reported practical verification of the three shell wrappers,
  including successful selection and cancellation behavior.
- Verify release workflow packaging for Linux x86_64/aarch64, macOS arm64, and
  Windows x86_64, including the absence of a mandatory external fuzzy finder.

## Completion Evidence

- Built-in simple filtering was implemented and committed in `0ed7208`.
- In-process fuzzy matching was implemented and committed in `df00d64`; the
  implementation uses no new dependency and the test suite passes 29 tests.
- The binary source organization was refactored and committed in `a27a411`:
  `main.rs` is now a small entrypoint, while application, CLI, and terminal
  code are separated into dedicated modules. The refactor preserves the 29-test
  passing suite and is recorded in `REVIEW-0002`.
- Explicit `..` parent and `.` current-directory navigation were implemented in
  `3578296`; the test suite passes 34 tests, and navigation entries remain out
  of the cache and directory counts.
- The human reviewer reports that the Bash, Zsh, and Nushell wrappers and the
  release packaging have been exercised in practice.
- The Phase 3 gate is accepted in
  `.project/review/REVIEW-0003-phase-3-internal-filter.md`.
- Bounded child-directory prefetch is deferred to
  `.project/plan/PLAN-0004-phase-4-bounded-prefetch.md`.
