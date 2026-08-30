---
id: ROADMAP-0001
status: proposed
project: fast
supersedes: none
review: none
---

# Roadmap: fast

## Outcome

Provide a small standalone TUI that lets a user browse directories and change
the parent shell's working directory without recursively scanning the file
system by default.

## Success Criterion

- A cache miss can render the first available directory entries before the
  complete scan finishes and shows that indexing is still in progress.
- A valid cache can be displayed after a directory fingerprint check without a
  complete directory read.
- A changed or invalid cache is rebuilt atomically without exposing partial
  index data as complete.
- Release artifacts target Linux x86_64/aarch64, macOS arm64, and Windows
  x86_64.

## Phase

### Phase 0: Foundation and Contract

- Objective: Establish the repository governance, formatting, scope, and
  review workflow.
- Gate: Human initializes the repository with `main`, reviews the candidate
  tree, and commits the approved foundation.
- Dependency: none

### Phase 1: Minimal Navigator

- Objective: Implement a directory-only, single-column TUI with chunked
  scanning, cancellation, and an indexing indicator.
- Gate: The first screen is available during a cold scan and the UI remains
  responsive while the scan continues.
- Dependency: phase-0-foundation

### Phase 2: Persistent Directory Cache

- Objective: Add a versioned, crash-safe cache for visited directories with
  fingerprint validation and bounded storage.
- Gate: Cache hits, invalidation, corruption, concurrent writers, and scan
  races have automated coverage.
- Dependency: phase-1-minimal-navigator

### Phase 3: Prefetch and Shell Integration

- Objective: Add bounded child-directory prefetch, Nushell/Bash/Zsh wrappers,
  optional fuzzy-finder backends, and release packaging.
- Gate: Shell navigation and platform release checks pass without a mandatory
  external database or CLI.
- Dependency: phase-2-persistent-directory-cache

## Assumption

- Rust is the proposed initial implementation language.
- The initial browser lists directories only and does not support mouse input.
- The default scan is shallow; prefetch is bounded and cancellable.
- The initial supported character set is UTF-8 with core cross-platform path
  and link behavior only.
- SQLite remains an implementation option; if selected, it must be bundled so
  users do not need a system SQLite installation or the `sqlite3` CLI.
- No fixed latency or binary-size target has been approved yet; both will be
  measured before optimization decisions are finalized.
