---
id: REVIEW-0005
status: pending
type: implementation
target: DECISION-0005
base_commit: 521e6b6
candidate_tree: pending
scope:
  - `--version` and `-V` CLI output
  - Compile-time release version injection with Cargo fallback
  - Direct argument forwarding in the Bash, Zsh, and Nushell wrappers
  - No-`v` release tag validation and stable/prerelease GitHub release handling
  - Version documentation and the version strategy decision record
staged_paths: []
reviewer: human reviewer
date: 2026-09-03
provenance: pending human review of the implementation candidate.
verdict: pending
transition: none
candidate_commit: pending
---

# Review: Version Reporting and Release Injection

## Evidence

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 40 tests.
- `git diff --check` passed.
- `bash -n shell/fast.bash`, `zsh -n shell/fast.zsh`, and Nushell source parsing
  passed.
- The workflow YAML parsed successfully with Ruby's YAML parser.
- A default build reported `fast 0.1.0`.
- Compile-time injection reported `fast 0.1.0-rc.1` and
  `fast 0.1.0-nightly.20260903.123`; removing the variable restored
  `fast 0.1.0`.
- Bash, Zsh, and Nushell wrapper invocations of `--version` and `-V` reported
  the injected version.

## Human Finding

- None recorded; human review is pending.

## Condition

- blocking: the candidate must be staged and its exact tree reviewed before
  approval under the phased-development workflow.
- non-blocking: no actionlint installation was available, so workflow
  validation covered YAML parsing and the changed shell fragments only.

## Agent Assessment

- The version is embedded at compile time, so runtime environment changes do
  not alter an installed binary's reported version.
- The manual workflow injects its validated no-`v` input into every build
  matrix job and adds the GitHub prerelease flag only when the version contains
  a prerelease suffix.
- No-argument shell navigation remains on the existing selection-file path.

## Human Decision

- Pending human review and exact-tree approval.
