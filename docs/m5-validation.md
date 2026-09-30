# M5 Cross-Platform Validation

Last updated: 2026-10-01

## Status

M5 is **In progress (1 / 6 tasks)**. T1 Windows delivery validation is complete by candidate-CI evidence plus the documented M4-equivalence decision below. T2 macOS delivery validation is in progress: the candidate passed the existing macOS CI job, but the required native official-distribution workflow and complete environment record are still missing. T3 Linux validation has not started because the requested execution order is T1 → T2 → T3.

No package version, tag, binary publication, or persistent PATH state changed during this validation update.

## Candidate

| Field | Value |
| --- | --- |
| Candidate commit | `cf5760d930585cf31fd9de39d50e076cfd5b5270` (`v0.4.0`) |
| Candidate CI | [run 36698517158](https://github.com/verslot/verslot/actions/runs/36698517158), push workflow, completed successfully on 2026-09-30 |
| Local documentation state | `docs/v0.5.md`, this record, and roadmap updates are uncommitted; they do not change the executable |
| Package version | `0.4.0` |
| Publication status | No Verslot binaries published |

The candidate CI has separate successful `fmt`, `windows-latest`, `macos-latest`, and `ubuntu-latest` jobs. Each platform test job reports successful Check, Clippy, and Test steps. The public Actions API exposes the commit, job labels, step conclusions, and timestamps. Raw job-log download was unavailable because the local GitHub CLI credential is invalid, so exact CI test totals and the runner architecture cannot be recovered from that API result in this task. Those missing details are not inferred.

## T1 Windows delivery validation

**Complete.** The v0.5 design permits reuse of M4 Windows workflow evidence when executable equivalence is documented and the candidate checks are rerun.

### Candidate checks

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed locally on 2026-10-01 with no output |
| `cargo check --all-targets` | Candidate CI Windows Check step passed in [job 109832162033](https://github.com/verslot/verslot/actions/runs/36698517158/job/109832162033) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Candidate CI Windows Clippy step passed in the same job |
| `cargo test --all` | Candidate CI Windows Test step passed in the same job; exact count unavailable from the public job metadata |

The first local `cargo check` attempt failed before compilation because Windows Schannel could not acquire TLS credentials while updating the crates.io index. An approved outside-sandbox retry reached Schannel but failed because the certificate-revocation service was offline. An offline retry then confirmed that `winapi-util` was absent from the local registry index/cache. No TLS or certificate check was disabled. These are local dependency-resolution failures, not candidate compilation failures; the exact candidate's Windows CI Check step passed.

### M4-equivalence decision

[M4 validation](m4-validation.md) records the native Windows x86_64 / NTFS official Node.js 22.0.0 / 24.0.0 A/B workflow, exact checksums, direct-entry and controlled-PATH execution, no-op selection, competing PATH behavior, protected active uninstall, inactive uninstall, payload preservation, and absence of reserved residue.

The M4 acceptance record includes SHA-256 values for every source and test file. Comparing those recorded values with the candidate established:

- every production file under `src` has the same hash as the M4-validated source;
- `Cargo.toml` and the Verslot entry in `Cargo.lock` changed the package version from `0.3.0` to `0.4.0`; the Windows dependency used by M4 was already present in the validated manifest state;
- two test files changed after acceptance: Unix atomic-observation isolation and unique CLI fixture naming; both are test-only stabilization changes;
- the exact `v0.4.0` candidate subsequently passed the three-platform CI run above.

The executable behavior used by the Windows smoke workflow is therefore unchanged. M5 T1 reuses the retained M4 official-workflow logs instead of downloading and executing the same official binaries again. The candidate CI supplies the required current-commit Windows Check, Clippy, and Test evidence. The local crates.io TLS failures are retained as environment limitations and do not replace either source equivalence or CI evidence.

### Windows limitations retained

- Windows arm64 and non-NTFS filesystems remain unverified.
- Antivirus, endpoint-protection, and arbitrary loaded-image behavior remain environment-dependent.
- The candidate CI raw test count is unavailable in this task; M4's retained native acceptance count is 160 tests and applies to the recorded M4 source state.
- Windows junction publication retains the documented possible fixed-entry gap.

## T2 macOS delivery validation

**In progress.** Candidate CI [job 109832162310](https://github.com/verslot/verslot/actions/runs/36698517158/job/109832162310) completed its Check, Clippy, and Test steps successfully on `macos-latest` for the exact candidate commit.

This is not sufficient to complete T2:

- the existing CI workflow does not run the official Node.js A/B install/list/use/current/PATH/uninstall workflow;
- the public job metadata labels the runner only as `macos-latest`, so Apple-silicon architecture and filesystem details are not established;
- raw logs and exact test totals were unavailable with the current invalid GitHub CLI credential;
- this Windows host has no native macOS execution environment.

T2 requires a native macOS run that records OS version, Apple-silicon architecture, filesystem and case-sensitivity, Rust toolchain, exact test totals, official archive URLs/digests, command outputs, symlink identity/no-op behavior, fixed-entry and controlled-PATH execution, uninstall protection, payload preservation, and residue checks. Until that evidence exists, T2 remains In progress and the macOS roadmap checkbox remains unchecked.

The manual-only [M5 T2 macOS validation workflow](../.github/workflows/m5-t2-macos.yml) prepares this evidence on a `macos-15` runner, asserts `arm64`, uses an isolated HOME, runs all four required checks, performs the complete official 22.0.0 / 24.0.0 workflow, and uploads text evidence without archives or extracted installations. Adding the workflow is preparation only; its results must be recorded below before T2 is complete.

## T3 Linux delivery validation

**Not started.** The exact candidate's [ubuntu-latest job 109832162362](https://github.com/verslot/verslot/actions/runs/36698517158/job/109832162362) reports successful Check, Clippy, and Test steps. This is supporting evidence only; it does not include the official A/B workflow or the required GNU Linux environment/filesystem/permission record.

Per the requested order, T3 begins only after T2 is complete or the user explicitly changes the order.

## Progress log

| Date | Item | Result / next step |
| --- | --- | --- |
| 2026-10-01 | T1 Windows delivery validation | Complete through exact-candidate Windows CI plus verified production-source equivalence with retained M4 official A/B evidence; local fmt passed, local Cargo dependency resolution was blocked by Schannel/index availability without weakening TLS |
| 2026-10-01 | Start T2 macOS delivery validation | Exact-candidate macOS Check/Clippy/Test steps passed, but native Apple-silicon official workflow and full environment/log evidence are missing; provide a native macOS runner or an authorized way to run the candidate workflow before T3 |
| 2026-10-01 | Repair initial manual-workflow parse failure | The first merged workflow produced run 36776022008 with no jobs and rejected dispatch because multiline expected-output literals escaped the YAML block indentation. Replaced them with single-line Bash ANSI-C newline expressions; no product code or acceptance result changed |
