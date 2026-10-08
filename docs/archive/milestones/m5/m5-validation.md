# M5 Cross-Platform Validation

Last updated: 2026-10-04

## Status

M5 is **Complete, release scope confirmed (6 / 6 tasks)**. The repaired final source is covered by Windows local checks plus source-equivalent M4 smoke reuse, retained Linux T3 acceptance, and macOS run 37205835491 reviewed below. Package version remains `0.4.0`; no new tag or binary publication is implied.

No package version, tag, binary publication, or persistent PATH state changed during this validation update.

## Candidate

| Field | Value |
| --- | --- |
| Original product candidate commit | `cf5760d930585cf31fd9de39d50e076cfd5b5270` (`v0.4.0`) |
| macOS validation commit | `7dc4272b1009d11d421c53f865c1f48ba4a4fcb4`; differs from the product candidate only by M5 workflow and documentation commits |
| Linux validation commit | `04c17a42b6f8099e0ef0ba13951abdd4511b201a`; includes the T3 workflow and the Linux lock-release repair described below |
| Candidate CI | [run 36698517158](https://github.com/verslot/verslot/actions/runs/36698517158), push workflow, completed successfully on 2026-09-30 |
| Documentation state | This record, `docs/archive/milestones/m5/v0.5.md`, and the roadmap are synchronized after T6 |
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

[M4 validation](../m4/m4-validation.md) records the native Windows x86_64 / NTFS official Node.js 22.0.0 / 24.0.0 A/B workflow, exact checksums, direct-entry and controlled-PATH execution, no-op selection, competing PATH behavior, protected active uninstall, inactive uninstall, payload preservation, and absence of reserved residue.

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

**Complete.** [Run 36843186140](https://github.com/verslot/verslot/actions/runs/36843186140) validated commit `04c17a42b6f8099e0ef0ba13951abdd4511b201a` on a GitHub-hosted `ubuntu-24.04` runner. The workflow asserted x86_64, a non-root user, and GNU libc, and recorded the distribution, kernel, libc, filesystem, mount options, virtualization, Rust toolchain, and clean checkout in its retained evidence.

| Evidence | Result |
| --- | --- |
| Required checks | `cargo fmt --check`, `cargo check --all-targets --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo test --all --locked` passed |
| Tests | 137 library + 19 CLI + 8 selection CLI tests, 164 total; 0 failed or ignored |
| Build | `cargo build --locked` passed |
| Official workflow | Node.js 22.0.0 and 24.0.0 install/list/use/current/direct-entry/controlled-PATH/no-op/protected-uninstall/inactive-uninstall workflow passed |
| Official Linux x64 SHA-256 | `74bb0f3a80307c529421c3ed84517b8f543867709f41e53cd73df99e6442af4d` (22.0.0) and `b760ed6de40c35a25eb011b3cf5943d35d7a76f0c8c331d5a801e10925826cb3` (24.0.0) |
| Artifact | `m5-t3-linux-x64-gnu-36843186140-1`, artifact ID `11152935391`, uploaded ZIP SHA-256 `01c3c8a70891bdbc67fce6ff3f6da945896dc6c3a36290d8b19e99b1ae4f85ca`, retained for 30 days |

The first two native runs exposed that successful Unix no-op and switching paths could return before a following operation observed the file lock as released. Commits `4a25760` and `04c17a4` explicitly unlock before both successful returns; the existing `relative_selection_is_untouched_on_noop_and_supported_on_switch` regression then passed in the final run. A separate push-trigger repair supplied the manual input defaults during validation. The temporary push trigger was removed after acceptance; the workflow remains manually dispatchable.

The final smoke confirmed executable permission, numeric list order, fixed-entry and controlled-PATH execution for both versions, inode preservation on no-op selection, documented competing-PATH precedence, unchanged receipt/executable hashes after switching, active-version uninstall protection, removal of only the inactive version, an empty operation directory, and no reserved switch siblings. Linux GNU arm64, non-GitHub-hosted distributions, other filesystems/mount policies, persistent shell-profile changes, and hostile same-user races remain unverified.

## T4 security review

**Complete.** The main review covered tree `6f42688`. T3 subsequently changed only Unix successful-return lock release in `src/storage/switching.rs`; a focused follow-up reviewed that diff and its existing sequential no-op/switch/query regression. It shortens lock ownership without weakening validation, rollback, or exclusive mutation, and introduces no dependency. No confirmed vulnerability or unresolved release-blocking finding was found.

| Area | Reviewed implementation and regression evidence | Finding and disposition | Residual limitation |
| --- | --- | --- | --- |
| Native roots and boundaries | `src/storage.rs`, `src/installation.rs`, `src/mutation.rs`, `src/install.rs`, `src/uninstall.rs`, `src/inventory.rs`; storage boundary/link tests, linked mutation-path tests, cleanup boundary tests | No finding. Roots are resolved to a canonical boundary before mutation; descendants are inspected component by component with `symlink_metadata`; non-direct operation cleanup and linked/non-directory ancestors fail closed | A linked storage root is intentionally supported. Hostile same-user path replacement races are outside the documented threat model |
| Archive extraction | `src/installation.rs`, `src/installation/zip.rs`, `src/installation/tar.rs`; unsafe-name, duplicate/conflict, byte/entry, ZIP64, CRC/truncation, tar metadata, special-entry, link escape/cycle and corruption tests | No finding. Extraction requires one expected top-level directory, rejects traversal/reserved receipt paths/unsupported types, bounds compressed download, expanded bytes, entries and metadata, and creates links only after resolving their in-archive targets | Correctly verified official payload code remains trusted executable content. Linux GNU arm64 and other host/filesystem combinations remain unverified |
| Checksums and transport | `src/distribution.rs`, `src/download.rs`; exact/duplicate checksum, SHA-256 vector, mismatch/truncation, HTTPS/certificate, redirect, status, size and timeout tests | No finding. URLs are fixed official HTTPS paths derived from a parsed full version; certificate verification remains enabled; redirects and content decoding are disabled; response sizes and time are bounded; checksum selection is exact and unique; extraction starts only after SHA-256 matches | Compromised official infrastructure and local trust-store/proxy policy are outside scope; custom mirrors and proxies are unsupported |
| Complete installations | `src/installation.rs`, `src/inventory.rs`, selection/uninstall callers; receipt, executable, incomplete inventory/current and malformed destination tests | No finding. Visibility requires a bounded exact receipt matching target/platform/digest, a real canonical direct directory and a nonempty regular executable; Unix also requires an execute bit | The receipt records verified installation provenance; it is not a continuing integrity monitor against later same-user payload modification |
| Mutations and cleanup | `src/mutation.rs`, `src/install.rs`, `src/uninstall.rs`; process contention, exclusive allocation, rename, partial cleanup, link-preservation, selected-version and residue tests | No finding. Writers use one nonblocking exclusive lock; complete queries use a shared lock; operations stay on the storage filesystem; cleanup is restricted to direct owned operation paths and never recursively follows links; selected or invalid state fails closed | Power loss, hostile same-user races and automatic recovery are not promised. Reported detached or temporary leftovers can require manual recovery |
| Switching | `src/storage/links.rs`, Unix and Windows switching modules; atomic observation, identity replacement, rollback, sharing, residue and unexpected-entry tests | No finding. Candidate and backup identity includes native link identity and destination; every destructive step revalidates state; Unix publication uses atomic rename; Windows holds the lock across its documented gap; rollback and cleanup failures retain evidence and fail closed | Windows publication is not gapless. Crash/power-loss recovery and malicious same-user races remain outside scope |
| Untrusted data and dependencies | Distribution/download/archive modules, `Cargo.toml`, `Cargo.lock`; parser and archive regressions; RustSec scan below | No finding. Server-controlled text, headers and archive metadata do not reach shell construction and remain under explicit parsing/size/type limits. No runtime dependency was added during review | Advisory absence does not prove vulnerability absence; the locked tree must be rescanned for a later release candidate |
| Command execution and evidence | `src` production search plus `.github/workflows/m5-t2-macos.yml` and `.github/workflows/m5-t3-linux.yml` | No finding. Production code does not spawn commands or build shell strings. Smoke inputs must match full numeric versions; paths are quoted; `run_expect` executes argument arrays; HOME is isolated; PATH changes are child-process-only; logs contain no credentials or archives | The workflow intentionally executes verified official Node.js binaries on disposable hosted runners |

### Dependency advisory evidence

- Tool: `cargo-audit 0.22.2`, installed under ignored `target/security-tools`; it is not a project dependency.
- Database: [RustSec Advisory Database](https://github.com/RustSec/advisory-db), commit `9b3a3b73a7f42606494c943e95f8196e9994df46`, committed 2026-09-30; 1,277 advisories loaded.
- Input: committed `Cargo.lock`, 99 locked dependencies scanned on 2026-10-01.
- Result: no vulnerabilities and no warnings reported. The previously published `rustls-webpki` CRL panic is fixed from `0.103.13`; the project locks `0.103.15`. The historical `ring` unmaintained notice is withdrawn.
- Limitation: this is a point-in-time database result, not a guarantee about undisclosed vulnerabilities or future advisories.

## T5 installation, usage, and limitations documentation

**Complete.** [README](../../../../README.md) now documents installation pinned to
`04c17a42b6f8099e0ef0ba13951abdd4511b201a`, native build prerequisites,
Cargo binary location, all five commands, full-version syntax, storage/fixed
entry paths, and manual per-shell PATH configuration. It separates native
validated platforms from mapped-but-unverified and unsupported targets.

Filesystem/permission, macOS fixture, Windows antivirus/sharing, network/proxy,
checksum/trust, PATH precedence, uninstall protection, publication-gap and
manual recovery limitations are tied to retained evidence. The pinned source
includes the T3 Unix lock-release repair reviewed in T4; older platform runs
are not a new combined acceptance run for that source. T6 owns that assessment.

Documentation only: no production code, dependency, version, tag, binary
publication or persistent PATH change. Checks, tests, builds and official
workflows **Not run** for T5; previous results remain historical evidence.

## T6 final evidence matrix and release decision

**Initial T6 decision: Awaiting validation; superseded by final acceptance below.** This decision is based
on retained repository evidence and local Git source comparisons on 2026-10-04.
No CI, checks, tests, builds, advisory scan or official workflow was run during
T6; remote log/artifact availability was not reverified.

### Final candidate and evidence matrix

The final source candidate is `04c17a42b6f8099e0ef0ba13951abdd4511b201a`,
package `0.4.0`. Comparing its production code, manifest, lockfile and tests
with the original `v0.4.0` candidate shows only the Unix successful-return
lock-release changes in `src/storage/switching.rs`: explicitly unlock before
both no-op and successful-switch returns. HEAD `9c35b90` differs from this
candidate only in documentation and the Linux workflow; current uncommitted
T5/T6 changes are documentation only. Windows production behavior is unchanged,
but the macOS workflow predates this Unix behavior change.

| Gate | Retained evidence | Final-candidate assessment |
| --- | --- | --- |
| Formatting | Original candidate CI; T2 and T3 native fmt passed | Passed for final source in T3; T5/T6 documentation not checked |
| Windows Check / Clippy / Test | Original candidate CI plus local 2026-10-04 checks; 160 tests | Passed locally for source identical to final candidate; see acceptance attempt below |
| Windows official A/B workflow | T1 reuses M4 Windows x86_64 / NTFS evidence | Final-source equivalence and current native checks confirmed on 2026-10-04; smoke reused |
| macOS Check / Clippy / Test and A/B workflow | Run 37205835491 on `9c35b90`; arm64 / APFS; successful job screenshot plus retained environment/smoke artifact | Final-source acceptance passed; exact test total not present in supplied evidence |
| Linux Check / Clippy / Test and A/B workflow | Run 36843186140 on `04c17a4`; Ubuntu 24.04 x86_64 GNU; 164 tests | Passed for final source; receipt/payload/PATH/residue evidence retained in T3 |
| Security review | T4 review of `6f42688` plus focused Unix repair review | No recorded unresolved release blocker; 99-dependency advisory scan dated 2026-10-01 |
| Instructions and limitations | T5 README and this record | Complete; mapped/unverified targets and observed limitations remain explicit |
| Final decision and synchronization | T6 record, README, roadmap and v0.5 task log | M5 Complete; minimum three-target scope confirmed; retain package 0.4.0; no new tag or publication |

Initial failures, repairs, exact archive checksums, artifact identifiers and
limitations remain in T1–T4 above. No cfg-excluded or absent test is counted as
passing. The macOS non-UTF-8 on-disk fixture, mapped Windows arm64/macOS x86_64/
Linux GNU arm64, and other filesystem/host combinations remain unverified.

### Scope, version and publication decision

- Candidate release scope: Node.js version management with the existing five
  commands on Windows `x86_64-pc-windows-msvc`, macOS `aarch64-apple-darwin`,
  and Linux `x86_64-unknown-linux-gnu`, subject to recorded environment limits.
  This scope is provisional until the final candidate passes the gate.
- Version decision: retain package version `0.4.0`; defer choosing a new
  release version until acceptance closes. The v0.5 design name does not
  select `0.5.0`; the old `v0.4.0` tag does not contain the Unix repair.
- Publication decision: no release approved, new tag, binary publication,
  installer, signing or provenance claim. Such actions remain separate.
- M5 decision: **Awaiting validation**, because the same repaired candidate
  lacks the complete required native evidence. No historical pass is revoked
  and no missing run is represented as passed.

### Conditions for completing M5

1. Record Check, Clippy and Test evidence for the repaired candidate on all
   three minimum native platforms, with shared fmt evidence. The design
   requires a full matrix rerun after locking behavior changes; the existing
   T3 run can supply the Linux result for this exact source.
2. Run the official macOS arm64 A/B workflow on the repaired candidate and
   retain native environment, exact outputs, checksums, PATH, payload and
   residue evidence. Windows smoke reuse requires an explicit final-candidate
   equivalence decision and current native checks; Linux evidence is retained.
3. Review the completed matrix, confirm security evidence still applies,
   select the release version explicitly, and synchronize M5 to Complete only
   when every gate is met. If candidate source/dependencies change, apply the
   design's affected-platform rerun and advisory-review requirements.

## Acceptance attempt on 2026-10-04

M5 remains **Awaiting validation**. Local Windows acceptance passed for HEAD
`9c35b9047c20fda8ce9d2c6bb00afd057fd9077d`; a Git comparison against
`04c17a42b6f8099e0ef0ba13951abdd4511b201a` found no differences in `src`,
`tests`, `Cargo.toml` or `Cargo.lock`. The working tree contained only the four
T5/T6 documentation edits. Rust was `1.98.1`, host
`x86_64-pc-windows-msvc`, Cargo `1.98.1`, LLVM `22.1.8`.

| Local command | Result |
| --- | --- |
| `cargo fmt --check` | Passed; no output |
| `cargo check --all-targets --locked --offline` | Passed |
| `cargo clippy --all-targets --all-features --locked --offline -- -D warnings` | Passed |
| `cargo test --all --locked --offline` | Passed: 133 library + 19 CLI + 8 selection CLI tests, 160 total; 0 failed or ignored |

Offline resolution used the locked cached dependencies without weakening TLS
or changing dependency versions. Windows smoke reuse is confirmed for this
final source: the only production change since the original candidate is in
the Unix-only switching module; Windows source behavior is unchanged and now
has current native checks. The retained M4 official 22.0.0 / 24.0.0 NTFS
workflow remains the smoke evidence; no new official binaries were downloaded
or executed. A current `Get-Volume` filesystem query returned access denied;
no new filesystem evidence is claimed.

The retained Linux T3 checks and official A/B run cover the same final source.
The remaining gate is macOS arm64 native Check/Clippy/Test and official A/B
workflow after the Unix lock-release repair. GitHub CLI authentication reports
an invalid token for the configured account. Attempting
`gh workflow run m5-t2-macos.yml --ref main -f node_a=22.0.0 -f node_b=24.0.0`
returned HTTP 401 and created no run. Browser automation had no existing
GitHub browser tab; opening the workflow page timed out, so no browser-based
run was created or verified. No credentials were changed.

Resume by restoring authorized GitHub access, dispatching the macOS workflow
on a ref whose production source matches the final candidate, and retaining
its commit, environment, checks, exact A/B outputs and artifact evidence.
Reconcile that run with the Windows local and Linux retained evidence before
marking M5 Complete. No new release version, tag or publication was created.

### GitHub plugin follow-up on 2026-10-04

The GitHub plugin successfully fetched remote commits independently of the
invalid local CLI credential: `dev` is
`9c35b9047c20fda8ce9d2c6bb00afd057fd9077d`, while `main` is
`4a705a65ce0bf69c921c06140082a52015479a16` and predates the Unix repair.
The earlier attempted `--ref main` dispatch would not have validated the
repaired source even if authentication had succeeded. Use `dev` for the
pending macOS workflow, and verify its resolved commit before accepting it.

Available plugin Actions tools read jobs/logs/artifacts and rerun existing
jobs; none dispatches a new workflow. Rerunning the old macOS job would retain
its old commit and cannot close this gap. The plugin's commit-workflow query
returned no entries for the final candidate/HEAD, but it filters to PR-triggered
runs and is first-page-only; that result does not prove absence of manual or
push runs. No new run was created through the plugin.

Start the existing macOS workflow on branch `dev` with `node_a=22.0.0` and
`node_b=24.0.0`, then provide the run URL or ID. The plugin can read the new
run's jobs, logs and artifacts to complete evidence review without repairing
local CLI authentication. M5 remains Awaiting validation.

## Final acceptance on 2026-10-04

**M5 Complete, release scope confirmed.** The user supplied the successful
[macOS run 37205835491](https://github.com/verslot/verslot/actions/runs/37205835491)
summary screenshot and its downloaded artifact. These close the macOS gate
for the repaired source. This section supersedes the earlier pending decisions;
those attempt records remain to preserve the failures and recovery history.

| Evidence | Reviewed result |
| --- | --- |
| Commit | `9c35b9047c20fda8ce9d2c6bb00afd057fd9077d`, branch `dev`; production source/tests/manifest/lockfile identical to Linux candidate `04c17a4` |
| Native environment | macOS 15.7.9 (24G830), arm64, Apple virtualized kernel; image `macos15` / `20260907.0337.1`; APFS root, temporary storage on Data volume; filename probe `case_sensitive=false` |
| Toolchain | Rust/Cargo 1.98.1, host `aarch64-apple-darwin`, Clippy 0.1.98 |
| Checks and build | Supplied run summary shows `validate-macos-arm64` Success. At this commit the job runs fmt, locked Check, locked Clippy with warnings denied, locked Test and build as mandatory sequential steps before smoke; success establishes their completion. Full check logs and exact test total were not supplied or independently fetched |
| Official A/B workflow | 22.0.0 / 24.0.0 archive digests match retained official values; empty initial queries, duplicate install, numeric list, selection/current, fixed-entry/PATH execution, no-op, competing PATH, selected-uninstall rejection, inactive-uninstall success and payload preservation passed |
| Final state | Smoke ends `PASS: B selected/executable; A absent; B bytes unchanged; tmp empty; no reserved siblings.` |
| Supplied artifact | `m5-t2-macos-arm64-37205835491-1.zip`; locally computed SHA-256 `e71d326b1e4bf5bd768047e0010ac7b7df12a5de4325d7577e36c812f400d571` matches the visible screenshot prefix; full server digest and artifact ID not independently retrieved |
| Retained evidence | [Environment](evidence/m5-macos-37205835491/environment.txt), [smoke](evidence/m5-macos-37205835491/smoke.txt), [user-supplied run summary](evidence/m5-macos-37205835491/run-summary.png) |

The new macOS evidence, Windows local 160-test acceptance and confirmed
Windows smoke equivalence, Linux final-source 164-test acceptance and official
workflow, T4 security review including the Unix repair, and T5 documentation
satisfy the minimum gate. The earlier macOS count of 163 belongs to its old
run and is not asserted for this rerun. No absent or excluded fixture is counted
as passed; previously recorded platform/filesystem/security limitations remain.
The advisory scan remains the dated 2026-10-01 result, not a new scan.

Confirmed scope is the five existing Node.js commands on Windows
`x86_64-pc-windows-msvc`, macOS `aarch64-apple-darwin`, and Linux
`x86_64-unknown-linux-gnu` under recorded environment limitations. Windows
arm64, macOS x86_64 and Linux GNU arm64 remain mapped but unverified.

Final version decision: retain package version `0.4.0` for this accepted source;
no new release version is selected. The existing `v0.4.0` tag still predates the
Unix repair and is not the accepted source identifier. Use the candidate SHA
or the source-equivalent HEAD above. M5 acceptance confirms scope; it does not
publish binaries, create a tag, approve signing/provenance, or authorize a new
hosted release. Publication and any later version bump remain separate actions.

## Progress log

| Date | Item | Result / next step |
| --- | --- | --- |
| 2026-10-01 | T1 Windows delivery validation | Complete through exact-candidate Windows CI plus verified production-source equivalence with retained M4 official A/B evidence; local fmt passed, local Cargo dependency resolution was blocked by Schannel/index availability without weakening TLS |
| 2026-10-01 | Start T2 macOS delivery validation | Exact-candidate macOS Check/Clippy/Test steps passed, but native Apple-silicon official workflow and full environment/log evidence are missing; provide a native macOS runner or an authorized way to run the candidate workflow before T3 |
| 2026-10-01 | Repair initial manual-workflow parse failure | The first merged workflow produced run 36776022008 with no jobs and rejected dispatch because multiline expected-output literals escaped the YAML block indentation. Replaced them with single-line Bash ANSI-C newline expressions; no product code or acceptance result changed |
| 2026-10-01 | Run repaired T2 workflow | [Run 36776678819](https://github.com/verslot/verslot/actions/runs/36776678819) passed arm64 assertion, all four checks, 163 tests, build, official 22.0.0 / 24.0.0 workflow and evidence upload. Evidence review found the filesystem filter recorded no filesystem/case-sensitivity fields, so T2 remains In progress pending a focused rerun; update deprecated Node 20 actions during that evidence repair |
| 2026-10-01 | Complete T2 macOS delivery validation | PR [#8](https://github.com/verslot/verslot/pull/8) repaired filesystem/case evidence and updated Actions. [Run 36777691817](https://github.com/verslot/verslot/actions/runs/36777691817) passed on macOS 15.7.9 arm64 / APFS: all four checks, 163 tests, build, official Node.js 22.0.0 / 24.0.0 workflow, payload/residue assertions and evidence upload. T2 Complete; begin T3 Linux validation next |
| 2026-10-01 | Start T3 Linux delivery validation | Added a manual Ubuntu 24.04 x86_64 GNU workflow that asserts a non-root glibc environment, records filesystem/mount and virtualization evidence, runs all four checks, and performs the official A/B workflow. Native execution and evidence review remain pending |
| 2026-10-01 | Complete T4 security review | Reviewed all eight v0.5 security areas and their mapped regression tests. `cargo-audit 0.22.2` scanned 99 locked dependencies against RustSec database commit `9b3a3b7` (1,277 advisories) with no vulnerabilities or warnings. No release-blocking finding; T3 remains independently pending |
| 2026-10-01 | Complete T3 Linux delivery validation | [Run 36843186140](https://github.com/verslot/verslot/actions/runs/36843186140) passed the Ubuntu 24.04 x86_64 GNU environment assertions, all four checks, 164 tests, build, official Node.js 22.0.0 / 24.0.0 workflow and evidence upload. Earlier runs exposed and repaired explicit Unix lock release on both successful return paths plus push-trigger input defaults. T3 Complete; begin T5 documentation next |
| 2026-10-04 | Complete M5 T5 documentation | README now covers pinned-source installation, build prerequisites, commands, platform evidence, download trust and observed limitations; [T5 record](m5-validation.md#t5-installation-usage-and-limitations-documentation). M5 In progress (5 / 6). Next: T6 final evidence matrix and release scope/version decision. Documentation only; checks Not run; no version, tag or publication change |
| 2026-10-04 | Complete T6 final record and decision | Added final-candidate evidence matrix, source comparison, provisional three-target scope and version/publication decision. T6 documentation complete; M5 Awaiting validation (6 / 6 deliverables). Final-candidate Windows checks and macOS checks/A/B workflow remain missing; no checks or workflows run in T6 |
| 2026-10-04 | Execute M5 acceptance | Windows fmt/Check/Clippy/Test passed with 160 tests on final-equivalent source; Windows official workflow reuse confirmed and Linux final-source evidence retained. macOS dispatch failed HTTP 401; browser fallback timed out. M5 remains Awaiting validation pending macOS native checks/A/B evidence |
| 2026-10-04 | Close M5 acceptance | Reviewed user-supplied successful macOS run 37205835491 screenshot and environment/smoke artifact for final-equivalent 9c35b90; retained evidence in docs. M5 Complete (6 / 6), minimum three-target scope confirmed, package 0.4.0 retained; no new tag or publication. Rerun test total not supplied |
