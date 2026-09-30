# M5 Cross-Platform Validation

Last updated: 2026-10-01

## Status

M5 is **In progress (2 / 6 tasks)**. T1 Windows delivery validation is complete by candidate-CI evidence plus the documented M4-equivalence decision below. T2 macOS delivery validation is complete through native Apple-silicon checks and the official two-version workflow. T3 Linux validation is next under the requested T1 → T2 → T3 order.

No package version, tag, binary publication, or persistent PATH state changed during this validation update.

## Candidate

| Field | Value |
| --- | --- |
| Product candidate commit | `cf5760d930585cf31fd9de39d50e076cfd5b5270` (`v0.4.0`) |
| macOS validation commit | `7dc4272b1009d11d421c53f865c1f48ba4a4fcb4`; differs from the product candidate only by M5 workflow and documentation commits |
| Candidate CI | [run 36698517158](https://github.com/verslot/verslot/actions/runs/36698517158), push workflow, completed successfully on 2026-09-30 |
| Documentation state | This record, `docs/v0.5.md`, and the roadmap are synchronized after T2; they do not change the executable |
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

**Complete.** [Manual run 36777691817](https://github.com/verslot/verslot/actions/runs/36777691817) validated merge commit `7dc4272b1009d11d421c53f865c1f48ba4a4fcb4` on a GitHub-hosted `macos-15-arm64` runner. That commit adds only the manual validation workflow and M5 documentation to the unchanged `v0.4.0` product source.

### Native environment and checks

| Evidence | Result |
| --- | --- |
| Runner | macOS 15.7.9 (24G830), image `macos15` version `20260907.0337.1`, `arm64` |
| Filesystem | Runner temporary directory on `/System/Volumes/Data`; root reports APFS; direct filename probe records `case_sensitive=false` |
| Rust | `rustc 1.98.1`, host `aarch64-apple-darwin`; Cargo 1.98.1; Clippy 0.1.98 |
| `cargo fmt --check` | Passed |
| `cargo check --all-targets` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all` | Passed: 136 library + 19 CLI + 8 selection CLI tests, 163 total; 0 failed or ignored |
| Build | `cargo build --bin verslot` passed |
| Artifact | `m5-t2-macos-arm64-36777691817-1`, artifact ID `11126770830`, uploaded ZIP SHA-256 `dcc0dd8f6539d27bc7a209488423adb91e204cad959a01c7b433133b51b5ebb2`, retained by GitHub Actions for 30 days |

The run's only annotation is GitHub's notice that macOS arm64 capacity can cause longer queue times. The updated workflow uses `actions/checkout@v5` and `actions/upload-artifact@v6`; the earlier Node 20 deprecation warnings are absent.

### Official two-version workflow

- Empty `list` and `current` were read-only and did not create the isolated storage root.
- Official Node.js `22.0.0` and `24.0.0` `darwin-arm64.tar.gz` archives installed successfully. The exact official SHA-256 values were `ea96d349cfaa67aa87ceeaa3e5b52c9167f7ac302fd8d1ff162d0785e9dc0785` and `194e2f3dd3ec8c2adcaa713ed40f44c5ca38467880e160974ceac1659be60121`.
- Duplicate installation returned `already installed`; numeric listing returned 22.0.0 before 24.0.0.
- Selecting each version made `current`, the fixed entry, and a controlled PATH execute the expected version. Re-selecting 24.0.0 returned `already using` and preserved the link inode.
- A deliberately competing PATH resolved Node 22.0.0 while Verslot still reported 24.0.0, confirming the documented PATH precedence limitation; restoring the controlled PATH executed 24.0.0.
- Receipt and executable hashes for both installations were unchanged after switching and the no-op selection.
- Uninstalling the active 24.0.0 version failed with the expected protection error. Uninstalling inactive 22.0.0 succeeded; 24.0.0 remained selected and executable with unchanged bytes.
- The final state contained no removed A installation, no reserved switch siblings, and an empty temporary directory. The smoke record ended with `PASS`.

### Repairs and retained limitations

The first workflow revision failed to parse because multiline expected-output literals escaped YAML block indentation. The repaired workflow then passed in [run 36776678819](https://github.com/verslot/verslot/actions/runs/36776678819), but its filesystem filter captured no filesystem or case-sensitivity fields, so it was not accepted as complete. PR [#8](https://github.com/verslot/verslot/pull/8) added raw filesystem output, a direct case probe, and Node 24 Action versions. Its CI passed after one Ubuntu rerun for a transient lock-release timing failure, and the final native rerun supplied the missing evidence without changing product code.

macOS x86_64, case-sensitive APFS, other macOS versions, non-APFS filesystems, persistent shell-profile changes, and hostile same-user races remain unverified. The non-UTF-8 on-disk fixture remains excluded on macOS as specified; in-memory native-byte coverage is supporting evidence only.

## T3 Linux delivery validation

**Not started.** The exact candidate's [ubuntu-latest job 109832162362](https://github.com/verslot/verslot/actions/runs/36698517158/job/109832162362) reports successful Check, Clippy, and Test steps. This is supporting evidence only; it does not include the official A/B workflow or the required GNU Linux environment/filesystem/permission record.

Per the requested order, T3 is now the next development task.

## Progress log

| Date | Item | Result / next step |
| --- | --- | --- |
| 2026-10-01 | T1 Windows delivery validation | Complete through exact-candidate Windows CI plus verified production-source equivalence with retained M4 official A/B evidence; local fmt passed, local Cargo dependency resolution was blocked by Schannel/index availability without weakening TLS |
| 2026-10-01 | Start T2 macOS delivery validation | Exact-candidate macOS Check/Clippy/Test steps passed, but native Apple-silicon official workflow and full environment/log evidence are missing; provide a native macOS runner or an authorized way to run the candidate workflow before T3 |
| 2026-10-01 | Repair initial manual-workflow parse failure | The first merged workflow produced run 36776022008 with no jobs and rejected dispatch because multiline expected-output literals escaped the YAML block indentation. Replaced them with single-line Bash ANSI-C newline expressions; no product code or acceptance result changed |
| 2026-10-01 | Run repaired T2 workflow | [Run 36776678819](https://github.com/verslot/verslot/actions/runs/36776678819) passed arm64 assertion, all four checks, 163 tests, build, official 22.0.0 / 24.0.0 workflow and evidence upload. Evidence review found the filesystem filter recorded no filesystem/case-sensitivity fields, so T2 remains In progress pending a focused rerun; update deprecated Node 20 actions during that evidence repair |
| 2026-10-01 | Complete T2 macOS delivery validation | PR [#8](https://github.com/verslot/verslot/pull/8) repaired filesystem/case evidence and updated Actions. [Run 36777691817](https://github.com/verslot/verslot/actions/runs/36777691817) passed on macOS 15.7.9 arm64 / APFS: all four checks, 163 tests, build, official Node.js 22.0.0 / 24.0.0 workflow, payload/residue assertions and evidence upload. T2 Complete; begin T3 Linux validation next |
