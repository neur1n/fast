---
id: PLAN-0010
status: approved
roadmap: ROADMAP-0001
phase: phase-6-filesystem-scan-and-cache-resilience
issue: .project/issue/ISSUE-0001-filesystem-scan-and-cache-resilience.md
review: none
---

# Plan: Filesystem Scan and Cache Resilience for 0.0.9

## Objective

Make direct-directory listings reliable on filesystems that do not provide a
definitive directory-entry type, provide a genuinely cache-bypassing `r`
rescan, and prevent incomplete scans from becoming persistent cache authority.
Release these changes as package version `0.0.9` without changing the
navigator's shallow, responsive, browse-only contract.

## Scope

- Included: metadata fallback classification for unknown or failed
  `DirEntry::file_type()` results, including the existing directory-symlink
  behavior.
- Included: scan completion metadata that distinguishes a complete empty result
  from an incomplete result with skipped or unresolved entries.
- Included: preventing incomplete directory-only scans from being persisted or
  used as authoritative cache results.
- Included: making `r` skip the directory cache and replace a valid cache only
  after a complete scan and the existing fingerprint check.
- Included: incrementing the cache format version so records without the new
  completeness contract are rebuilt once.
- Included: regression coverage for unknown types, metadata fallback failures,
  valid empty results, incomplete results, forced rescans, cache replacement,
  and cache-version invalidation.
- Included: user-facing documentation for `r`, the cache rebuild behavior, and
  synchronized package metadata at `0.0.9`.
- Excluded: recursive indexing, bounded child-directory prefetch, file-visible
  cache records, MIME detection, file launching, and timestamp rendering or
  freshness semantics.

## Acceptance Criteria

- An entry whose directory type is unavailable from enumeration is classified
  through metadata when that metadata is available.
- A directory symlink retains the current target-directory navigation behavior.
- A type or metadata failure does not silently produce a cacheable empty or
  partial directory listing.
- A directory-only scan with no child directories is cached when enumeration and
  classification completed without errors, including a directory containing
  only files.
- Pressing `r` starts a fresh scan even when a matching cache record exists.
- A complete forced rescan can replace the previous cache after the fingerprint
  check; an incomplete rescan cannot.
- Existing cache records are invalidated by the new cache format version and do
  not bypass the new completeness rules.
- File-visible scans continue to bypass cache reads and writes, and timestamp
  refresh behavior remains unchanged.
- The root package manifest, lockfile, and `--version` output identify the
  package as `0.0.9`.

## Steps

1. Define a scanner classification helper with a cheap known-type path and a
   metadata fallback for unknown or failed type queries, preserving symlink
   directory semantics.
2. Extend scan completion events with cacheability/completeness information;
   mark iterator, type, and fallback metadata failures without aborting a usable
   partial listing.
3. Add an explicit cache policy to application scan startup so normal navigation
   may load the cache while `r` always starts a foreground scan.
4. Gate cache persistence on a complete directory-only result and increment the
   persistent cache format version to invalidate legacy records.
5. Add unit and application regression coverage for classification fallback,
   valid and invalid empty results, forced rescans, fingerprint races, cache
   invalidation, and timestamp-refresh isolation.
6. Update README behavior notes, synchronize package metadata and lockfile to
   `0.0.9`, run the implementation checks, and prepare the exact-tree review.

## Affected File Or Interface

- `src/scan.rs`
- `src/app.rs`
- `src/cache.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
- `.project/decision/DECISION-0011-filesystem-type-and-cache-completeness.md`
- `.project/issue/ISSUE-0001-filesystem-scan-and-cache-resilience.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/project.json`
- `.project/STATE.md`

## Risk And Reversibility

- Metadata fallback can add one remote filesystem request for each entry whose
  type is unavailable; limiting it to unknown or failed fast-path results avoids
  paying that cost for ordinary entries.
- A failed fallback may leave an entry out of the directory-only view for the
  current scan, but the incomplete result will not be persisted and a later
  scan can recover it.
- Forced rescans can be slower than cache hits by design; they provide an
  explicit recovery path without weakening normal startup latency.
- Incrementing the cache format causes one rebuild of existing records and does
  not affect user data, shell selection, or navigation paths.
- The directory fingerprint remains a filesystem-dependent freshness heuristic;
  `r` is the explicit escape when a remote filesystem does not update or expose
  that metadata reliably.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- Verify a known local directory still uses the fast type path without changing
  the visible listing.
- Verify an injected unknown type resolves through metadata and an injected
  metadata failure marks the scan non-cacheable.
- Verify truly empty and files-only directories produce valid cache entries,
  while iterator/type/fallback failures do not.
- Verify `r` ignores an existing cache, displays fresh entries, and replaces the
  cache only after a complete unchanged scan.
- Verify an old cache record is rejected after the format-version bump.
- Verify file-visible mode and asynchronous timestamp refresh remain unchanged.
- Verify `cargo run --quiet -- --version` reports `fast 0.0.9`.

## Completion Evidence

- Pending implementation and exact-tree review.
