---
id: DECISION-0011
status: accepted
date: 2026-09-18
supersedes: none
review: none
---

# Decision: Filesystem Type Fallback and Cache Completeness

## Context

The foreground scanner currently uses `DirEntry::file_type()` to decide whether
an entry is a directory. On Unix, this value can come directly from the
directory enumeration record and can be unavailable on network, FUSE, virtual,
or otherwise limited filesystems. An unavailable type must not be treated as a
regular file or as proof that the entry is not a directory.

The directory-only cache also cannot distinguish a genuinely empty result from
a result made empty because entries or their types were skipped after an
enumeration or metadata error. The current `r` action follows the normal cache
lookup path, so it cannot reliably recover from such a cached result.

## Options

1. Keep the cheap directory-entry type check and continue caching every
   completed-looking result. This preserves the current cost but can hide
   directories and persist incomplete listings.
2. Stat every direct child before classifying it. This gives stronger type
   correctness but adds unnecessary filesystem work, especially on remote
   mounts and large directories.
3. Keep the cheap type fast path, use metadata only for unknown or failed type
   queries, explicitly mark scan completeness, and make `r` bypass the cache.
   Cache a zero-directory result only when the complete enumeration and all
   classifications succeeded.

## Decision

Use option 3.

Resolve a direct entry's directory status in this order:

1. Trust a definitive directory, regular-file, symlink, or other known type
   from `file_type()`.
2. For an unknown or failed type query, use a metadata fallback that preserves
   the existing behavior of recognizing symlinks whose targets are directories.
3. If the fallback also fails, retain the entry only where the active view
   permits an uncertain non-directory row, mark the scan incomplete, and never
   use that result to update the directory-only cache.

The scanner will publish whether a completed scan is cacheable. A directory-only
scan with no child directories remains cacheable when every direct entry was
enumerated and classified successfully; this covers both a truly empty directory
and a directory containing only files. Any skipped entry, enumeration error, or
unresolved type makes the result non-cacheable while leaving the usable partial
listing available to the UI.

`r` will start a fresh scan without attempting a cache read. A successful,
complete directory-only rescan may replace the old cache after the existing
start/end fingerprint check. File-visible scans continue to bypass cache reads
and writes.

Increment the cache format version for `0.0.9`. Existing records cannot encode
whether an empty result came from a complete scan, so they must be treated as
obsolete and rebuilt once rather than trusted indefinitely.

## Rationale

The hybrid type strategy preserves the fast path for ordinary local filesystems
while making the correctness fallback available to filesystems that omit
directory-entry type data. A zero-result listing is not inherently suspicious;
the scan's completeness, rather than its entry count, determines whether it is
safe to cache. An explicit rescan gives users a predictable escape from a stale
or remote-filesystem cache without weakening normal cache performance.

The one-time cache version invalidation is necessary because old empty records
have no provenance or completeness bit. It is safer than silently interpreting
those records under the new contract and remains reversible because the cache is
only an acceleration layer.

## Consequence

- Unknown or failed type queries can incur one metadata request per affected
  entry, which may be noticeable on a remote filesystem.
- A complete scan with zero child directories remains cacheable and does not
  cause an endless rescan loop.
- Incomplete scans remain usable for the current session but do not become
  authoritative cache data.
- `r` performs a real fresh scan and can rewrite a valid cache after it finishes.
- Version `0.0.9` causes a one-time rebuild of existing directory cache records.
- Timestamps, file-visible cache isolation, shallow scanning, and the shell
  selection protocol remain unchanged.

## Affected Record Or Consumer

- `.project/plan/PLAN-0010-filesystem-scan-and-cache-resilience-0.0.9.md`
- `.project/issue/ISSUE-0001-filesystem-scan-and-cache-resilience.md`
- `.project/decision/DECISION-0002-cache-storage.md`
- `src/scan.rs`
- `src/app.rs`
- `src/cache.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
