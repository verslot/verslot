# AGENTS.md

## Scope

Verslot is a version manager.

Do not add functionality outside version management unless explicitly requested.

Do not implement hypothetical future requirements.

## Rust

- Use stable Rust.
- Unsafe Rust is forbidden.
- Prefer the standard library.
- Avoid unnecessary dependencies.
- Avoid async unless an actual requirement needs it.
- Prefer straightforward, readable Rust.
- Do not use advanced abstractions when simple code works.

## Architecture

- Start with one crate.
- Do not split into a workspace unless current code requires it.
- Do not create abstractions solely for future extensibility.
- Keep platform-specific behavior isolated.

## Changes

- Make the smallest change necessary.
- Do not refactor unrelated code.
- Every behavior change requires tests.
- Every bug fix requires a regression test.

## Required validation

Before completing implementation:

cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all

## Safety

Pay special attention to:
- path traversal
- archive extraction
- symlink attacks
- checksum verification
- command injection
- atomic filesystem operations
- Windows file locking
- untrusted remote data

## Dependencies

When adding a dependency, explain:
1. Why it is needed.
2. Why the standard library is insufficient.
3. Why this crate was selected.
4. Whether a smaller alternative exists.
