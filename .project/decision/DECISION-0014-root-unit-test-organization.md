---
id: DECISION-0014
status: accepted
date: 2026-11-10
supersedes: none
review: .project/review/REVIEW-0015-tui-redraw-source-organization.md
---

# Decision: Root Unit Test Organization

## Context

The source organization change moves existing unit-test bodies out of the Rust
source files. Their locations need a consistent long-term convention without
turning tests of private module behavior into integration tests.

## Options

1. Keep unit-test bodies inline in each source file. Private access is natural,
   but source files grow together with their tests.
2. Store unit-test bodies under a root `test/` directory and include them as
   test-only modules of the corresponding source modules. This separates the
   files while retaining Rust unit-test visibility.
3. Move them into Cargo's `tests/` integration-test directory. This separates
   the files but changes test boundaries and access to private items.

## Decision

Use option 2. The human confirmed root `test/`, alongside `src/`, as the
long-term location for future unit tests. Group tests by their owning source
module and include them as test-only modules, using an appropriate relative
`#[path = "..."]` where needed. The existing six unit-test modules follow this
layout.

## Rationale

Separating test bodies makes implementation files easier to navigate while
preserving private-item access and the existing unit-test model.

## Consequence

- Future unit tests belong under root `test/`; their source modules remain
  responsible for including them for test builds.
- Root `test/` is distinct from Cargo's `tests/` integration-test directory;
  this decision does not establish a convention for integration tests.
- The existing layout was accepted with the implementation in `REVIEW-0015`.

## Affected Record Or Consumer

- `.project/review/REVIEW-0015-tui-redraw-source-organization.md`
- `src/`
- `test/`
