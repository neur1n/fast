---
id: DECISION-0008
status: accepted
date: 2026-09-08
supersedes: none
review: .project/review/REVIEW-0009-logical-path-preservation-and-version-0.0.6.md
---

# Decision: Preserve Logical Paths Through Symlinks

## Context

When a shell starts `fast` from a directory reached through a symlink, the
operating system's current-directory API can report the physical target path
instead of the logical path shown by the shell. Starting from the physical path
causes `fast` to display the target directory and makes parent navigation leave
the symlink path.

## Decision

At startup, obtain the physical current directory from `env::current_dir()` and
the shell's logical path from `PWD`. Use `PWD` only when it is absolute and its
canonical path matches the canonical physical current directory. Otherwise,
retain the physical current directory as the safe fallback.

After startup, preserve the selected logical path through normal navigation.
Do not canonicalize `current_dir` when entering a directory or when calculating
its parent. Shells configured for physical-path mode are intentionally outside
this guarantee because they have already discarded the logical symlink path.

## Rationale

The validated `PWD` value restores the shell's path spelling without trusting an
unrelated or stale environment variable. Keeping the path lexical after the
startup check makes `Path::parent()` return the symlink's containing directory,
while filesystem operations continue to follow the symlink normally.

The fallback avoids turning an invalid environment value into a startup error
and keeps behavior correct for direct process launches and shells that do not
provide `PWD`.

## Consequence

- Starting from a valid logical symlink path preserves that path in the TUI.
- Returning from a symlinked directory returns to its logical parent.
- Invalid, relative, or mismatched `PWD` values do not affect the startup path.
- Physical-path shell modes remain the user's responsibility.
- The cache format, shell selection protocol, and directory scanning behavior
  remain unchanged.

## Affected Record Or Consumer

- `.project/plan/PLAN-0008-logical-path-preservation-and-version-0.0.6.md`
- `.project/review/REVIEW-0009-logical-path-preservation-and-version-0.0.6.md`
- `.project/STATE.md`
- `.project/roadmap/ROADMAP-0001.md`
- `src/main.rs`
