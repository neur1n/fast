---
id: ISSUE-0002
status: completed
plan: none
blocked_by: []
review: .project/review/REVIEW-0016-tui-flicker-and-footer-hints.md
---

# Issue: TUI Flicker and Idle Redraw

## Intended Result

Avoid unnecessary idle redraws while preserving responsive updates, and
determine whether the reported Windows flicker is eliminated in the original
terminal configuration.

## Scope

- Included: the observed flicker, the implemented idle-redraw change, the
  follow-up buffered synchronized output and changed-line rendering candidate,
  automated checks, Windows visual verification, and the human-approved
  pre-commit footer-hint reduction.
- Excluded: claiming the flicker is fixed without visual verification, changing
  ConPTY or Zellij, or adding cell-level differential rendering.

## Acceptance Criterion

- After a scan finishes, idle waiting does not repeatedly redraw the screen;
  input, resize, and background scan updates continue to be handled.
- Rendering does not issue application-level full-screen clears. With unchanged
  dimensions, only rows whose visible text, color, or selection style changed
  are emitted. Non-scrolling selection movement updates only the old and new
  selection rows; an unchanged footer is not rewritten during movement or
  scrolling.
- The first frame and frames with changed dimensions redraw every visible row.
  Removed entries become blank rows, and the bottom line reserves the last
  column to avoid bottom-right autowrap.
- Changed-row commands remain bracketed with DEC mode 2026 markers, assembled
  in memory, submitted with one `write_all` call, and then flushed. Identical
  frames emit nothing. The previous-frame cache advances only after successful
  output; drawing, write, or flush errors do not hide pending changes.
- In the reported Windows setup, visually check the `/ filter …` line both
  while idle and during interaction, including a comparison without Zellij.
  Record the result and resolve the reported symptom based on that evidence.
- Browsing hints list only filter, select, rescan, copy path, file visibility,
  and Esc, in that order. Filter input shows only `Tab toggle mode`, `Enter
  apply`, and `Esc clear`. Existing filtering still takes one Esc to clear and
  another to cancel; no key bindings or human-edited colors are changed by the
  hint reduction.

## Verification Evidence

- The report identified flicker on Windows with WezTerm nightly, Zellij
  `0.45.1`, and Nushell `0.116.1`; it was not observed without Zellij.
- Commit `6fa809fd8e8f176e5f6e156a74f2f2efe26456db` has candidate tree
  `4d0cdd6704100631d633db67c3870f9b7ebfda81` and removes the unconditional idle
  redraw. On macOS, a pseudo-terminal check observed no output for 0.6 seconds
  after a 130-directory scan reached `Ready`, and exercised selection, copy,
  rescan, resize, filtering, and cancellation.
- The candidate passed 82 unit tests, including redraw and scan-event tests.
  The accepted implementation and full evidence are in `REVIEW-0015`; at that
  point, this issue remained open for the original Windows symptom.
- On 2026-11-14, the human reported that after updating the repository, the
  bottom line no longer flickers continuously but still flashes on each
  selection movement.
- The first follow-up working-tree candidate used crossterm's `sync_update`
  around the complete draw directly on stdout. Event-driven scheduling,
  full-screen clearing, dependency versions, and package version `0.0.11`
  remained unchanged.
- On Windows, `cargo test --locked` passed 80 baseline tests before this change
  and 83 tests afterward. The three new tests cover begin/content/end ordering,
  flushing after the end marker, drawing-error propagation with a frame-end
  attempt, and flush-error propagation. `cargo clippy --locked --all-targets
  --all-features -- -D warnings` and `cargo fmt --all -- --check` passed.
- The human reported that synchronization reduced the symptom to occasional
  flashes. Slower movement made flashes less frequent, but intervals of one to
  four seconds did not reveal a consistent pattern. The executable is native
  Windows `fast.exe`.
- A native Windows capture used the installed Zellij `0.46.0` and Nushell
  `0.117.0`. The local artifact is
  `%LOCALAPPDATA%/Temp/opencode/fast-flicker-20261114-192445/zellij/zellij-log/zellij-0.log`;
  the human supplied a `192611` path, but the capture was found under `192445`.
  Only the `fast` alternate-screen region, byte range `[8184, 28834)`, was
  analyzed for frame boundaries. It contains 22 begin markers, 22 end markers,
  and 22 footer outputs; 20 footer outputs occur outside synchronized intervals.
  The invoked debug-binary path is present in the capture.
- This establishes a framing anomaly in the bytes received by Zellij, not which
  Windows output component caused it or which particular updates visibly
  flashed. Source-level framing alone is not sufficient verification.
- The human authorized a minimal buffering experiment. That candidate rendered
  the complete synchronized frame into a `Vec<u8>`, then wrote that buffer to
  stdout and flushed. Layout, clearing, scheduling, dependencies, CLI behavior,
  cache format, and package version `0.0.11` remained unchanged.
- A new 64 KiB regression test failed against direct output with five writes
  (`[8, 6, 65536, 6, 8]`) and passes with one complete buffered write. The
  buffered candidate passed 85 Windows unit tests, including write-error and
  flush-error propagation. Clippy with warnings denied and the formatting check
  also passed. These tests exercise the application-side output wrapper, not
  ConPTY or Zellij rendering.
- The human reported less frequent flashing with buffering and supplied
  `%LOCALAPPDATA%/Temp/opencode/fast-flicker-20261114-203349/zellij/zellij-log/zellij-0.log`.
  Its `fast` alternate-screen region, `[8256, 81532)`, contains 92 begin/end
  pairs and 92 footer outputs; 90 footer outputs are outside synchronization.
  Most intervals contain only `BEGIN -> CLEAR -> END`, followed by page
  contents. Buffering did not restore the received frame boundaries. These
  counts do not measure visible flash frequency.
- After reviewing that evidence, the human authorized changed-line rendering.
  On 2026-11-15, the candidate was updated to cache the previous visible rows
  and dimensions in a renderer local to the event loop. It compares clipped
  text, color, and reverse-selection state, emits changed rows without clearing
  the whole screen, and retains buffered synchronized output. Event scheduling,
  CLI behavior, dependencies, cache format, and package version `0.0.11` remain
  unchanged.
- `cargo test --locked` passed 100 Windows tests. The 15 new rendering tests
  cover initial and resized viewports, no-op frames, non-scrolling movement,
  scrolling, style-only changes, filtering and stale-row clearing, footer
  transitions, timestamp updates, file-header insertion/removal, clipped text,
  bottom-right avoidance, tiny viewports, and cache preservation after drawing,
  write, or flush errors. The tests record the actual row-selection decisions
  through an injected row writer; they are not ConPTY or visual tests.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, and `cargo build --locked` passed.
  `cargo package --list --allow-dirty --locked` includes the new
  `test/render.rs` module; its existing missing-package-metadata warning remains
  outside this change.
- The human subsequently reported that flashing was gone with changed-line
  rendering and accepted the changes before requesting pre-commit UI tweaks.
  This is human visual evidence, not an agent-run terminal test or identification
  of the specific Windows component responsible for the earlier framing anomaly.
- The human adjusted colors directly and approved shorter browsing hints while
  explicitly retaining Esc instead of adopting a unified Ctrl-C cancel hint.
  The footer now omits navigation hints and browsing-mode Tab. The human then
  requested browsing hints in `/`, `q`, `r`, `y`, `F`, `Esc` order, including
  browsing with a retained filter. That state still says `Esc clear/cancel`;
  filter input now shows only `Tab toggle mode  Enter apply  Esc clear`.
  Ctrl-C remains supported by the unchanged key handler, but is no longer
  advertised in the footer. The manual color changes are preserved.
- `cargo test --locked` passed 103 Windows tests after the hint reduction,
  including three new tests for the browsing actions, file on/off state,
  filter-input hints, 79-column fit, and Esc clear-then-cancel behavior.
  Clippy with warnings denied also passed.
- After the final hint-order and filter-input wording changes, the same 103
  Windows tests, Clippy with warnings denied, formatting check, build, and
  `git diff --check` passed again. Tests assert the complete footer text in
  browsing and filter-input modes; Esc behavior remains unchanged.
- The human then said "LGTM" and authorized preparation of the necessary
  governance before committing. That preparation introduced proposed
  `DECISION-0015` for the rendering strategy and `REVIEW-0016` for the
  candidate scope, working-tree checks, human findings, and tree-bound review.
  `REVIEW-0015` remains evidence for the earlier implementation only.
- The human subsequently supplied tree
  `b8bd438fe228234574f183daf343ad4816bf2561` and explicitly accepted it.
  Read-only inspection against base
  `ae4287b5558d6710bdcf6b3ef971b2019db115b6` confirmed the four expected
  implementation paths, with pending governance excluded. The index and
  checked implementation match that tree; 103 Windows tests, Clippy, formatting,
  build, and candidate-diff checks passed again. `REVIEW-0016` records the
  tree-bound approval.
- The human supplied implementation commit
  `607f0dc9e6af9f9a864dce0e48a663c199502c66` and authorized governance
  finalization. Read-only inspection confirms the reviewed base and exact
  candidate tree, with no diff between the supplied tree and committed content.
  `REVIEW-0016` records that correspondence and the final acceptance;
  `DECISION-0015` records the accepted rendering choice.

## Blocker

- Reported symptom: the human confirms the flashing is gone; the earlier visual
  verification blocker is no longer outstanding for the changed-line result.
- Remaining blocker: none. The implementation is committed, matches the
  accepted candidate, and has human authorization for governance finalization.
- Pending action: none for issue resolution. Any reproducible recurrence is
  subject to the deferred-work trigger below.

## Deferred Work

- Work: investigate further terminal-output changes or the Windows output
  components if needed.
- Ceiling: the renderer differs at row granularity, not cell granularity. It
  still computes visible rows on each requested redraw and does not repair
  ConPTY's synchronization behavior. Changes within a row can still tear.
- Trigger: reproducible recurring flicker, particularly when its PTY capture
  confirms that movement emits no footer or full-screen clear commands.

## Related Records

- `.project/decision/DECISION-0013-event-driven-tui-redraw.md`
- `.project/decision/DECISION-0015-changed-line-tui-rendering.md`
- `.project/review/REVIEW-0015-tui-redraw-source-organization.md`
- `.project/review/REVIEW-0016-tui-flicker-and-footer-hints.md`
