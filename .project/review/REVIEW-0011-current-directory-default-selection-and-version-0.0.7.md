---
id: REVIEW-0011
status: approved
type: implementation
target: DECISION-0009
base_commit: e6a0041fa1d2cdf12250786f126d40b0a2f849bc
candidate_tree: f38667916eb8f559ee220b0a9fc5b6f537f0e8ee
scope:
  - Current-directory default selection when no remembered path exists
  - Asynchronous remembered-path restoration and explicit movement priority
  - Current-directory fallback when a remembered path is unavailable
  - Navigation documentation and regression coverage
  - Package version `0.0.7` and synchronized Cargo lock metadata
staged_paths:
  - Cargo.lock
  - Cargo.toml
  - README.md
  - src/app.rs
reviewer: human reviewer
date: 2026-09-11
provenance: Human reviewer committed the implementation and requested governance reconciliation after reviewing the resulting behavior.
verdict: approve
transition: Accept the current-directory default selection and retain Phase 4 bounded prefetch as the deferred selected frontier.
candidate_commit: b6966577fba7ece307c1573fb2465debd537aea5
---

# Review: Current-Directory Default Selection and 0.0.7

## Evidence

- Commit `b6966577fba7ece307c1573fb2465debd537aea5` has parent
  `e6a0041fa1d2cdf12250786f126d40b0a2f849bc` and candidate tree
  `f38667916eb8f559ee220b0a9fc5b6f537f0e8ee`.
- `src/app.rs` selects `.` when no remembered selection exists, preserves that
  selection across asynchronous scan chunks, restores a remembered path when
  it appears, and falls back to `.` when it never appears.
- Explicit `Up`, `Down`, `Home`, and `End` movement still cancels pending
  remembered-path restoration.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 59 tests.
- `cargo check --locked` passed.
- `cargo run --quiet -- --version` reported `fast 0.0.7`.
- `git diff --check` passed for the implementation candidate.
- The cache format and shell selection protocol remain unchanged.

## Human Finding

- The human reviewer committed the implementation and requested that the
  accepted default-selection decision and project navigation records be
  reconciled with the committed behavior.

## Condition

- blocking: none.
- non-blocking: none.

## Agent Assessment

- The implementation removes the first-child pending target and fallback while
  retaining the pending remembered-path state required for chunked scans.
- A late remembered-path match can still take over before explicit user input;
  a missing match cannot select an unrelated child directory.
- The package metadata is synchronized at `0.0.7`, and the change does not
  alter persistent cache records or shell output.
- The committed implementation satisfies the recorded checks and its exact
  commit and tree are verified above.

## Human Decision

- Approve commit `b6966577fba7ece307c1573fb2465debd537aea5` and candidate tree
  `f38667916eb8f559ee220b0a9fc5b6f537f0e8ee`; accept `DECISION-0009` and retain
  `PLAN-0004` as the deferred selected frontier.
