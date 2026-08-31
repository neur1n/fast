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
always-available built-in simple filter, an optional fuzzy backend selected
after evaluating the built-in and `fzf` options, and verified shell/release
integration. The standalone binary, current cache contract, and shallow scan
behavior remain the defaults.

## Scope

- Included: bounded, cancellable prefetch for direct child directories without
  turning navigation into an implicit recursive index.
- Included: an interactive, case-insensitive literal substring filter over
  directory names, with a clear input mode and safe behavior when no entries
  match.
- Included: a visible-entry mapping that keeps the complete scan result in
  memory and in the cache; filtering must not mutate the authoritative
  `entries` collection.
- Included: an evaluation gate for an optional fuzzy backend. After the simple
  filter is stable, select either a small in-process fuzzy implementation or an
  external `fzf` backend based on dependency, binary-size, platform, terminal,
  and fallback costs.
- Included: Bash, Zsh, and Nushell wrapper verification and end-to-end checks
  for the existing platform release workflow.
- Excluded: a mandatory `fzf` executable, a mandatory external database,
  recursive indexing, file browsing, previews, mouse input, and content search.
- Excluded: cache format changes unless a separately reviewed decision makes
  them necessary.

## Acceptance Criteria

- The simple filter works without any external executable or new mandatory
  runtime dependency.
- The `..` entry remains available for parent navigation, and confirming with
  no visible entry cannot accidentally select the current directory.
- Selection, scrolling, incremental scan chunks, cache hits, and directory
  changes preserve the selected path or apply a defined nearest-visible-entry
  fallback.
- Completed scans persist the complete unfiltered child-directory set, and
  cached data is unaffected by the active query.
- Prefetch has explicit queue/concurrency and work bounds, stops on navigation
  cancellation, and cannot make the UI wait for recursive work.
- An optional fuzzy backend, if selected, is unavailable-safe: the built-in
  simple filter remains usable when `fzf` is missing or the optional backend is
  disabled.
- Shell navigation and the supported release targets pass their verification
  checks without requiring an external database or fuzzy-finder executable.

## Step

1. Record the filter input semantics and prefetch limits before implementation;
   keep the simple filter as the mandatory baseline and the fuzzy backend as a
   later, reviewable choice.
2. Keep `entries` authoritative, add a small filter module and visible-index
   state, and update input handling, selection, scrolling, rendering, and
   incremental scan restoration. Add unit tests for matching, query editing,
   empty results, parent navigation, and selection stability.
3. Add bounded child-directory prefetch using cancellable work and the existing
   cache as an optimization. Test bounds, cancellation, cache interaction, and
   the absence of implicit recursive indexing.
4. Measure the simple-filter baseline and evaluate the in-process fuzzy and
   external `fzf` alternatives. Record the selected backend in a separate
   reviewed decision before adding backend-specific dependencies or process
   handling. If `fzf` is selected, invoke it outside raw mode and the alternate
   screen through direct process arguments, with cancellation and fallback
   behavior covered by tests.
5. Verify the Bash, Zsh, and Nushell selection protocol, platform release
   packaging, and documented installation paths. Keep release checks aligned
   with the existing manual workflow.
6. Run the phase checks, inspect the complete candidate tree, and prepare the
   implementation gate review.

## Affected File Or Interface

- `src/main.rs`
- `src/filter.rs` (new)
- `src/scan.rs` or a dedicated prefetch module
- `Cargo.toml` and `Cargo.lock` only if the selected fuzzy backend requires a
  dependency
- `shell/fast.bash`
- `shell/fast.zsh`
- `shell/fast.nu`
- `README.md`
- `.github/workflows/manual-release.yml`
- `.project/decision/` if a fuzzy backend is selected

## Risk And Reversibility

- A separate visible-index layer prevents filtering from corrupting cache
  persistence, but selection bugs can still cause an unintended directory to
  be selected; empty-result and path-restoration tests are required.
- Prefetch can consume threads, descriptors, and I/O unexpectedly; explicit
  bounds, cancellation, and no-recursion tests keep it reversible.
- Fuzzy ranking can reorder results and an external `fzf` process can disturb
  terminal state or be absent. The simple filter remains the fallback, and an
  external backend must be isolated from the core TUI lifecycle.
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
- Exercise the three shell wrappers with successful selection, cancellation,
  missing executable, and paths containing whitespace/newlines where supported
  by the current UTF-8 contract.
- Verify release workflow packaging for Linux x86_64/aarch64, macOS arm64, and
  Windows x86_64, including the absence of a mandatory external fuzzy finder.

## Completion Evidence

- To be filled after implementation, verification, and human gate review.
