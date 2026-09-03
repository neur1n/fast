---
id: DECISION-0005
status: accepted
date: 2026-09-03
supersedes: none
review: .project/review/REVIEW-0005-version-reporting-and-release-injection.md
---

# Decision: Version Reporting and Release Injection

## Context

The binary previously had no version option. `Cargo.toml` contains the package
version, while the manual release workflow accepts a release tag separately.
Without an explicit build-time connection, release and nightly artifacts would
all report the package version instead of their actual artifact version.

Release tags should also use one format consistently. Stable releases,
prereleases, and nightly builds need distinguishable versions without relying
on a leading `v`.

## Options

1. Report only `CARGO_PKG_VERSION` and leave release tags independent.
2. Rewrite `Cargo.toml` for every release and use it as the only version source.
3. Use `Cargo.toml` as the local fallback and inject an explicit version into
   release builds through a compile-time environment variable.

## Decision

Use option 3. The application reports `fast <version>` for `--version` and
`-V`. It reads `FAST_BUILD_VERSION` at compile time when that variable is set,
and otherwise falls back to `CARGO_PKG_VERSION`.

The manual release workflow accepts tags in the form
`MAJOR.MINOR.PATCH[-PRERELEASE]`, without a leading `v`, and sets
`FAST_BUILD_VERSION` to the exact input. A tag without a prerelease suffix
creates a regular GitHub Release; a tag containing a `-` prerelease suffix
creates a GitHub prerelease.

Nightly builds are not created by the manual release workflow. A nightly build
entry point should explicitly inject a unique version such as
`0.0.3-nightly.20260903.123`, using a UTC date and a CI run identifier.

The shell wrappers preserve their no-argument navigation behavior. When any
argument is supplied, they forward all arguments directly to the binary so
that `fast --version` and other CLI options remain available through shell
integration.

## Rationale

Compile-time injection keeps the displayed version tied to the artifact and
does not depend on runtime environment variables, Git metadata, or a complete
checkout. Keeping the Cargo package version as a fallback makes local builds
and ordinary `cargo run` invocations useful without additional configuration.

Using the same no-`v` SemVer-like format for tags and displayed versions avoids
translation rules in archive names and release automation. The workflow's
prerelease decision follows the version suffix rather than forcing every
manual release to be a prerelease.

## Consequence

- Local builds report the version in `Cargo.toml` unless `FAST_BUILD_VERSION` is
  explicitly set before compilation.
- Release and prerelease artifacts report the manually supplied workflow input.
- A nightly workflow or script must generate and inject its own unique version.
- Runtime changes to `FAST_BUILD_VERSION` after compilation have no effect.
- Existing no-argument shell navigation remains unchanged.

## Affected Record Or Consumer

- `src/cli.rs`
- `src/main.rs`
- `shell/fast.bash`
- `shell/fast.zsh`
- `shell/fast.nu`
- `Cargo.toml`
- `Cargo.lock`
- `.github/workflows/manual-release.yml`
- `README.md`
