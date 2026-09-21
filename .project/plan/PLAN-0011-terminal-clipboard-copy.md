---
id: PLAN-0011
status: completed
roadmap: ROADMAP-0001
phase: phase-7-terminal-clipboard-copy
issue: []
review: .project/review/REVIEW-0014-terminal-clipboard-copy.md
---

# Plan: Terminal Clipboard Copy for 0.0.10

## Objective

Add a normal-navigation `y` shortcut that copies the highlighted entry's
logical path to the terminal host clipboard through OSC 52, without leaving the
TUI or changing directory selection behavior. Release the work as package
version `0.0.10`.

## Scope

- Included: enabling the existing `crossterm` OSC 52 clipboard feature and
  emitting a copy command for the clipboard destination.
- Included: handling lowercase `y` in normal navigation mode and copying the
  direct path of the highlighted entry, including directories, files, `.`, and
  `..` when those entries are visible.
- Included: preserving the current logical path spelling and avoiding
  canonicalization before copying.
- Included: leaving filter-mode `y` behavior unchanged so it remains a query
  character.
- Included: keeping the TUI active after a copy and leaving `q`, shell
  integration, selection-file output, scanning, filtering, and cache behavior
  unchanged.
- Included: user-facing shortcut documentation and regression coverage for
  dispatch, path selection, empty results, file-visible mode, and filter mode.
- Included: synchronizing package metadata, the lockfile, and `--version`
  output to `0.0.10` without changing the cache format or shell protocol.
- Excluded: native clipboard APIs, `pbcopy`/`xclip`/`wl-copy` fallbacks,
  clipboard paste, primary-selection support, and clipboard history.

## Acceptance Criteria

- Pressing lowercase `y` in normal navigation mode emits an OSC 52 copy for
  the path returned by the highlighted `DirectoryEntry`.
- A highlighted file is copied as the file path rather than being mapped to the
  current directory as `q` does.
- The synthetic `.` and `..` entries copy their displayed logical paths, and an
  empty filtered result emits no copy command.
- Pressing `y` in filter mode extends the filter query and does not copy.
- Copying does not open an entry, exit the TUI, alter selection, or change the
  shell selection protocol.
- Non-UTF-8 paths are handled without emitting malformed clipboard text and
  without terminating normal navigation unexpectedly.
- README and the in-TUI shortcut footer document OSC 52 terminal or
  multiplexer support as a prerequisite.
- `Cargo.toml`, `Cargo.lock`, and `cargo run --quiet -- --version` identify the
  package as `0.0.10`.

## Steps

1. Record and apply the OSC 52 clipboard decision while preserving the existing
   terminal output ownership and event-loop behavior.
2. Enable the `osc52` feature for `crossterm` and integrate a copy command into
   the normal-mode key path using the highlighted entry's direct path.
3. Define the UTF-8/error behavior and ensure empty selections and filter-mode
   input remain no-ops for copying.
4. Add regression tests for key dispatch, direct directory/file paths,
   synthetic navigation entries, empty results, and filter-mode input.
5. Update the footer and README, synchronize package metadata to `0.0.10`, and
   keep the cache format and shell selection protocol unchanged.
6. Run the implementation checks and prepare an exact-tree implementation
   review.

## Affected File Or Interface

- `src/app.rs`
- `Cargo.toml`
- `Cargo.lock`
- `README.md`
- `.project/decision/DECISION-0012-terminal-clipboard-copy.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/project.json`
- `.project/STATE.md`

## Risk And Reversibility

- OSC 52 depends on terminal-emulator and multiplexer policy; unsupported or
  disabled terminals may silently ignore the copy command. Documentation and a
  manual smoke test are required, and a native fallback remains explicitly
  outside this workstream.
- Enabling `crossterm`'s feature adds its encoding dependency to the lockfile,
  but does not introduce an external executable, persistent data migration, or
  shell protocol change. Removing the feature is reversible.
- A path containing non-UTF-8 bytes cannot be represented reliably as text;
  rejecting that copy request is safer than silently replacing path bytes.
- OSC 52 sends the selected path through the terminal transport by design. The
  terminal's existing clipboard and escape-sequence security settings remain
  authoritative.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --locked`
- `cargo check --locked`
- `git diff --check`
- `cargo run --quiet -- --version` reports `fast 0.0.10`.
- Verify the help/footer documentation includes `y` and the OSC 52
  prerequisite.
- In a terminal that permits OSC 52, verify copying a directory, file, `.`, and
  `..`, then paste after leaving `fast`.
- Verify filter-mode `y`, empty filtered results, and unsupported-terminal
  behavior do not change navigation or shell selection semantics.

## Completion Evidence

- Commit `cbcfef71e42f7c91f7479fd4ad34aa2908d2eb7f` implements the OSC 52
  clipboard shortcut, `0.0.10` metadata, documentation, regression coverage,
  and the approved cleanup scope.
- The committed tree is
  `aac58992078b91623677bf3e05b4c307b503c7e5`, with the parent and exact staged
  paths recorded in `REVIEW-0014`.
- Human acceptance and the implementation checks are recorded in
  `REVIEW-0014`.
