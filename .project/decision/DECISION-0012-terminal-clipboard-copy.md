---
id: DECISION-0012
status: accepted
date: 2026-09-21
supersedes: none
review: .project/review/REVIEW-0014-terminal-clipboard-copy.md
---

# Decision: Use OSC 52 for Terminal Clipboard Copy

## Context

`fast` is a cross-platform terminal UI that needs a keyboard shortcut for
copying the highlighted path. The implementation should not depend on external
clipboard executables, should work when `fast` is running through SSH, and
should avoid adding platform-specific native clipboard code for Linux,
macOS, and Windows.

## Options

1. Emit OSC 52 through `crossterm`. This keeps the implementation terminal
   native, works across the supported host platforms and SSH, and adds no
   external runtime executable, but requires terminal or multiplexer support.
2. Use a native clipboard crate such as `arboard`. This can access desktop
   clipboard APIs directly, but adds platform backends and Linux Wayland and
   clipboard-owner lifecycle concerns.
3. Invoke platform commands such as `pbcopy`, `xclip`, `wl-copy`, or `clip`.
   This avoids a Rust backend but makes behavior depend on installed commands
   and differs across supported platforms.

## Decision

Use option 1. Enable `crossterm`'s `osc52` feature and emit
`CopyToClipboard::to_clipboard_from` for the standard clipboard destination.

The shortcut is lowercase `y` in normal navigation mode only. It copies the
highlighted entry's direct logical path without canonicalization. Directory,
file, `.`, and `..` entries use the same direct path rule. In filter mode,
`y` remains ordinary query input. Native clipboard fallbacks, paste, and
primary-selection support are outside this workstream.

Paths must be valid UTF-8 before being emitted as clipboard text. An invalid
path must not be silently rewritten into a different path string.

## Rationale

OSC 52 matches the application's terminal-first architecture and preserves
clipboard ownership in the terminal host after `fast` exits. It avoids making
desktop clipboard availability a build or runtime prerequisite and is useful
for remote sessions. The terminal-support limitation is explicit and can be
addressed by a separately reviewed native fallback if actual usage requires
it.

## Consequence

- Terminals and multiplexers must permit OSC 52 for the shortcut to have an
  effect; the application cannot reliably detect a terminal policy rejection.
- The `crossterm` lockfile graph gains the encoding dependency required for
  OSC 52.
- The shell selection protocol, cache format, scanner, and `q` behavior remain
  unchanged.
- The copied value is the logical path currently shown by `fast`, not a
  canonical physical path.

## Affected Record Or Consumer

- `.project/plan/PLAN-0011-terminal-clipboard-copy.md`
- `.project/roadmap/ROADMAP-0001.md`
- `.project/project.json`
- `.project/STATE.md`
- `src/app.rs`
- `Cargo.toml`
- `Cargo.lock`
- `README.md`
