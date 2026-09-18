---
id: ISSUE-0001
status: completed
plan: PLAN-0010
blocked_by: []
review: .project/review/REVIEW-0013-filesystem-scan-and-cache-resilience.md
---

# Issue: Filesystem Scan and Cache Resilience

## Intended Result

`fast` correctly discovers direct directories when the filesystem omits type
information, gives `r` a predictable force-rescan meaning, and never persists a
partial or uncertain directory-only listing as a valid cache result. The work
is released as package version `0.0.9`.

## Scope

- Included: unknown/failed entry-type fallback, scan completeness propagation,
  forced cache bypass for `r`, cache-write gating, cache format invalidation,
  regression tests, documentation, and version synchronization.
- Excluded: recursive prefetch, a new cache backend, file-visible cache records,
  MIME/file launching behavior, and changes to timestamp semantics.

## Acceptance Criterion

- Directories remain visible and navigable when `file_type()` lacks a definitive
  type but metadata identifies a directory.
- A metadata or iterator failure can leave a usable current listing but cannot
  create or overwrite a directory-only cache record.
- A complete empty directory-only result remains cacheable, including a
  directory containing only regular files.
- `r` bypasses a matching cache record and performs a fresh scan.
- A forced scan replaces the cache only when it completes and the directory
  fingerprint remains unchanged.
- The cache format bump rejects legacy records and causes them to be rebuilt.
- `F`, timestamp refreshes, shell selection, and shallow scanning retain their
  existing behavior.
- `fast --version` reports `0.0.9` after implementation.

## Verification Evidence

- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed with 72 tests.
- `cargo check --locked` passed.
- `cargo run --quiet -- --version` reported `fast 0.0.9`.
- `git diff --check` passed.
- Commit `1f5634f8c5dc1dc5f6c090a088ebc67f3ce99cfa` has candidate tree
  `c9201f402ad55500bc7de8a810aaf4406a4e411b` and changes only the planned
  implementation paths.
- Human acceptance and the final implementation disposition are recorded in
  `REVIEW-0013`.

## Blocker

- Reason: none.
- Resolution: Human accepted the committed implementation and authorized
  governance closure without a separate tree-ID review round.
- Pending action: none.

## Deferred Work

- Filesystem-specific performance tuning beyond the unknown-type fallback,
  including readdir-plus support or remote attribute-cache tuning.
- Bounded child-directory prefetch remains governed separately by `PLAN-0004`.
