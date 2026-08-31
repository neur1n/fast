---
id: PLAN-0002
status: completed
roadmap: ROADMAP-0001
phase: phase-2-persistent-directory-cache
issue: []
review: .project/review/REVIEW-0001-phase-2-cache.md
---

# Plan: Persistent Directory Cache

## Objective

Add a small, versioned, crash-safe cache for direct child directories so a
previously visited directory can render without a complete directory read.

## Scope

- Included: a custom binary cache format, directory metadata fingerprints,
  cache invalidation, atomic writes, bounded cache storage, scan-race checks,
  and automated coverage.
- Excluded: recursive indexing, background child prefetch, fuzzy-finder
  backends, file previews, and a mandatory external database or CLI.

## Step

1. Define and document a versioned per-directory cache record with a checksum,
   bounded decoding, and UTF-8 path handling consistent with the initial scope.
2. Load a record after a metadata-only fingerprint check and treat missing,
   stale, or corrupt records as cache misses.
3. Store completed scans only when the directory fingerprint is unchanged from
   scan start, writing a complete temporary record before replacement.
4. Enforce maximum record size, cache file count, and total cache size without
   making cache failures prevent navigation.
5. Add tests for cache hits, invalidation, corruption, scan races, concurrent
   writers, atomic replacement, and storage bounds.

## Affected File Or Interface

- `src/cache.rs`
- `src/main.rs`
- `README.md`
- `.project/project.json`
- `.project/STATE.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/decision/DECISION-0002-cache-storage.md`

## Risk And Reversibility

- Directory modification timestamps can have filesystem-dependent granularity;
  a future stronger fingerprint can invalidate this format version.
- Cache data is an optimization. Read, decode, and write failures fall back to
  the existing scanner and do not change navigation semantics.
- The custom format avoids a new runtime dependency and can be replaced by a
  reviewed backend decision before the cache format becomes stable.

## Verification

- `cargo fmt --check`
- `cargo test`
- `git diff --check`
- Confirm the cache remains optional and no external executable is required.

## Completion Evidence

- Implemented in `src/cache.rs` and integrated into `src/main.rs`.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `cargo test` passed with 16 tests.
- Implementation was committed in `882c4ac` with candidate tree
  `bd4b10ed717f89585ba24b54daef9f50afab51b5`.
- Human acceptance and the Phase 2 gate transition are recorded in
  `.project/review/REVIEW-0001-phase-2-cache.md`.
