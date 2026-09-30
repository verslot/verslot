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

## Git Commit Convention

All Git commit messages must follow Conventional Commits:

```text
<type>(<scope>): <description>
```

The scope is optional.

Allowed types:

- feat: new feature
- fix: bug fix
- refactor: code refactoring without behavior change
- perf: performance improvement
- test: tests
- docs: documentation
- build: build system or dependency changes
- ci: CI/CD changes
- chore: maintenance
- revert: revert a previous commit

Preferred scopes for this project:

- cli
- install
- uninstall
- inventory
- storage
- switching
- download
- checksum
- windows
- unix
- deps

Rules:

- Use lowercase type and scope.
- Use imperative mood for the description.
- Keep the subject concise and do not end it with a period.
- Prefer a project-domain scope instead of a filename.
- Use `!` or a `BREAKING CHANGE:` footer for breaking changes.
- Before committing, inspect the staged diff and choose the type and scope from the actual changes.
- When the user asks to commit without providing a commit message, generate a Conventional Commit message automatically.
- If the user explicitly provides a commit message, follow that message.
- Never use vague messages such as `update`, `changes`, `fix stuff`, or `misc`.

Examples:

- feat(cli): add current command
- feat(switching): implement version switching
- fix(windows): verify junction identity before cleanup
- test(switching): add selection workflow coverage
- docs: record M4 Windows acceptance
- build(deps): add winapi-util dependency
