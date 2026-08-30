# fast

FAST is a Shell Traverser.

`fast` is planned as a small, cross-platform TUI for browsing directories and
changing the shell working directory.

## Status

The project currently contains its governance and formatting foundation. The
application implementation has not started.

## Initial Scope

- Linux x86_64 and aarch64
- macOS arm64
- Windows x86_64
- Single-column directory-only browsing
- Keyboard interaction without mouse support
- Chunked directory scanning with an indexing indicator
- Persistent cache for visited directories
- Bounded background prefetch of direct child directories
- Nushell, Bash, and Zsh integration

## Non-goals

- Full file-management operations
- Recursive scanning by default
- File previews, icons, Git integration, or content search
- A mandatory external database or fuzzy-finder executable

Development follows the governance records under `.project/`.
