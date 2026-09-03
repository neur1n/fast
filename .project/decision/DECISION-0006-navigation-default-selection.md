---
id: DECISION-0006
status: accepted
date: 2026-09-03
supersedes: DECISION-0004
review: .project/review/REVIEW-0006-navigation-default-selection-and-version-0.0.4.md
---

# Decision: Navigation Default Selection

## Context

The accepted navigator selection behavior restores paths remembered during the
current process, but a directory without history currently starts on the first
navigation entry. That makes the first useful child directory require an extra
downward movement. Scanning is incremental, so the default must also behave
consistently while entries arrive from cache or foreground scan chunks.

## Decision

Keep the fuzzy-filter default, process-local path-based selection map, parent
return restoration, cache contract, and shell selection protocol from
`DECISION-0004`.

When the current directory has no remembered selection, use a pending
`FirstChild` target. Select the first visible entry after the navigation entries
(`..` and `.` where present) as soon as it is available. The ordering is the
same ordering shown in the current list; no separate sort or cache data is
introduced.

If no child directory exists by the end of the scan, select the last available
navigation entry, which is `.` for both a normal directory and the filesystem
root. If a remembered path exists, it remains higher priority than the
first-child default. A missing remembered path uses the first-child fallback
and then the navigation fallback.

An explicit user selection movement (`Up`, `Down`, `Home`, or `End`) cancels
the pending automatic target. Later asynchronous scan chunks must preserve that
explicit selection instead of moving the cursor automatically.

Selection state remains process-local and is not added to persistent cache
records.

## Rationale

The first actual child is the most useful default for a new directory, while
remembered paths preserve continuity for directories the user has already
visited. Separating `FirstChild` from a remembered path prevents the initial
navigation placeholder from incorrectly winning after scan results arrive.

Cancelling a pending target on explicit movement gives user input priority over
filesystem timing. The navigation fallback keeps empty directories and the
initial navigation-only screen valid without inventing a child selection.

## Consequence

- A new non-root directory selects its first child after `..` and `.`.
- A new root directory selects its first child after `.`.
- Empty directories remain safely selectable through `.` or `..` navigation.
- Cache hits and cold scan chunks converge on the same selection policy.
- Existing remembered-path and parent-return behavior remains path-based and
  process-local.
- The cache format and shell selection protocol remain unchanged.

## Affected Record Or Consumer

- `.project/plan/PLAN-0006-navigation-default-selection-and-version-0.0.4.md`
- `.project/STATE.md`
- `.project/roadmap/ROADMAP-0001.md`
- `src/app.rs`
- `README.md`
