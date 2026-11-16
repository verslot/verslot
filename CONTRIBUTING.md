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

Precompiled binary distribution is tracked by [#12](https://github.com/verslot/verslot/issues/12).
M1's independent historical acceptance is unrecorded; we do not retroactively
accept it or require reconciliation for current releases.

## Development and pull requests

Use a `codex/` branch for one small issue. Use stable Rust, one crate, no unsafe
Rust, and prefer the standard library. Preserve unrelated working-tree changes.
Bug fixes require regression tests; behavior changes require corresponding tests.
Explain any dependency addition and why smaller alternatives are insufficient.

Development pull requests target `dev`, which also accepts direct pushes.
`main` accepts changes only through pull requests. When ready to release, open
a pull request from `dev` to `main`. CI runs on pull requests targeting either
branch and on pushes to either branch, including merges. Wait for the CI checks
to pass before merging a pull request.

Link the Issue in the PR (for example, `Closes #123`), describe resulting behavior,
and record actual validation and any limitations. Update README when usage changes.
Describe user-visible changes in Conventional Commits so release-plz can include
them in the changelog. The commit format is
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

Use release-plz to determine the next version from commits since the latest
version tag. On the current 0.x series, `fix` and `perf` increment the patch
version, and `feat` and breaking changes increment the minor version. Other
changes are included when a release is prepared for an eligible commit.
An Issue may close when its fix merges; CHANGELOG and Release separately identify
the first version containing it. Publish when needed, without a fixed schedule.

## Release procedure

1. When ready to release, pause unrelated changes to `dev`, then open
   **Actions → Prepare release patch → Run workflow** and select `dev`.
   The workflow runs release-plz and shows the source SHA,
   proposed version and changelog diff in the run summary. If there are no
   release changes, stop. Otherwise, download `release-patch` from Artifacts
   and extract `release.patch`. On a clean, up-to-date `dev` checkout, confirm
   that `git rev-parse HEAD` matches the source SHA in the summary. If `dev`
   has advanced, run the workflow again for its latest commit. Check the patch
   with `git apply --check <path-to-release.patch>`, then apply it with
   `git apply <path-to-release.patch>`. Review `Cargo.toml`, `Cargo.lock` and
   the new version section in `CHANGELOG.md`. Check that locked dependency
   versions are still pinned and add migration instructions for any incompatible
   changes.
   Record the first binary release in CHANGELOG and update README's pending-release
   wording when publishing it.
2. Run all four checks, commit the three files as `chore(release): prepare vX.Y.Z`,
   push to `dev`, and open a PR from `dev` to `main`. If new changes enter `dev`
   while the PR is open, revert the release preparation commit and push the
   revert to `dev`. Repeat step 1 and push the new preparation commit to update
   the PR. Wait for CI on the latest PR head and use a merge commit to preserve
   the `dev` history.
   Confirm the final `main` commit's three-platform CI and formatting job pass.
   Record its SHA and run URL.
3. Install from a clean checkout of that exact commit using
   `cargo install --path <checkout> --locked --root <temporary-install-directory>`.
   Invoke its bin/verslot (bin/verslot.exe on Windows) by full path; verify the
   version and help. Do not overwrite your everyday executable.
4. Check that the intended local and remote `vX.Y.Z` tag do not exist. Create it
   from the verified final commit and push it; never move or overwrite a tag.
   The push starts `Release build`, which checks the tag against Cargo's version
   and builds Windows x86-64 MSVC, macOS Apple Silicon and Linux x86-64 GNU.
   Merge `main` back into `dev` to include the tagged commit in the development
   history.
5. Confirm all three jobs pass on the tagged SHA. Each verifies its archive,
   checksum, extracted executable and shim before uploading. After all pass, the
   workflow verifies the matching main CI run and creates a Release draft with
   the three archives and three `.sha256` files. The draft notes include the
   tagged version's `CHANGELOG.md` section, SHA, CI and build links, runner
   versions and runtime dependencies, including Linux glibc symbol versions.
6. Download the three artifacts from that run. On each target, use an environment
   without Rust or native build tools. Use the draft's installation instructions
   to verify, extract and set up the downloaded files. Confirm `--version`
   matches the Cargo version, `--help` succeeds and the new shim runs. Record
   the tested OS versions and runtime requirements in the draft; do not claim
   unverified minimum versions.
7. Review the six attachments, notes and historical M5 evidence links, then
   publish the draft. Download the public assets and verify their filenames and
   SHA-256 digests. Verify the remote tag SHA against the final commit. Link the
   first binary Release to #12 before closing it.

For a transient failure, re-run the failed jobs on the existing tag. A partial
draft can be completed only if its existing assets and build evidence match the
artifacts from that run; inspect any mismatch before retrying. Do not publish
while the workflow is running or alter a published Release. If a fix changes the
commit, use a new version and tag rather than moving the existing tag.

M1–M5 roadmap, designs and acceptance records stay frozen; new work belongs in
Issues. Do not rewrite historical versions or conclusions as release evidence.
