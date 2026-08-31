---
id: PLAN-0003
status: proposed
roadmap: ROADMAP-0001
phase: phase-3-prefetch-and-shell-integration
issue: []
review: none
---

# Plan: Prefetch and Filter Backends

## Objective

Complete Phase 3 with bounded cancellable child-directory prefetch, an
always-available built-in simple filter, a small in-process fuzzy matcher, and
verified shell/release integration. The standalone binary, current cache
contract, and shallow scan behavior remain the defaults.

## Scope

- Included: bounded, cancellable prefetch for direct child directories without
  turning navigation into an implicit recursive index.
- Included: an interactive, case-insensitive literal substring filter over
  directory names, with a clear input mode and safe behavior when no entries
  match.
- Included: a visible-entry mapping that keeps the complete scan result in
  memory and in the cache; filtering must not mutate the authoritative
  `entries` collection.
- Included: a small in-process fuzzy matcher in the existing filter module and
  TUI, reusing the current query input and visible-entry model without an
  external process or mandatory runtime dependency.
- Included: Bash, Zsh, and Nushell wrapper verification and end-to-end checks
  for the existing platform release workflow.
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
- The `..` entry remains available for parent navigation, and confirming with
  no visible entry cannot accidentally select the current directory.
- Selection, scrolling, incremental scan chunks, cache hits, and directory
  changes preserve the selected path or apply a defined nearest-visible-entry
  fallback.
- Completed scans persist the complete unfiltered child-directory set, and
  cached data is unaffected by the active query.
- Prefetch has explicit queue/concurrency and work bounds, stops on navigation
  cancellation, and cannot make the UI wait for recursive work.
- Fuzzy matching does not require `fzf`, and the built-in simple filter remains
  available as the predictable baseline.
- Shell navigation and the supported release targets pass their verification
  checks without requiring an external database or fuzzy-finder executable.

## Step

1. Record the fuzzy matching contract and prefetch limits before implementation;
   keep the simple filter as the mandatory baseline and cap fuzzy scoring at a
   small, testable in-process algorithm.
2. Keep `entries` authoritative, add a small filter module and visible-index
   state, and update input handling, selection, scrolling, rendering, and
   incremental scan restoration. Add unit tests for matching, query editing,
   empty results, parent navigation, and selection stability.
3. Extend the filter module with the in-process fuzzy matcher after the simple
   baseline is stable. Preserve stable score ties, selected paths, parent
   navigation, and the complete unfiltered cache data. Do not add `fzf` process
   handling in this phase.
4. Add bounded child-directory prefetch using cancellable work and the existing
   cache as an optimization. Test bounds, cancellation, cache interaction, and
   the absence of implicit recursive indexing.
5. Verify the Bash, Zsh, and Nushell selection protocol, platform release
   packaging, and documented installation paths. Keep release checks aligned
   with the existing manual workflow.
6. Run the phase checks, inspect the complete candidate tree, and prepare the
   implementation gate review.

## Affected File Or Interface

- `src/main.rs`
- `src/filter.rs`
- `src/scan.rs` or a dedicated prefetch module
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
- Prefetch can consume threads, descriptors, and I/O unexpectedly; explicit
  bounds, cancellation, and no-recursion tests keep it reversible.
- A custom fuzzy scorer can become difficult to maintain if it tries to match a
  mature external tool feature-for-feature. A deliberately small contract,
  stable tie-breaking, and focused scoring tests keep the behavior reversible.
- Unicode and platform path behavior remain subject to the initial UTF-8 scope;
  backend-specific path transport must not weaken the existing NUL-terminated
  selection protocol.
- The cache is an optimization. Feature failures must fall back to scanning
  and must not change the directory-navigation contract.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `git diff --check`
- Verify fuzzy score ordering, stable ties, case handling, empty results, and
  incremental scan updates without spawning an external process.
- Exercise the three shell wrappers with successful selection, cancellation,
  missing executable, and paths containing whitespace/newlines where supported
  by the current UTF-8 contract.
- Verify release workflow packaging for Linux x86_64/aarch64, macOS arm64, and
  Windows x86_64, including the absence of a mandatory external fuzzy finder.

## Completion Evidence

- Built-in simple filtering was implemented and committed in `0ed7208`.
- Fuzzy matching, bounded prefetch, and the remaining Phase 3 gate evidence are
  pending.
