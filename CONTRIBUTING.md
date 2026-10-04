# Contributing and releasing

This is the sole ongoing maintenance procedure after the frozen M1–M5 records.
Keep changes within version management, small and tied to an Issue. Do not add
speculative features, daily progress documents, M6/M7 plans, templates, boards
or extra automation.

## Issues

[GitHub Issues](https://github.com/verslot/verslot/issues) are the only backlog.
Bugs should include Verslot version, OS/architecture, relevant filesystem,
reproduction commands, expected and actual results, and diagnostics with secrets
removed. Feature requests should state the use case, scope and acceptance
criteria. Search existing Issues before opening a duplicate.

Precompiled binaries are a future enhancement tracked by an Issue, with no
promised release date. M1's independent historical acceptance is unrecorded;
we do not retroactively accept it or require reconciliation for current releases.

## Development and pull requests

Use a `codex/` branch for one small issue. Use stable Rust, one crate, no unsafe
Rust, and prefer the standard library. Preserve unrelated working-tree changes.
Bug fixes require regression tests; behavior changes require corresponding tests.
Explain any dependency addition and why smaller alternatives are insufficient.

Link the Issue in the PR (for example, `Closes #123`), describe resulting behavior,
and record actual validation and any limitations. Update README when usage changes;
put user-visible changes under CHANGELOG's `Unreleased`. Use Conventional Commits:
`<type>(<scope>): <imperative description>`, with lowercase type/scope and a concise
subject. Inspect the staged diff before committing.

Pure documentation changes need no Cargo checks. Code changes and every version
release must pass all four checks before completion:

```text
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

The final release commit must also pass the existing Windows/macOS/Linux CI
matrix. Record the exact SHA and run links in the Release. Label checks not run
honestly; retained M5 acceptance is historical evidence, not new execution.

## Versions

Compatible fixes use `0.5.x`. Features or incompatible changes use the next minor
version; document migration for incompatible changes. Move released Unreleased
entries into a dated version section. Update Cargo.toml and only Verslot's own
Cargo.lock version; do not upgrade dependencies as part of a version bump.
An Issue may close when its fix merges; CHANGELOG and Release separately identify
the first version containing it. Publish when needed, without a fixed schedule.

## Release procedure

1. Review the final diff for scope, required accepted fixes and preserved history.
   Update the version, CHANGELOG and README's fixed tag installation command.
2. Run all four checks, commit with Conventional Commits and open a PR to main.
   Merge through the PR, then confirm the final main commit's three-platform CI
   and formatting job all pass. Record its SHA and run URL; do not substitute
   earlier candidate runs for the final commit.
3. Install from a clean checkout of that exact commit using
   `cargo install --path <checkout> --locked --root <temporary-install-directory>`.
   Invoke its bin/verslot (bin/verslot.exe on Windows) by full path; verify the
   version and help. Do not overwrite your everyday executable.
4. Check that the intended local and remote tag do not exist. Create the tag from
   the verified final commit and push it; never move or overwrite an existing tag.
5. Create a GitHub Release from CHANGELOG, labelled as a source release. Include
   fixed-tag installation, changes, supported targets, limitations, final SHA/CI
   and historical M5 evidence links. Do not attach precompiled binaries or publish
   to crates.io for v0.5.0.
6. Verify remote tag SHA, Release URL and README installation tag agree. Failed
   candidates must be repaired and revalidated before publication. If remote
   permissions block delivery, retain the local commit and complete Release body,
   and report the unfinished steps explicitly.

M1–M5 roadmap, designs and acceptance records stay frozen; new work belongs in
Issues. Do not rewrite historical versions or conclusions as release evidence.
