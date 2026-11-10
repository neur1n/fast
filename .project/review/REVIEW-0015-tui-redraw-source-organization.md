---
id: REVIEW-0015
status: approved
type: implementation
target: tui-redraw-and-source-organization
base_commit: 41e804c34026fafa99b1c181a59576664e1a7470
candidate_tree: 4d0cdd6704100631d633db67c3870f9b7ebfda81
scope:
  - Event-driven redraw and idle terminal waiting
  - Application rendering module and isolated unit tests
  - Import and enum-variant ordering
  - Package version 0.0.11 and the human-selected LICENSE changes
staged_paths:
  - Cargo.lock
  - Cargo.toml
  - LICENSE
  - src/app.rs
  - src/app/render.rs
  - src/cache.rs
  - src/cli.rs
  - src/filter.rs
  - src/main.rs
  - src/scan.rs
  - test/app.rs
  - test/cache.rs
  - test/cli.rs
  - test/filter.rs
  - test/main.rs
  - test/scan.rs
reviewer: human reviewer
date: 2026-11-10
provenance: >-
  Human accepted the implementation, selected version 0.0.11, confirmed LICENSE
  inclusion and non-blocking Windows verification, supplied the candidate tree
  and commit, and requested governance finalization.
verdict: approve
transition: >-
  Accept the committed implementation and its two decisions; keep ISSUE-0002
  open for Windows verification and leave Phase 4 prefetch deferred.
candidate_commit: 6fa809fd8e8f176e5f6e156a74f2f2efe26456db
---

# Review: TUI Redraw and Source Organization

## Related Records

- `.project/issue/ISSUE-0002-tui-flicker-and-idle-redraw.md` tracks the reported
  Windows symptom and its outstanding verification.
- `.project/decision/DECISION-0013-event-driven-tui-redraw.md` records the
  accepted redraw scheduling choice.
- `.project/decision/DECISION-0014-root-unit-test-organization.md` records the
  accepted long-term unit-test layout.

## Evidence

- Commit `6fa809fd8e8f176e5f6e156a74f2f2efe26456db` has parent
  `41e804c34026fafa99b1c181a59576664e1a7470` and tree
  `4d0cdd6704100631d633db67c3870f9b7ebfda81`, matching the supplied candidate.
  It changes exactly the 16 paths listed above; governance records are not in
  the code commit.
- `Cargo.toml` and `Cargo.lock` in the candidate tree both specify `0.0.11`;
  `cargo run --locked --quiet -- --version` reported `fast 0.0.11`.
- On macOS, `cargo test --locked` passed all 82 tests,
  `cargo clippy --locked --all-targets --all-features -- -D warnings` passed,
  and `cargo fmt --all -- --check` passed.
- `git diff --check 41e804c34026fafa99b1c181a59576664e1a7470 4d0cdd6704100631d633db67c3870f9b7ebfda81`
  passed.
- `cargo package --list --allow-dirty --locked` included the rendering module
  and all six `test/` modules. Cargo warned about missing package metadata,
  including a manifest license field; this warning predates this work.
- A macOS pseudo-terminal check scanned 130 directories and observed no output
  during 0.6 seconds of idle time after `Ready`. Selection, OSC 52 copy, rescan,
  resize, filtering, and cancellation were also exercised. This was not a
  Windows, WezTerm, or Zellij visual test.
- The application and rendering function bodies and existing test contents were
  compared with the pre-organization working baseline. Subsequent changes were
  limited to import grouping and enum-variant ordering; the `Fuzzy` default
  attribute remained attached to its variant.
- The `LICENSE` change was present before this work. The human confirmed its
  inclusion; it reformats text and fills the two appendix notice templates.

## Human Finding

- The human accepted the implementation, chose version `0.0.11`, confirmed
  that `LICENSE` belongs in the code candidate, and requested governance
  tracking before the commit.
- The human confirmed that Windows visual verification may remain outstanding
  without blocking the code commit, and that root `test/` is the long-term
  convention for future unit tests.

## Condition

- Blocking for the code transition: none.
- Non-blocking, confirmed by the human for the code commit: Windows visual
  verification remains unavailable. The change removes idle redraws but still
  clears the screen on actual updates, so it does not establish that the
  reported flicker is fixed.

## Agent Assessment

- The candidate preserves chunked background scanning while drawing after
  display-related input, resize, and scan events, and waiting for terminal
  events when no scan is active.
- Rendering is isolated under `src/app/`; existing unit tests remain private
  module tests and are stored under the root `test/` directory. Every Rust
  source and test file is under 2,000 lines.
- Reordering enum variants can alter implicit discriminants. No current use of
  enum ordering, numeric casts, or serialization was found.
- When Windows access returns, test the reported WezTerm, Zellij, and Nushell
  configuration both idle and during interaction. If flicker remains, revisit
  whole-screen clearing and synchronized or differential rendering.
- The committed tree matches the reviewed candidate; the selected Phase 4
  prefetch frontier remains deferred.

## Human Decision

- The human accepted the implementation, supplied its commit, and requested
  governance finalization. Windows visual verification is non-blocking for the
  code commit; this does not establish that the original flicker is fixed.
- Accept the two associated decisions, keep `ISSUE-0002` open for Windows
  verification, and leave the Phase 4 prefetch frontier deferred.
