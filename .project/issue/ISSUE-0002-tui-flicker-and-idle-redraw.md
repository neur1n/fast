---
id: ISSUE-0002
status: active
plan: none
blocked_by: []
review: .project/review/REVIEW-0015-tui-redraw-source-organization.md
---

# Issue: TUI Flicker and Idle Redraw

## Intended Result

Avoid unnecessary idle redraws while preserving responsive updates, and
determine whether the reported Windows flicker is eliminated in the original
terminal configuration.

## Scope

- Included: the observed flicker, the implemented idle-redraw change, its
  verification, and follow-up Windows visual verification.
- Excluded: claiming the flicker is fixed without that verification or selecting
  a further rendering change before the remaining behavior is observed.

## Acceptance Criterion

- After a scan finishes, idle waiting does not repeatedly redraw the screen;
  input, resize, and background scan updates continue to be handled.
- In the reported Windows setup, visually check the `/ filter …` line both
  while idle and during interaction, including a comparison without Zellij.
  Record the result and resolve the reported symptom based on that evidence.

## Verification Evidence

- The report identified flicker on Windows with WezTerm nightly, Zellij
  `0.45.1`, and Nushell `0.116.1`; it was not observed without Zellij.
- Commit `6fa809fd8e8f176e5f6e156a74f2f2efe26456db` has candidate tree
  `4d0cdd6704100631d633db67c3870f9b7ebfda81` and removes the unconditional idle
  redraw. On macOS, a pseudo-terminal check observed no output for 0.6 seconds
  after a 130-directory scan reached `Ready`, and exercised selection, copy,
  rescan, resize, filtering, and cancellation.
- The candidate passed 82 unit tests, including redraw and scan-event tests.
  The accepted implementation and full evidence are in `REVIEW-0015`; this
  issue remains open for the original Windows symptom.
- No Windows visual verification of the candidate has been performed; the
  original flicker is not confirmed fixed.

## Blocker

- Reason: the reported Windows terminal configuration is currently unavailable
  for visual verification. This blocks verification of the original symptom.
  The human confirmed that verification did not block the code commit, which has
  since been made.
- Resolution: the configuration becomes available and the candidate can be
  checked visually.
- Pending action: record the Windows result when access is available and assess
  whether the issue can be resolved or requires further investigation.

## Deferred Work

- Work: verify the original Windows scenario, including operation both with and
  without Zellij.
- Ceiling: actual updates still clear the whole screen, so flicker may remain.
- Trigger: access to the reported environment; if flicker persists, investigate
  whole-screen clearing and alternatives such as synchronized or differential
  rendering before choosing further changes.

## Related Records

- `.project/decision/DECISION-0013-event-driven-tui-redraw.md`
- `.project/review/REVIEW-0015-tui-redraw-source-organization.md`
