---
id: REVIEW-0009
status: approved
type: implementation
target: PLAN-0008
base_commit: 518b86431a01e8e4806aa0daf12dd67a68f38273
candidate_tree: cec199816f4e7c54631aaec45a156e35f57fd467
scope:
  - Validated logical `PWD` recovery at application startup
  - Logical parent preservation for directories reached through symlinks
  - Safe fallback for invalid, relative, or mismatched `PWD` values
  - Symlink and fallback regression coverage
  - Package version `0.0.6` and synchronized Cargo lock metadata
staged_paths:
  - Cargo.lock
  - Cargo.toml
  - src/main.rs
reviewer: human reviewer
date: 2026-09-08
provenance: Human testing confirmed the symlink navigation behavior and the human reviewer supplied the committed implementation hash.
verdict: approve
transition: Complete PLAN-0008 and advance project navigation to the deferred Phase 4 prefetch plan.
candidate_commit: a824db7760426ebb908f4ef439ff0de0ed18ab33
---

# Review: Logical Symlink Paths and 0.0.6

## Evidence

- Commit `a824db7760426ebb908f4ef439ff0de0ed18ab33` has parent
  `518b86431a01e8e4806aa0daf12dd67a68f38273` and tree
  `cec199816f4e7c54631aaec45a156e35f57fd467`.
- `src/main.rs` compares a canonical physical current directory with a
  canonical absolute `PWD` before preserving the logical path.
- A real Unix symlink regression test verifies that a logical path such as
  `B/foo2/bar1` is retained when it targets `A/foo1/bar1`.
- A fallback regression test verifies that mismatched and relative logical
  paths are ignored.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 59 tests.
- `cargo check --locked` passed.
- `git diff --check` passed for the implementation commit.
- `cargo run --quiet -- --version` reported `fast 0.0.6`.
- The root package version is `0.0.6` in both `Cargo.toml` and `Cargo.lock`.
- The cache format, shell wrappers, and normal navigation path handling remain
  unchanged.

## Human Finding

- The human reviewer confirmed that starting `fast` from the symlinked path
  retains that logical path and returns to its logical parent.
- The human reviewer accepts the validated `PWD` fallback behavior and the
  synchronized `0.0.6` package metadata.
- Physical-path shell mode remains outside the application's responsibility.

## Condition

- blocking: none.
- non-blocking: a shell that has already enabled physical-path mode cannot
  recover a logical alias that is absent from `PWD`.

## Agent Assessment

- The fix is limited to startup path selection and does not canonicalize paths
  during directory entry or parent navigation.
- Canonical comparison prevents a stale or unrelated `PWD` from changing the
  actual working directory used by the application.
- The implementation satisfies the logical symlink path, fallback, version,
  and regression coverage requirements.

## Human Decision

- Approve commit `a824db7760426ebb908f4ef439ff0de0ed18ab33` and candidate tree
  `cec199816f4e7c54631aaec45a156e35f57fd467` for `PLAN-0008`.
- Complete the governance reconciliation and advance project navigation to the
  deferred Phase 4 prefetch plan.
