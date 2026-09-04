---
id: PLAN-0007
status: implemented
roadmap: ROADMAP-0001
phase: phase-3-file-visibility
issue: []
review: .project/review/REVIEW-0008-file-visibility-version-0.0.5.md
---

# Plan: File Visibility Toggle

## Objective

Allow the TUI to display all direct non-directory entries on demand while
keeping navigation directory-only, browse-only, shallow, chunked, and
compatible with the existing shell selection protocol and cache format. Release
the feature as package version `0.0.5`.

## Scope

- Included: a runtime `F` toggle with directory-only startup behavior.
- Included: collecting files and directories together in the existing chunked
  foreground scan.
- Included: directory type metadata for safe selection and navigation behavior.
- Included: no-op `Enter`, `Right`, and `l` behavior for non-directory entries.
- Included: treating `q` on a non-directory entry as selection of `.`.
- Included: directory-first default selection and current-directory fallback.
- Included: directory-first display ordering, a non-selectable `-- Files --`
  section label, and dimmed file rows.
- Included: bypassing persistent cache reads and writes while files are visible.
- Included: filtering and selection behavior for mixed file/directory listings.
- Included: documentation and regression coverage for the new interaction.
- Included: synchronized package version metadata at `0.0.5`.
- Excluded: a CLI flag, MIME detection, file launching, previews, recursive
  indexing, content search, mouse input, and cache format migration.

## Acceptance Criteria

- A new application starts with only direct child directories displayed.
- Pressing `F` cancels the current scan and displays direct directories and
  non-directory entries as one chunked listing.
- Pressing `F` again returns to directory-only mode and can use the existing
  directory cache.
- In file-visible mode, child directories are sorted before files, and the
  `-- Files --` label separates the two sections without becoming selectable.
- File rows remain movable, highlightable, and filterable; only the section
  label is non-selectable.
- A file or other non-directory entry is never opened by `Enter`, `Right`, or
  `l`.
- Pressing `q` on a non-directory entry returns the current directory path.
- `h`, `Backspace`, and `Left` retain parent navigation behavior.
- Mixed listings choose the first child directory by default; listings without
  child directories choose `.`.
- Existing cache records remain directory-only and are not changed by a
  file-visible scan.
- File-visible scans remain direct-child, cancellable, and chunked.
- The root package manifest and lockfile identify the package as `0.0.5`.

## Steps

1. Record the file visibility and browse-only interaction contract.
2. Add directory/non-directory metadata to scan entries and make the scanner's
   inclusion policy explicit.
3. Add the runtime view state and `F` scan replacement to the application.
4. Guard directory opening, map non-directory confirmation to `.`, and update
   default selection, status, footer, filtering, cache boundaries, ordering,
   and rendering rows.
5. Add regression coverage for scan inclusion, chunking, toggling, grouping,
   section placement, colors, selection, cancellation boundaries, and cache
   isolation.
6. Update the navigation documentation and run the implementation checks.
7. Synchronize the package metadata to `0.0.5` and prepare the follow-up
   exact-tree review.

## Affected File Or Interface

- `src/app.rs`
- `src/scan.rs`
- `src/cache.rs`
- `src/filter.rs`
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
- `.project/decision/DECISION-0007-file-visibility-and-browse-only.md`

## Risk And Reversibility

- Replacing a scan while chunks are pending must not apply old events to the
  new view; the existing per-scan channel ownership and `stop_scan` boundary
  provide the isolation.
- A non-directory selected from a cached or changing filesystem must never be
  treated as navigable; explicit entry metadata and guarded open behavior keep
  the action local.
- File-visible listings intentionally pay for a fresh scan and do not benefit
  from the current cache until a mode-aware cache is separately designed.
- The change is reversible without a cache migration because directory-only
  cache records and the selection protocol remain unchanged.
- The version bump is limited to the root manifest and lockfile; a mismatch
  would make locked verification and release metadata inconsistent.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- Verify file and directory entries arrive together in chunks.
- Verify `F` scan replacement and no stale scan event application.
- Verify non-directory `Enter`/`Right`/`l` no-op and `q` current-directory
  fallback.
- Verify directory-only cache records are unchanged by file-visible scans.
- Verify the manifest, lockfile, and `--version` output report `0.0.5`.

## Completion Evidence

- Implemented runtime `F` toggling, browse-only non-directory behavior,
  directory-first file grouping, the non-selectable Files label, dimmed file
  rows, and directory metadata in `src/app.rs`, `src/scan.rs`, and
  `src/filter.rs`.
- Preserved the existing directory-only cache format and bypassed cache reads
  and writes while files are visible.
- Added scanner, chunking, grouping, rendering-row, selection, shortcut, and
  cache-isolation regression coverage; the test suite contains 57 tests.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --
  -D warnings`, `cargo test --locked`, `cargo check --locked`, and
  `git diff --check` pass.
- Human exact-tree review accepted the implementation commit
  `2d545386f4b11eabecc66c5bde1e35fc51b35e05` and candidate tree
  `9b264a4922ec04eef6d461718ecfbfab84efeb1b` in `REVIEW-0007`.
- The root package metadata is synchronized to `0.0.5`; `cargo test --locked`,
  `cargo check --locked`, and `cargo run --quiet -- --version` report the
  expected version. The metadata and governance follow-up remains pending in
  `REVIEW-0008`.
