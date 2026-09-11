---
id: DECISION-0009
status: accepted
date: 2026-09-11
supersedes: DECISION-0006
review: .project/review/REVIEW-0011-current-directory-default-selection-and-version-0.0.7.md
---

# Decision: Current-Directory Default Selection

## Context

The accepted navigation behavior selected the first child directory when the
current directory had no remembered selection. That default is not useful when
the user wants to confirm the current directory, and it can also select a
different entry while an asynchronous scan is still discovering results.

For file-visible listings, this decision replaces only the default-selection
rule from `DECISION-0007`; its file visibility and browse-only behavior remain
unchanged.

Remembered selections can arrive in a later scan chunk, so the default must be
separate from asynchronous restoration.

## Option

1. Keep selecting the first child directory when no remembered selection exists.
2. Select `.` by default and restore a remembered entry only when it appears in
   the scan, with `.` as the fallback when that entry is unavailable.

## Decision

When the current directory has no remembered selection, select its `.`
current-directory entry as soon as the navigation entries are initialized. Do
not automatically move the selection to child directories as scan chunks or
cache entries arrive.

When a remembered selection exists, initially select `.` while waiting for the
remembered path. Select the remembered path as soon as it appears, unless the
user has explicitly moved the selection. If the scan finishes without the
remembered path, reset the selection to `.`.

The same behavior applies to directory-only scans, file-visible scans, cache
hits, and incremental foreground scans. Selection state remains process-local;
the cache format and shell selection protocol are unchanged.

## Rationale

The current-directory entry is a stable and intentional default that cannot be
mistaken for a newly discovered child. Keeping it selected during scanning
avoids filesystem timing affecting the UI, while a pending remembered path
preserves continuity for previously visited directories.

## Consequence

- A directory without a remembered selection starts and remains on `.`.
- A remembered path found in a later chunk takes over only before explicit user
  movement.
- A missing remembered path falls back to `.` rather than another child.
- The first-child pending target and first-child fallback logic are removed.
- Existing navigation, cache, filtering, and shell selection contracts remain
  unchanged.

## Affected Record Or Consumer

- `.project/decision/DECISION-0006-navigation-default-selection.md`
- `.project/decision/DECISION-0007-file-visibility-and-browse-only.md`
- `src/app.rs`
- `README.md`
