---
id: DECISION-0015
status: accepted
date: 2026-11-15
supersedes: none
review: .project/review/REVIEW-0016-tui-flicker-and-footer-hints.md
---

# Decision: Changed-Line TUI Rendering

## Context

`DECISION-0013` governs when the application requests a redraw. Its event-driven
scheduling removed repeated idle frames, but actual redraws still cleared the
whole screen. The human reported that the footer continued to flash during
selection movement in the Windows terminal-multiplexer setup.

Synchronized output and then whole-frame buffering reduced the symptom without
eliminating it. The supplied PTY captures showed drawing content outside the
intended synchronization boundaries, but did not identify the responsible
Windows component. `ISSUE-0002` owns the capture evidence and experiment history.

The human authorized changed-line rendering, subsequently confirmed that flashing
was gone, and accepted the working-tree result. The human then supplied and
accepted implementation tree `b8bd438fe228234574f183daf343ad4816bf2561`;
commit `607f0dc9e6af9f9a864dce0e48a663c199502c66` has that exact tree.
The human supplied the commit and authorized governance finalization.
`REVIEW-0016` records the approval and verified commit correspondence.

## Option

1. Keep full-screen clearing with buffered synchronized frames. This retains
   the simpler renderer, but the human's tests showed that these measures alone
   did not eliminate the footer flashing.
2. Compare final visible rows and emit only changed rows, retaining buffering
   and synchronization markers. This adds a visible-frame cache and comparison
   work, but avoids erasing unchanged content.
3. Implement cell-level differences or investigate changes in terminal output
   components. These could address finer-grained effects, but exceed the
   demonstrated need after the human confirmed the changed-line result.

## Decision

Use option 2:

- Keep a renderer local to the application event loop, caching the previous
  visible frame and terminal dimensions.
- Compare clipped text, foreground color, and reverse-selection state for each
  visible row. With unchanged dimensions, emit only rows that differ; identical
  frames do not write or flush.
- Redraw all visible rows on the first frame or a dimension change. Represent
  removed entries as blank rows so stale content is cleared.
- Retain row-local clearing, but do not issue application-level full-screen
  clears. Reserve the bottom-right cell to avoid autowrap scrolling.
- Assemble changed-row commands and DEC mode 2026 markers in memory, submit the
  buffer with one application-level `write_all`, and flush. Advance the cached
  frame only after successful drawing and output.

This extends rather than supersedes `DECISION-0013`: its scheduling decision
remains in force. This decision replaces only that record's
whole-screen-clearing and outstanding-rendering-verification consequences.
Those statements describe the earlier implementation, not the new renderer.

## Rationale

Avoiding the erase-and-rewrite cycle removes the opportunity for unchanged
footer content to disappear between output batches. It does not require the
Windows terminal chain to present synchronization markers atomically.

The human's visual confirmation supports the result in the reported setup.
Automated tests separately cover row-selection decisions and buffered output;
neither establishes the behavior of every terminal or identifies a root cause
inside ConPTY or Zellij.

## Consequence

- Requested redraws still construct all visible rows before comparison. The
  renderer adds memory for a visible-frame cache and CPU work for comparisons,
  while reducing terminal output. No overall performance improvement is claimed
  without a benchmark.
- Differences are row-granular, not cell-granular; updates within a changed row
  can still tear. Buffering does not guarantee one OS write or repair terminal
  synchronization behavior.
- Event scheduling, key bindings, CLI output, dependencies, persistent cache
  format, and package version `0.0.11` remain unchanged. Footer wording and
  human-edited colors belong to the implementation review, not a new binding
  contract.
- Deferred work: cell-level rendering and terminal-component investigation.
  Ceiling: the current implementation only avoids unnecessary row output.
  Trigger: reproducible remaining or recurring flicker, especially when a
  capture confirms no unchanged-footer output or full-screen clearing.

## Affected Record Or Consumer

- `.project/decision/DECISION-0013-event-driven-tui-redraw.md`
- `.project/issue/ISSUE-0002-tui-flicker-and-idle-redraw.md`
- `.project/review/REVIEW-0016-tui-flicker-and-footer-hints.md`
- `src/app.rs`
- `src/app/render.rs`
- `test/app.rs`
- `test/render.rs`
