---
id: REVIEW-0005
status: approved
type: implementation
target: DECISION-0005
base_commit: 521e6b6
candidate_tree: pending
scope:
  - `--version` and `-V` CLI output
  - Compile-time release version injection with Cargo fallback
  - Direct argument forwarding in the Bash, Zsh, and Nushell wrappers
  - No-`v` release tag validation and stable/prerelease GitHub release handling
  - Package version `0.0.3` and synchronized Cargo lock metadata
  - Version documentation and the version strategy decision record
staged_paths:
  - Cargo.toml
  - Cargo.lock
  - README.md
  - src/cli.rs
  - src/main.rs
  - shell/fast.bash
  - shell/fast.zsh
  - shell/fast.nu
  - .github/workflows/manual-release.yml
  - .project/decision/DECISION-0005-version-reporting-and-release-injection.md
  - .project/review/REVIEW-0005-version-reporting-and-release-injection.md
reviewer: human reviewer
date: 2026-09-03
provenance: Human reviewer confirmed the 0.0.3 package version and ran the offline lockfile synchronization.
verdict: approve
transition: Accept version reporting and release injection; keep automatic nightly scheduling deferred.
candidate_commit: pending
---

# Review: Version Reporting and Release Injection

## Evidence

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 40 tests.
- `git diff --check` passed.
- `cargo check --offline` synchronized the root package entry in `Cargo.lock`
  from `0.1.0` to `0.0.3`.
- `bash -n shell/fast.bash`, `zsh -n shell/fast.zsh`, and Nushell source parsing
  passed.
- The workflow YAML parsed successfully with Ruby's YAML parser.
- A default build reported `fast 0.0.3`.
- Compile-time injection reported `fast 0.0.3-rc.1` and
  `fast 0.0.3-nightly.20260903.123`; removing the variable restored
  `fast 0.0.3`.
- Bash, Zsh, and Nushell wrapper invocations of `--version` and `-V` reported
  the injected version.

## Human Finding

- The human reviewer confirmed `0.0.3` as the package version and accepted the
  version reporting and release injection behavior after running
  `cargo check --offline`.

## Condition

- blocking: the candidate tree and commit identifiers remain pending until the
  synchronized lockfile and governance update are staged and committed.
- non-blocking: no actionlint installation was available, so workflow
  validation covered YAML parsing and the changed shell fragments only.

## Agent Assessment

- The version is embedded at compile time, so runtime environment changes do
  not alter an installed binary's reported version.
- The manual workflow injects its validated no-`v` input into every build
  matrix job and adds the GitHub prerelease flag only when the version contains
  a prerelease suffix.
- The package manifest and lockfile both identify the root package as `0.0.3`,
  so the workflow's `--locked` checks are reproducible.
- No-argument shell navigation remains on the existing selection-file path.

## Human Decision

- Approve the version reporting and release injection candidate. Record the
  final candidate tree and commit identifiers after the candidate is committed.
