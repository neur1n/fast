---
id: DECISION-0013
status: accepted
date: 2026-11-10
supersedes: none
review: .project/review/REVIEW-0015-tui-redraw-source-organization.md
---

# Decision: Event-Driven TUI Redraw

## Context

The earlier event loop redrew unconditionally at roughly 50 ms intervals,
including when the interface was idle. Windows flicker was reported in a
terminal-multiplexer configuration, but its cause has not been established.
The scan channel cannot wake a blocking terminal event read, so background
results still need to be checked while a scan is active.

## Options

1. Continue polling and redrawing periodically. This is simple but emits full
   frames even when nothing needs updating.
2. Poll periodically only while scanning, and redraw in response to relevant
   input, resize, or scan events. This avoids idle frames while retaining
   bounded checks for background results, but does not change how a frame is
   rendered.
3. Change the rendering strategy, for example to synchronized or differential
   rendering. This could address remaining flicker but adds work whose need and
   terminal behavior have not yet been verified.

## Decision

Use option 2, as implemented in the committed candidate: draw the initial frame
and redraw after display-related input, resize, or received scan events and
scanner disconnection. While a scan is active, use a 50 ms terminal-event poll
so scan results can be checked; a timeout alone does not cause a redraw. With no
active scan, block waiting for a terminal event. Ignored input and clipboard
copying do not themselves cause a redraw.

## Rationale

This removes the established source of repeated idle frames without preventing
input or chunked scan results from updating the interface. It does not assume
that idle redraw was the cause of the Windows symptom.

## Consequence

- The 50 ms interval remains relevant to checking scan results; it is no longer
  an unconditional frame interval.
- At acceptance in `REVIEW-0015`, drawing still cleared the whole screen on
  actual redraws and the Windows symptom remained unverified.
- Accepted follow-up `DECISION-0015` now governs changed-line rendering and
  replaces that rendering consequence; this record's scheduling choice remains
  in force. `ISSUE-0002` records the human's visual confirmation and resolution,
  with commit correspondence in `REVIEW-0016`.

## Affected Record Or Consumer

- `.project/issue/ISSUE-0002-tui-flicker-and-idle-redraw.md`
- `.project/decision/DECISION-0015-changed-line-tui-rendering.md`
- `.project/review/REVIEW-0015-tui-redraw-source-organization.md`
- `.project/review/REVIEW-0016-tui-flicker-and-footer-hints.md`
- `src/app.rs`
- `src/app/render.rs`
- `test/app.rs`
