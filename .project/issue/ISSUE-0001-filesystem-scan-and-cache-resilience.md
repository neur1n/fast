---
id: ISSUE-0001
status: active
plan: PLAN-0010
blocked_by: []
review: none
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

- Pending implementation checks, manual filesystem tests, and candidate-tree
  review.

## Blocker

- Reason: none.
- Resolution: none.
- Pending action: implement the approved `PLAN-0010` scope.

## Deferred Work

- Filesystem-specific performance tuning beyond the unknown-type fallback,
  including readdir-plus support or remote attribute-cache tuning.
- Bounded child-directory prefetch remains governed separately by `PLAN-0004`.
