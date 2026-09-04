---
id: REVIEW-0007
status: approved
type: implementation
target: PLAN-0007
base_commit: 42ea3aea51264c158a23042ffbf1ba8ced9f7f54
candidate_tree: 9b264a4922ec04eef6d461718ecfbfab84efeb1b
scope:
  - Runtime uppercase `F` toggle for direct non-directory entries
  - Shared shallow chunked scanning for files and directories
  - Browse-only non-directory interaction and current-directory q fallback
  - Directory-first ordering, non-selectable Files label, and dimmed file rows
  - Directory-only cache isolation
  - README navigation documentation and regression coverage
staged_paths:
  - README.md
  - src/app.rs
  - src/cache.rs
  - src/filter.rs
  - src/scan.rs
reviewer: human reviewer
date: 2026-09-04
provenance: Human reviewer accepted the implementation tree and supplied the committed candidate hash.
verdict: approve
transition: Complete PLAN-0007 and advance project navigation to the deferred Phase 4 prefetch plan.
candidate_commit: 2d545386f4b11eabecc66c5bde1e35fc51b35e05
---

# Review: File Visibility Toggle

## Evidence

- Commit `2d54538` has parent `42ea3ae` and tree
  `9b264a4922ec04eef6d461718ecfbfab84efeb1b`.
- The candidate keeps directory-only startup behavior and adds runtime
  uppercase `F` switching without adding a CLI flag or changing shell wrappers.
- Files and directories use the same shallow, cancellable, chunked foreground
  scan; file-visible scans bypass the existing directory-only cache.
- Non-directory entries remain movable, highlightable, and filterable. `Enter`,
  `Right`, and `l` do nothing for them, while `q` returns the current directory.
- Matching directories remain before matching files. The `-- Files --` label is
  rendered outside the selectable entry index, and file rows use `DarkGrey`.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 57 tests.
- `cargo check --locked` passed.
- `git diff --check` passed for the candidate.

## Human Finding

- The human reviewer accepts the committed candidate tree and the file
  presentation behavior.
- The existing directory cache format, shell selection protocol, and deferred
  Phase 4 prefetch scope remain unchanged.

## Condition

- blocking: none for the implementation commit.
- non-blocking: this governance candidate remains to be staged and committed
  by the human reviewer.

## Agent Assessment

- The candidate satisfies the runtime file visibility, grouping, styling,
  browse-only interaction, cache isolation, and chunked-scan requirements.
- The supplied commit and tree identifiers establish the exact implementation
  candidate for this review.
- No code rollback or cache migration is required.

## Human Decision

- Approve commit `2d545386f4b11eabecc66c5bde1e35fc51b35e05` and candidate tree
  `9b264a4922ec04eef6d461718ecfbfab84efeb1b` for `PLAN-0007`.
- Complete the governance reconciliation and advance project navigation to
  the deferred Phase 4 prefetch plan.
