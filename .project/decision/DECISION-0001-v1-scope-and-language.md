---
id: DECISION-0001
status: proposed
date: 2026-08-30
supersedes: none
review: none
---

# Decision: Initial Scope and Language

## Context

The project needs a standalone TUI for directory navigation on Linux, macOS,
and Windows. The first version should favor maintainability and correct
background work while remaining small enough to distribute as an executable.

## Option

1. Rust-first implementation with a small terminal layer, a directory-level
   persistent cache, and optional external fuzzy-finder backends.
2. C or C++ implementation with platform-specific terminal and filesystem
   adapters, trading lower possible binary size for more manual resource,
   Unicode, and concurrency handling.

## Decision

Use Rust as the proposed initial implementation language. Keep the first
version directory-only, single-column, keyboard-driven, shallow by default,
and responsive during chunked scans. Use bounded background prefetch for
direct child directories.

## Rationale

Rust provides stronger guarantees for cache parsing, background scan
cancellation, resource ownership, and cross-platform path handling. It does
not impose broot's recursive tree model or feature set. The binary and
dependency cost must still be measured rather than inferred from the language.

## Consequence

- The terminal backend and persistent storage backend remain separate choices.
- SQLite may be used through a bundled Rust binding, but a custom binary cache
  remains an allowed alternative.
- The initial cache stores visited directories; prefetch must be bounded and
  must not become an implicit recursive index.
- Human review is required before this proposed decision becomes authoritative.

## Affected Record Or Consumer

- `.project/roadmap/ROADMAP-0001.md`
- `.project/plan/PLAN-0001-phase-0-foundation.md`
- Future Phase 1 and Phase 2 implementation plans
