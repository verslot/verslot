# M2 validation: Windows

Date: 2026-09-22. T1–T6 deliverables were complete before these checks ran.

## Environment

- Host: `x86_64-pc-windows-msvc`.
- Compiler: `rustc 1.98.1 (48a229cea 2026-09-01)`; LLVM 22.1.8.
- Scope: the local working tree, including the uncommitted M2 implementation.
- Version at execution: `0.1.0`, before the package metadata was bumped to `0.2.0`. Checks were not rerun for the subsequent version/documentation-only update; the results below describe the original M2 validation run.
- Filesystem fixtures: temporary directories; Windows junction tests executed successfully.

## Results

| Command | Exit code | Result |
| --- | --- | --- |
| `cargo fmt --check` (initial) | 1 | Formatting differences in two parser unit-test blocks in `src/target.rs` |
| `rustfmt --edition 2024 src/target.rs` | 0 | Applied formatting only; no behavior change |
| `cargo fmt --check` (after repair) | 0 | Passed, no output |
| `cargo check --all-targets` | 0 | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Passed, no warnings |
| `cargo test --all` | 0 | Passed: 27 library tests and 17 CLI integration tests |

The initial formatting failure reopened the T2 deliverable for formatting repair. It is complete again after the successful format check. Compilation passed before the formatting-only repair; Clippy and tests passed after it.

Relevant output from the successful runs:

```text
cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.10s

cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

cargo test --all
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.83s

Library tests:
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

Binary tests:
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

CLI integration tests:
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s

Doc tests:
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The executed tests cover shared parser diagnostics, all three target commands, help and argument constraints, native storage roots, junction boundaries, missing and invalid current state, link loops, and preservation of existing contents. See the [acceptance mapping](v0.2.md#acceptance-mapping) for test names.

## Remaining platform evidence

At the original local acceptance on 2026-09-22, Linux and macOS checks had not run. WSL is not installed on this host; no macOS runtime is available in this task. The subsequent PR CI failure and repair are recorded below.

The following Unix-only tests were not compiled or executed on Windows:

- `unix_root_preserves_native_bytes_and_whitespace`
- `relative_current_links_are_resolved_against_the_link_parent`
- `non_utf8_current_version_names_are_rejected`
- `permission_failures_are_not_treated_as_missing_state`

Shared filesystem tests also still need execution with Unix symlinks. Windows ACL-denial behavior has no dedicated fixture. Concurrent filesystem mutation safety remains outside M2, as documented in the specification.

Windows local acceptance has passed. M2 is **Complete**. Linux/macOS validation remains incomplete and is recorded as a limitation, not a prerequisite for M2 acceptance. This corrects the earlier interpretation that all platform results were required to close M2. Full cross-platform delivery validation belongs to roadmap M5; M2 completion is not a release or a claim that installation and switching are implemented.

## PR CI repair: 2026-09-26

[PR #1](https://github.com/verslot/verslot/pull/1) was closed without merging after [CI run 35751483466](https://github.com/verslot/verslot/actions/runs/35751483466) failed for head commit `88a9d150e6d7b6632caa87a7815fea32a9dcbe7d`.

- Formatting and the Windows job passed.
- Linux and macOS passed `cargo check --all-targets`, then failed Clippy with `non_octal_unix_permissions` at `src/storage/tests.rs:504`. Their test steps were skipped, not passed.
- The Unix-only regression test `permission_failures_are_not_treated_as_missing_state` used `fs::Permissions::from_mode(0)`. Changed the literal to `0o0`, preserving the same permission bits and existing assertions. No production behavior or dependencies changed; the existing test and Unix CI Clippy jobs cover this repair.
- Windows excludes this test through `#[cfg(unix)]`, so the earlier local validation could not catch the diagnostic.

Local validation was rerun on Windows against package version `0.2.0` after the repair:

| Command | Exit code | Result |
| --- | --- | --- |
| `cargo fmt --check` | 0 | Passed |
| `cargo check --all-targets` | 0 | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Passed |
| `cargo test --all` | 0 | Passed: 27 library tests and 17 CLI integration tests; 0 failures, 0 ignored |

Linux/macOS validation of the repaired code remains pending the replacement PR's CI run. The local Windows results do not establish that the Unix-only test compiles or passes.
