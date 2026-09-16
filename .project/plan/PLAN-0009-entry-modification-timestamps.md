---
id: PLAN-0009
status: completed
roadmap: ROADMAP-0001
phase: phase-5-entry-modification-timestamps
issue: []
review: .project/review/REVIEW-0012-entry-modification-timestamps.md
---

# Plan: Entry Modification Timestamps

## Objective

Display the last-modified timestamp for every displayed direct entry in both
directory-only and file-visible modes while preserving the navigator's shallow,
responsive, browse-only behavior. Cache hits must refresh entry metadata without
treating a cached timestamp as authoritative or blocking the TUI on a large
directory. Release the feature as package version `0.0.8`.

## Scope

- Included: capturing a direct entry's modification time during foreground
  scans, with a safe unavailable state when metadata cannot be read.
- Included: a cancellable metadata-only worker that refreshes every cached
  entry in bounded chunks after a directory-only cache hit.
- Included: applying metadata updates by path and active refresh context so
  late results cannot update a newer directory view.
- Included: keeping the persistent cache as a directory-listing cache; cached
  records remain readable without storing or trusting entry timestamps.
- Included: displaying local timestamps in `YYYY-MM-DD HH:mm` format with
  directory rows and timestamps in `White` and file rows and timestamps in
  `DarkGrey`.
- Included: rendering `...` while a timestamp refresh is pending and `-` when
  metadata is unavailable.
- Included: an 80-display-cell upper bound for the complete entry line, with a
  derived display-width-aware name column followed by four ASCII spaces and the
  timestamp, without anchoring the timestamp to the terminal's right edge.
- Included: render-time name truncation with an ASCII `...` suffix, terminal
  display-width-aware measurement, and recalculation after terminal resizing.
- Included: preserving navigation, filtering, selection, file grouping, cache
  isolation, and browse-only behavior.
- Included: regression coverage, user documentation, and synchronized package
  version metadata at `0.0.8`.
- Excluded: sorting or filtering by modification time, relative timestamps,
  seconds, recursive or descendant timestamps, previews, file launching, a
  timestamp configuration flag, and bounded child-directory prefetch.

## Acceptance Criteria

- Directory-only and file-visible listings show a timestamp for every entry
  whose metadata is available, including the synthetic `.` and `..` entries
  when their paths can be read.
- A cold foreground scan obtains entry metadata in the existing background
  chunk stream and does not wait for a complete directory scan before showing
  the first entries.
- A valid directory cache displays its listing immediately, then refreshes
  every displayed entry's timestamp asynchronously in chunks before reporting
  the refresh as complete.
- A cache hit does not use an old cached timestamp as the final value; each
  entry is queried once for the active listing refresh, but ordinary redraws,
  filtering, and selection movement do not issue metadata queries.
- Navigation, file-mode toggling, rescanning, and exit cancel obsolete metadata
  work, and results from an obsolete context cannot change the active view.
- A metadata failure leaves the entry visible and renders `-`; a pending
  refresh renders `...`.
- Names and timestamps remain separated by four ASCII spaces and timestamps
  share one vertical column for the current visible listing. The complete entry
  line is capped at 80 display cells before terminal-width constraints are
  applied. The timestamp remains visible whenever the terminal can accommodate
  the timestamp and a usable name; otherwise the timestamp column is hidden
  before the name becomes unusable.
- A name that exceeds the current render width is truncated only for display
  and receives an ASCII `...` suffix. Enlarging the terminal reveals more of
  the original name, and shrinking it never compounds a previous truncation.
- Directory and file colors remain consistent for both the name and timestamp,
  and the existing reverse-highlight behavior remains intact.
- The existing directory-only cache record format remains readable and file-
  visible scans still bypass cache reads and writes.
- The root manifest, lockfile, and `--version` output identify the package as
  `0.0.8`.

## Steps

1. Review and accept the timestamp freshness, cache, metadata failure, color,
   spacing, placeholder, truncation, and resize contract in
   `DECISION-0010` before implementation.
2. Extend the in-memory scan entry interface with optional modification
   metadata. Populate it during direct scans, retain the existing entry type
   and symlink classification, and define the cross-platform local-time
   formatting and terminal-cell-width helpers.
3. Add a cancellable metadata-only refresh stream for cached listings. Keep the
   listing visible while updates arrive in chunks, preserve pending/error
   states, and isolate updates by active context and entry path.
4. Integrate cache-hit refresh with application lifecycle events, including
   navigation, file-mode replacement, rescan, exit, selection restoration,
   filtering, and status text.
5. Replace the single-column renderer with an 80-display-cell bounded,
   width-aware name and timestamp layout. Maintain the visible-name maximum
   incrementally for unfiltered scan chunks, recompute it for filter changes,
   derive the name column after reserving the marker, timestamp, and four ASCII
   spaces, append `...` only when the original name is truncated, and recompute
   terminal constraints on resize.
6. Preserve the existing listing-only cache encoding and add tests for scan
   metadata, cache-hit refresh, metadata errors, cancellation, stale events,
   cache compatibility, colors, spacing, long names, Unicode display widths,
   narrow terminals, and resize expansion/contraction.
7. Update the README, synchronize package metadata to `0.0.8`, run the
   implementation checks, inspect the candidate tree, and prepare the exact-
   tree implementation review.

## Affected File Or Interface

- `src/app.rs`
- `src/scan.rs`
- `src/cache.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
- `.project/decision/DECISION-0010-entry-modification-timestamps.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/project.json`
- `.project/STATE.md`

## Risk And Reversibility

- Querying metadata for every entry on a cache hit adds filesystem work. A
  bounded worker, chunked publication, and cancellation preserve interaction
  responsiveness; the query happens on listing refresh, not every redraw.
- Cached directory membership and child metadata have different freshness
  properties. Keeping timestamps out of the authoritative cache result avoids
  stale values, while a cache hit still avoids the full directory enumeration.
- Late metadata results can be associated with a replaced directory or entry.
  Active refresh ownership, cancellation, and path-based application prevent
  stale publication.
- Modification metadata can be unavailable, predate the Unix epoch, or have
  platform-specific symlink behavior. Such cases retain the entry and render
  the documented fallback rather than failing the scan.
- Four spaces and a fixed timestamp reservation reduce the usable width of a
  narrow terminal. Width-aware truncation and hiding the timestamp at the
  minimum threshold keep navigation usable.
- Local-time formatting and terminal-cell-width support may require small
  direct dependencies. Any dependency addition must be locked and justified;
  removing the feature remains possible without a persistent cache migration.
- The package version bump is limited to the root manifest and lockfile; the
  cache format and shell selection protocol remain unchanged.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- Verify cold directory-only and file-visible scans publish metadata with the
  first available chunks.
- Verify a cache hit shows the listing promptly, refreshes all entry paths in
  chunks, handles metadata errors, and reaches the ready state.
- Verify navigation, file-mode toggling, rescan, and exit cancel refresh work
  and discard late results.
- Verify the old listing-only cache format remains readable and is not written
  by file-visible scans.
- Verify the four-space layout, `...` pending state, `-` error state, matching
  colors, and render-time truncation at narrow and expanded terminal widths.
- Verify Unicode and full-width names do not shift or overwrite the timestamp.
- Verify the manifest, lockfile, and `cargo run --quiet -- --version` report
  `fast 0.0.8`.

## Completion Evidence

- Commit `715028818636c6bd26e3fd1a3cb7680e7cc83492` implements direct-entry
  timestamps, asynchronous cache-hit metadata refresh, and width-aware aligned
  rendering at package version `0.0.8`.
- The implementation preserves the listing-only cache format and passes the
  recorded formatting, lint, test, check, version, and diff validations.
- Human binary testing and the accepted implementation disposition are recorded
  in `REVIEW-0012`.
