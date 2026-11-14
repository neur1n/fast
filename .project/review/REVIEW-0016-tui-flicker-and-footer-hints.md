---
id: REVIEW-0016
status: approved
type: implementation
target: tui-flicker-and-footer-hints
base_commit: ae4287b5558d6710bdcf6b3ef971b2019db115b6
candidate_tree: b8bd438fe228234574f183daf343ad4816bf2561
scope:
  - Changed-line rendering with buffered synchronized output
  - Human-edited colors and reduced footer hints
  - Rendering and output regression tests
staged_paths:
  - src/app.rs
  - src/app/render.rs
  - test/app.rs
  - test/render.rs
reviewer: human reviewer
date: 2026-11-15
provenance: >-
  Human confirmed flashing was gone, accepted the working-tree changes and final
  hints, authorized governance preparation, then supplied candidate tree
  b8bd438fe228234574f183daf343ad4816bf2561 and explicitly accepted it.
  The agent inspected that exact tree and verified its implementation scope.
  Human subsequently supplied commit 607f0dc9e6af9f9a864dce0e48a663c199502c66
  and authorized governance finalization; the committed tree matches.
verdict: approve
transition: >-
  Accept the committed implementation and DECISION-0015, complete ISSUE-0002,
  and retain the deferred Phase 4 prefetch frontier.
candidate_commit: 607f0dc9e6af9f9a864dce0e48a663c199502c66
---

# Review: TUI Flicker and Footer Hints

## Related Records

- `.project/issue/ISSUE-0002-tui-flicker-and-idle-redraw.md` owns the symptom,
  experiments, capture evidence, and human visual confirmation.
- `.project/decision/DECISION-0015-changed-line-tui-rendering.md` records the
  durable rendering choice and its limits.
- `.project/decision/DECISION-0013-event-driven-tui-redraw.md` continues to govern
  event-driven redraw scheduling.
- `REVIEW-0015` remains the historical review of the earlier idle-redraw and
  source-organization implementation; it does not cover this candidate.

## Candidate Scope

The supplied tree changes exactly these four paths against `base_commit`:

- `src/app.rs`: retain one renderer across requested redraws without changing
  event scheduling or key handling.
- `src/app/render.rs`: changed-line comparison, buffered synchronized output,
  human-edited colors, and the final footer wording.
- `test/app.rs`: output-wrapper regression tests and a shared application
  fixture for rendering tests.
- `test/render.rs`: new rendering and footer regression tests.

The footer acceptance text is:

```text
 / filter  q select  r rescan  y copy path  F files off  Esc cancel
 / edit filter  q select  r rescan  y copy path  F files off  Esc clear/cancel
 Tab toggle mode  Enter apply  Esc clear
```

File visibility substitutes `on` for `off` as appropriate. Navigation keys and
Ctrl-C still work but are not advertised. Esc clears active filtering before a
subsequent Esc cancels; only the hints changed.

The implementation commit excludes pending governance. Its finalization is
maintained separately in:

- `.project/decision/DECISION-0013-event-driven-tui-redraw.md`
- `.project/decision/DECISION-0015-changed-line-tui-rendering.md`
- `.project/issue/ISSUE-0002-tui-flicker-and-idle-redraw.md`
- `.project/review/REVIEW-0016-tui-flicker-and-footer-hints.md`
- `.project/STATE.md`

## Evidence

- The human supplied tree `b8bd438fe228234574f183daf343ad4816bf2561` and
  accepted it. `git cat-file -t` confirmed it is a tree. At candidate inspection,
  `HEAD` was the unambiguous base
  `ae4287b5558d6710bdcf6b3ef971b2019db115b6`.
- Read-only comparison against that base confirms exactly the four
  implementation paths listed above, including the new `test/render.rs`.
  Pending governance is excluded.
- `git diff --cached --exit-code b8bd438fe228234574f183daf343ad4816bf2561`
  passed, confirming the index matches the supplied tree without modifying it.
- `git diff --exit-code b8bd438fe228234574f183daf343ad4816bf2561 -- Cargo.toml
  Cargo.lock build.rs .cargo src test` passed. The manifests, configuration,
  sources, and tests used by the checks match the supplied candidate.
- After the tree was supplied and compared, `cargo test --locked` again passed
  all 103 Windows tests on that matching working tree.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo build --locked`, and
  `git diff --check ae4287b5558d6710bdcf6b3ef971b2019db115b6
  b8bd438fe228234574f183daf343ad4816bf2561` also passed.
- Output tests cover synchronized begin/content/end ordering, buffering of a
  64 KiB frame, and drawing, write, and flush errors.
- Rendering tests cover first and resized frames, no-op frames, selection
  movement, scrolling, style changes, filtering and stale-row removal, footer
  transitions and exact text, file visibility, timestamp changes, clipping,
  bottom-right avoidance, tiny viewports, and cache preservation after errors.
- The checks ran in the matching working directory, not a separate checkout;
  the tree comparisons above establish their candidate correspondence. The
  prior capture analysis and the distinction between application output and
  terminal presentation remain in `ISSUE-0002`.
- The human subsequently supplied commit
  `607f0dc9e6af9f9a864dce0e48a663c199502c66`. Read-only inspection confirms
  parent `ae4287b5558d6710bdcf6b3ef971b2019db115b6` and tree
  `b8bd438fe228234574f183daf343ad4816bf2561`, exactly matching the reviewed
  candidate. Comparing the supplied tree with the commit produces no diff;
  the base-to-commit whitespace check also passes. The human's revised commit
  message does not change candidate identity.

## Human Finding

- The human reported "the flashing is gone!" after changed-line rendering and
  explicitly accepted those changes.
- The human adjusted colors directly, retained Esc rather than adopting a
  Ctrl-C cancel hint, and specified the final browsing order and filter-input
  wording.
- After the final hints were implemented and checked, the human said "LGTM" and
  requested the necessary governance updates before committing.
- The human then supplied tree `b8bd438fe228234574f183daf343ad4816bf2561`
  and said "I accept it", binding acceptance to this implementation candidate.
- The human supplied the implementation commit and explicitly requested
  governance finalization, including checking whether `project.json` and
  `STATE.md` needed updates.

## Condition

- Blocking for implementation acceptance and governance finalization: none.
  The exact candidate, base, scope, human acceptance, and committed-tree
  correspondence have been verified.
- The reported visual symptom is confirmed resolved by the human; this record
  does not add a new visual-verification requirement or claim testing of other
  terminal configurations.

## Agent Assessment

- The reviewed candidate matches the described row-difference strategy
  and final footer requirements. It preserves the existing event-driven redraw
  scheduling and key behavior.
- Human-edited colors are part of the reviewed candidate and were not
  overwritten by the hint changes. Current source uses Blue for the title and
  Red for status; file entries, the file separator, and footer use DarkGrey.
  This records the inspected candidate scope, not a mandated palette.
- Version `0.0.11`, dependency versions, CLI selection output, and the persistent
  cache format are unchanged.
- There is no performance benchmark or claim that comparison is globally faster.
  Row-level updates can still tear, and the responsible Windows component has
  not been established. `DECISION-0015` records the ceiling and trigger for
  further work.
- No blocking implementation findings were identified in the supplied tree.
  Its approval is tree-bound; any candidate change requires a new tree ID and
  review round.
- The committed tree matches the reviewed candidate. `DECISION-0015` is accepted
  and `ISSUE-0002` is complete on the human's authorization and visual evidence.
  `DECISION-0013` retains its scheduling choice and now points to the accepted
  rendering follow-up rather than leaving its historical consequence
  unqualified.
- Navigation reconciliation retains deferred Phase 4 prefetch as the selected
  frontier. `STATE.md` refreshes its navigation summary and reconciliation date,
  with canonical links. `project.json` needs no change because the workflow,
  record paths, and selected plan are unchanged.

## Human Decision

- Accept implementation commit `607f0dc9e6af9f9a864dce0e48a663c199502c66`,
  whose tree matches the supplied candidate, including the rendering strategy,
  human-edited colors, and final hints.
- Accept `DECISION-0015` and complete `ISSUE-0002` under the human's explicit
  governance-finalization request. The reported Windows flashing is resolved
  based on the human's observation; this is not a claim that every terminal
  configuration was tested or that a particular Windows component was repaired.
- Leave Phase 4 bounded prefetch deferred. The human reviews and commits the
  governance separately; no Git-changing operations were performed by the agent.
