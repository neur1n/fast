<pre align="center">
█▀▀▀▀▀▀▀█▀▀▀▀▀▀▀█▀▀▀▀▀▀▀█▀▀▀▀▀▀▀█
█   ▄▄▄▄█   ▄   █   ▄▄▄▄█▄▄   ▄▄█
█       █       █       ███   ███
█   █████   █   █▀▀▀▀   ███   ███
█▄▄▄█████▄▄▄█▄▄▄█▄▄▄▄▄▄▄███▄▄▄███
</pre>

<p align="center">
  FAST is a Shell Traverser
</p>

<details open=true>
  <summary>Table of Contents</summary>
  <ul>
    <li><a href="#features">Features</a></li>
    <li><a href="#installation">Installation</a></li>
    <li><a href="#shell-integration">Shell Integration</a></li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#license">License</a></li>
  </ul>
</details>

## Features

`fast` is a small, cross-platform TUI for `ls` + `cd`.

<ul>
  <li>
    <details>
      <summary><strong>Responsive on huge directories</strong></summary>
      <p>
        Directory entries are discovered and rendered in chunks instead of
        waiting for the complete scan. The first results appear while the rest
        are still being scanned, so browsing can start immediately.
      </p>
    </details>
  </li>
  <li>
    <details>
      <summary><strong>Skip repeat scans</strong></summary>
      <p>
        Visited directories are stored with a fingerprint. An unchanged
        directory can be shown from the persistent cache without a full scan;
        missing or stale cache entries trigger a fresh scan instead.
      </p>
    </details>
  </li>
  <li>
    <details>
      <summary><strong>Keyboard-first navigation</strong></summary>
      <p>
        Start in the current directory, browse its child directories, return to
        the parent, rescan, and select a directory without leaving the keyboard.
      </p>
    </details>
  </li>
  <li>
    <details>
      <summary><strong>Simple filtering</strong></summary>
      <p>
        Type part of a directory name to use predictable, case-insensitive
        substring matching. The parent directory entry remains available while
        filtering.
      </p>
    </details>
  </li>
  <li>
    <details>
      <summary><strong>Fuzzy filtering</strong></summary>
      <p>
        Switch from simple matching to fuzzy matching with <code>Tab</code>.
        Query characters only need to appear in order, and better-scoring names
        are ranked first when an exact substring is not convenient.
      </p>
    </details>
  </li>
  <li>
    <details>
      <summary><strong>Shell integration</strong></summary>
      <p>
        Bash, Zsh, and Nushell wrappers read the selected path and apply it with
        <code>cd</code> in the parent shell. Confirm with <code>q</code> to change
        directories, or cancel with <code>Esc</code> without changing the shell.
      </p>
    </details>
  </li>
</ul>

## Installation

### Download a Release

Download an archive from the [Releases](https://github.com/neur1n/fast/releases)
page. Extract it, put the `fast` binary in `PATH`, and keep the bundled `shell/`
directory available for shell integration.

### Build from Source

Install the binary with Cargo:

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

# Bash/Zsh
export FAST_BIN="$PWD/target/debug/fast"

# Nushell
$env.FAST_BIN = (pwd | path join "target" "debug" "fast")
```

## Shell Integration

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
then changes the parent shell's directory after `q` confirms the highlighted
selection. `Esc` or `Ctrl-C` leaves the directory unchanged.

## Usage

<ul>
  <li>
    <strong>Cache directory:</strong> Set <code>FAST_CACHE_DIR</code> to override
    the platform cache directory.
  </li>
  <li>
    <strong>Move:</strong> Use <code>Up</code>/<code>Down</code> or
    <code>j</code>/<code>k</code> to move the selection.
  </li>
  <li>
    <strong>Jump:</strong> Use <code>Home</code>/<code>g</code> for the first
    entry or <code>End</code>/<code>G</code> for the last entry.
  </li>
  <li>
    <strong>Open:</strong> Press <code>Enter</code>/<code>Right</code> or
    <code>l</code> to open the selected directory.
  </li>
  <li>
    <strong>Parent:</strong> Press <code>Backspace</code>/<code>Left</code> or
    <code>h</code> to go to the parent directory.
  </li>
  <li>
    <strong>Rescan:</strong> Press <code>r</code> to scan the current directory
    again.
  </li>
  <li>
    <strong>Select:</strong> Press <code>q</code> to select the highlighted
    directory.
  </li>
  <li>
    <strong>Filter:</strong> Press <code>/</code> to enter filter mode. Typed
    text uses case-insensitive substring matching by default.
  </li>
  <li>
    <strong>Toggle filter:</strong> Press <code>Tab</code> in filter mode to
    switch between simple and fuzzy matching.
  </li>
  <li>
    <strong>Edit filter:</strong> Type to extend the query, use
    <code>Backspace</code> to edit it, and press <code>Enter</code> to keep the
    filter and return to navigation.
  </li>
  <li>
    <strong>Cancel:</strong> Press <code>Esc</code> to clear an active filter;
    press it again, or use <code>Ctrl-C</code>, to cancel without selecting a
    directory.
  </li>
</ul>

## License

Distributed under the [MulanPSL-2.0](http://license.coscl.org.cn/MulanPSL2) license. See [LICENSE](LICENSE) for details.

<details>
  <summary>Why MulanPSL-2.0?</summary>

The Mulan Permissive Software License v2 (MulanPSL-2.0) may be less familiar than more widely used licenses. To provide clarity and context, the following table (cited from [*Choose a License*](https://choosealicense.com/appendix/)) compares key aspects of MulanPSL v2 with popular licenses including *Apache-2.0*, *BSD-3-Clause*, and *MIT*.

| License          | Commercial Use | Distribution | Modification | Patent Use | Private Use | Disclose Source | License and Copyright Notice | Network Use is Distribution | Same License | State Changes | Liability | Trademark Use | Warranty |
|:----------------:|:--------------:|:------------:|:-------------:|:----------:|:-----------:|:---------------:|:----------------------------:|:---------------------------:|:------------:|:-------------:|:---------:|:-------------:|:--------:|
| Apache-2.0       | 🟢             | 🟢           | 🟢           | 🟢         | 🟢          |                 | 🔵                           |                             |              | 🔵            | 🔴        | 🔴            | 🔴       |
| BSD-3-Clause     | 🟢             | 🟢           | 🟢           |            | 🟢          |                 | 🔵                           |                             |              |               | 🔴        |               | 🔴       |
| MIT              | 🟢             | 🟢           | 🟢           |            | 🟢          |                 | 🔵                           |                             |              |               | 🔴        |               | 🔴       |
| MulanPSL-2.0     | 🟢             | 🟢           | 🟢           | 🟢         | 🟢          |                 | 🔵                           |                             |              |               | 🔴        | 🔴            | 🔴       |

The drafter of the MulanPSL-2.0 license addressed similar concerns in this [comment](https://github.com/originjs/vite-plugin-federation/issues/464#issuecomment-1774859600):

> Thank you for raising this issue. Please allow me to explain. (I'm the one responsible for drafting MulanPSL-2.0 and getting it approved by OSI.)
>
> Actually at the beginning we just say in the license, english and chinese version have the same legal effect (because we carefully translated the two versions word by word, sentence by sentence). However, the OSI community suggested that IN CASE, in case there is a conflict between the two languages, we should indicate which language prevails.
>
> However, I must say, there is a tiny chance (close to zero) that this circumstance will happen. On the one hand, many people (including technical experts and lawyers) did careful proofreading between english version and chinese version; on the other hand, MulanPSL-2.0 is such a loose license that really doesn't have constrains, what conflict will you expect? We worry about conflict because we worry about legal risk that may bring, but since the legal terms are so loose we hardly see a risk.
</details>
