---
id: DECISION-0010
status: accepted
date: 2026-09-15
supersedes: none
review: none
---

# Decision: Entry Modification Timestamps

## Context

The navigator now displays direct directories and can optionally display files,
but rows do not expose when an entry was last modified. A timestamp is useful in
both listing modes, yet the existing directory cache only fingerprints the
current directory and stores its child listing. A cached child timestamp could
therefore become stale without invalidating the listing cache.

The TUI must also remain responsive for large directories. The display needs a
portable loading state, a compact layout, and behavior that remains correct when
the terminal is resized while scan or metadata work is still arriving.

## Option

1. Use timestamps stored with the cached listing and refresh them only during a
   full directory scan. This preserves cache-hit speed but presents stale
   metadata after a child changes.
2. Refresh every cached entry synchronously before completing a cache hit. This
   gives a fresh result but can block the TUI on a large directory.
3. Keep the persistent cache as a listing-only cache, refresh every cached entry
   in a cancellable metadata-only worker with bounded chunks, and obtain
   metadata in the existing background scan for cold listings. This gives
   prompt listing display and eventual fresh timestamps without making cached
   values authoritative.

## Decision

Use option 3.

Display timestamps for all direct entries in both directory-only and file-visible
views. Use local `YYYY-MM-DD HH:mm` timestamps. Direct scans attach metadata to
entries as they are emitted. A directory-only cache hit displays the cached
listing immediately and starts a cancellable, chunked metadata refresh for every
displayed path. Pending values render as `...`; unavailable values render as
`-`. Refresh results are applied only to the active listing and matching entry
path.

Keep timestamps out of the authoritative persistent cache result so the existing
listing-only cache format remains readable and does not claim freshness it cannot
guarantee. The cache continues to validate directory membership with its
existing fingerprint, while entry metadata is refreshed separately.

Render each row as the marker, the original name truncated only for the current
terminal width, four ASCII spaces, and the timestamp field. Do not pin the
timestamp to the terminal's right edge. Append ASCII `...` only to a name that
is truncated; recompute the rendering from the full name and current terminal
size on every draw. Use `White` for directory names and timestamps and
`DarkGrey` for file names and timestamps.

## Rationale

Option 3 separates directory-listing freshness from child-metadata freshness
without discarding the existing cache optimization. Chunked background work
matches the current scan model and allows navigation, rescan, and exit to cancel
obsolete work. ASCII placeholders and spacing avoid font and terminal-width
ambiguity. A direct separator instead of terminal-edge alignment keeps short
names and timestamps visually associated and avoids layout jitter as scan chunks
arrive.

## Consequence

- A cache hit performs one metadata query per displayed path for that listing
  refresh, but ordinary redraws, filtering, and selection movement do not.
- A cache hit can show `...` temporarily while its listing remains usable.
- Metadata failures do not remove entries or fail the directory scan.
- The persistent cache format and shell selection protocol remain unchanged.
- Directory timestamps describe the directory entry itself, not the newest
  modification anywhere below it.
- Long names may gain or lose their display-only `...` suffix as the terminal
  is resized; the stored path and full name remain unchanged.
- The feature increases filesystem metadata work and may add a small direct
  dependency for local-time formatting or terminal-cell-width measurement.

## Affected Record Or Consumer

- `.project/plan/PLAN-0009-entry-modification-timestamps.md`
- `src/app.rs`
- `src/scan.rs`
- `src/cache.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
