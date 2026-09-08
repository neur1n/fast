---
id: PLAN-0008
status: completed
roadmap: ROADMAP-0001
phase: phase-3-logical-path-preservation
issue: []
review: .project/review/REVIEW-0009-logical-path-preservation-and-version-0.0.6.md
---

# Plan: Logical Symlink Paths and 0.0.6

## Objective

Preserve the shell's logical working-directory path when `fast` starts inside a
directory reached through a symlink, so parent navigation remains within the
logical path. Release the fix as package version `0.0.6` without changing the
cache format or shell selection protocol.

## Scope

- Included: reading the physical current directory and the optional `PWD`
  logical path at startup.
- Included: validating `PWD` by requiring an absolute path whose canonical
  directory matches the physical current directory.
- Included: falling back to the physical current directory for invalid,
  relative, missing, or mismatched logical paths.
- Included: preserving the logical path for later lexical parent navigation.
- Included: regression coverage for a symlinked directory and invalid `PWD`
  values.
- Included: synchronized package version metadata at `0.0.6`.
- Excluded: changing shell physical-path mode, shell wrappers, cache identity,
  directory scanning, or path canonicalization during normal navigation.

## Acceptance Criteria

- Starting from a valid logical path such as `B/foo2/bar1`, where `bar1` points
  to `A/foo1/bar1`, retains `B/foo2/bar1` as the application directory.
- Parent navigation from that logical path returns to `B/foo2`, not `A/foo1`.
- A valid absolute `PWD` is used only when it resolves to the physical current
  directory.
- Missing, relative, invalid, or mismatched `PWD` values fall back safely.
- The package manifest and lockfile identify the root package as `0.0.6`.
- Existing cache, scanning, shell wrapper, and selection behavior remain
  unchanged.

## Steps

1. Read the physical current directory and optional shell logical path at
   startup.
2. Validate the logical path against the canonical physical directory and
   select the safe fallback when validation fails.
3. Keep the chosen path lexical through the existing navigation code.
4. Add symlink and invalid-`PWD` regression coverage.
5. Synchronize package metadata and run the implementation checks.

## Affected File Or Interface

- `src/main.rs`
- `Cargo.toml`
- `Cargo.lock`
- `.project/decision/DECISION-0008-logical-path-preservation-through-symlinks.md`
- `.project/review/REVIEW-0009-logical-path-preservation-and-version-0.0.6.md`

## Risk And Reversibility

- An untrusted `PWD` must not replace the actual working directory; canonical
  comparison prevents that mismatch.
- Canonicalization can fail because a path is missing or inaccessible; the
  physical path remains a usable fallback in that case.
- A shell using physical-path mode cannot provide the discarded logical alias;
  the application cannot recover information that is absent from `PWD`.
- The change is reversible without a cache migration because only startup path
  selection and test coverage are changed.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- Verify `cargo run --quiet -- --version` reports `fast 0.0.6`.
- Verify a valid logical symlink path is preserved and its parent remains
  logical.
- Verify invalid and relative `PWD` values fall back to the physical path.

## Completion Evidence

- `src/main.rs` now prefers a validated absolute `PWD` and falls back to
  `env::current_dir()` when validation fails.
- Regression tests cover a real symlinked directory and invalid or relative
  logical paths.
- Human testing confirmed that starting from a symlinked directory preserves
  the logical path and returns to the logical parent.
- Commit `a824db7760426ebb908f4ef439ff0de0ed18ab33` contains the implementation,
  synchronized `0.0.6` metadata, and tree
  `cec199816f4e7c54631aaec45a156e35f57fd467`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --
  -D warnings`, `cargo test --locked`, `cargo check --locked`, and
  `git diff --check` passed; the test suite contains 59 tests.
- `cargo run --quiet -- --version` reported `fast 0.0.6`.
- The accepted implementation review is recorded in `REVIEW-0009`.
