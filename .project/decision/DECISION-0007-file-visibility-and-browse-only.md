---
id: DECISION-0007
status: accepted
date: 2026-09-04
supersedes: none
review: .project/review/REVIEW-0007-file-visibility-toggle.md
---

# Decision: File Visibility and Browse-Only Interaction

## Context

The navigator currently scans and displays directories only. Users also need
to inspect files in the current directory, but the navigator must remain a
directory browser rather than launching files through MIME associations or
external programs.

The existing foreground scan is shallow and chunked, and its persistent cache
stores directory-only results. A second view that includes files must not
reuse or overwrite those records as if they represented the same result.

## Decision

Keep directory-only display as the startup default. In normal navigation mode,
uppercase `F` toggles whether the current directory's non-directory entries
are displayed. Toggling replaces the active foreground scan with a new shallow
scan; it does not scan files by default and then hide them.

The scan continues to read only direct children and emits directories and
non-directory entries together in the existing chunks. Directory entries,
including directory symlinks recognized by the current scanner, remain the
only entries that can be opened with `Enter`, `Right`, or `l`. Those keys are a
no-op for files and other non-directory entries.

When files are visible, child directories are sorted first and non-directory
entries are sorted after them. A non-selectable `-- Files --` label marks the
start of the file section, and file rows use a dimmed color while remaining
movable and highlightable.

The `h` shortcut remains parent navigation. When a non-directory entry is
highlighted, `q` selects the current directory (`.`) and exits instead of
returning the non-directory path. The existing `--select` protocol and shell
wrappers remain unchanged.

File-visible scans bypass the persistent directory cache and are not written
to it. Directory-only scans retain the existing cache behavior and format.

## Rationale

Starting the file scan only when requested preserves the current startup and
cache behavior. Replacing the active scan is necessary because a directory-
only scan discards non-directory entries before they reach the application.

Using one worker and one chunk stream preserves the existing responsiveness and
avoids ordering differences between separate directory and file scans. Keeping
file-visible results out of the existing cache avoids mixing two incompatible
listing modes without a cache migration.

## Consequence

- `F` during a scan cancels the old scan and starts the selected listing mode.
- File and directory entries can appear in the same chunk; directories and
  files are sorted within their own sections.
- A mixed listing defaults to the first available directory; a listing with no
  child directory falls back to `.`.
- Filtering applies to files and directories equally while navigation entries
  remain available, with matching directories before matching files.
- The `-- Files --` label is not an entry and cannot be selected; file rows can
  be selected and highlighted.
- No MIME detection, file preview, process launch, recursive indexing, or new
  runtime dependency is introduced.

## Affected Record Or Consumer

- `.project/plan/PLAN-0007-file-visibility-toggle.md`
- `src/app.rs`
- `src/scan.rs`
- `src/cache.rs`
- `src/filter.rs`
- `README.md`
