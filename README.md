# fast

FAST is a Shell Traverser.

`fast` is a small, cross-platform TUI for browsing directories and changing the
shell working directory.

## Status

The Phase 2 navigator and the built-in Phase 3 simple and fuzzy filters are
implemented. It starts in the current directory, lists direct child directories,
uses a persistent fingerprinted cache for visited directories, and falls back to
cancellable chunked scanning when the cache is missing or stale. Cache failures
do not prevent navigation.

## Run

```sh
cargo run
```

Set `FAST_CACHE_DIR` to override the platform cache directory, for example when
testing cache behavior in an isolated directory.

Use the arrow keys or `j`/`k` to move, `Enter` or `l` to open a directory,
`Backspace` or `h` to go to its parent, `r` to rescan, `q` to select the
highlighted directory, and `Esc` to cancel. Press `/` to enter filter mode;
typed text is matched as a case-insensitive substring of directory names by
default. Press `Tab` in filter mode to switch between simple and fuzzy matching;
fuzzy matching keeps query characters in order and ranks better-scoring names
first. `Backspace` edits the query, `Enter` keeps the filter and returns to
navigation, and `Esc` clears an active filter. Press `Esc` again when no filter
is active to cancel.

## Shell Integration

Install the binary so the wrappers can find `fast`:

```sh
cargo install --path .
```

If Cargo's binary directory is not already in `PATH`, add it before using the
wrapper:

```sh
# Bash/Zsh
export PATH="$HOME/.cargo/bin:$PATH"

# Nushell
$env.PATH = ($env.PATH | prepend ($nu.home-dir | path join ".cargo" "bin"))
```

For a local checkout without installing, build the binary and set `FAST_BIN`:

```sh
cargo build
export FAST_BIN="$PWD/target/debug/fast"  # Bash/Zsh
# Nushell: $env.FAST_BIN = (pwd | path join "target" "debug" "fast")
```

Source the matching wrapper in the shell where the directory should change:

```sh
# Bash
source /path/to/fast/shell/fast.bash

# Zsh
source /path/to/fast/shell/fast.zsh

# Nushell
source /path/to/fast/shell/fast.nu
```

Run `fast` from that shell. The wrapper keeps the TUI attached to the terminal,
then changes the parent shell's directory after `q` confirms the highlighted selection.
`Esc` or `Ctrl-C` leaves the directory unchanged.

## Initial Scope

- Linux x86_64 and aarch64
- macOS arm64
- Windows x86_64
- Single-column directory-only browsing
- Keyboard interaction without mouse support
- Chunked directory scanning with an indexing indicator
- Persistent cache for visited directories
- Built-in simple directory-name filtering
- Built-in fuzzy directory-name filtering
- Bounded background prefetch of direct child directories
- Nushell, Bash, and Zsh integration

## Non-goals

- Full file-management operations
- Recursive scanning by default
- File previews, icons, Git integration, or content search
- A mandatory external database or fuzzy-finder executable

Development follows the governance records under `.project/`.
