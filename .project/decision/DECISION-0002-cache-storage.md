---
id: DECISION-0002
status: accepted
date: 2026-08-31
supersedes: none
review: .project/review/REVIEW-0001-phase-2-cache.md
---

# Decision: Cache Storage Format

## Context

Phase 2 needs persistent direct-directory results with corruption detection,
crash-safe replacement, bounded storage, and no required system service or
external CLI. The cache is an acceleration layer rather than user data.

## Option

1. Use a custom versioned binary record per directory, keeping the format and
   storage limits local to `fast`.
2. Use SQLite through a bundled Rust binding, gaining query and eviction
   support at the cost of a larger dependency and migration surface.
3. Require an external database or fuzzy-finder executable, reducing code in
   `fast` but violating the standalone distribution goal.

## Decision

Use a custom versioned binary record per directory. Identify records with a
stable hash of the directory path, validate the directory metadata fingerprint
before loading, and protect records with a checksum. Write complete temporary
files and replace the record path; on Windows, fall back to remove-then-rename
when the platform refuses replacement of an existing file.

## Rationale

The project only needs one directory's direct children at a time, so it does
not need database queries. A small self-contained format preserves the current
dependency footprint and makes corruption, size, and compatibility behavior
explicit. Cache misses and cache errors remain safe because the existing
scanner is authoritative.

## Consequence

- Cache records are limited to 4 MiB and 100,000 entries; the cache directory
  is limited to 256 records and 16 MiB.
- A scan is cached only when its start and end directory fingerprints match.
- The fingerprint uses directory modification time and metadata length; a
  future format version may add stronger platform-specific identity data.
- The cache format is accepted for Phase 2; an incompatible future format
  requires a new version and reviewed decision.

## Affected Record Or Consumer

- `.project/roadmap/ROADMAP-0001.md`
- `.project/plan/PLAN-0002-phase-2-persistent-directory-cache.md`
- `src/cache.rs`
- `src/main.rs`
