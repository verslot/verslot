# Verslot Development Roadmap and Progress

Last updated: 2026-10-01

This document tracks development phases, tasks, and delivery progress. The CLI foundation is defined by the [v0.1 specification](v0.1.md) and [README](../README.md); implemented target validation and local state are defined by the [v0.2 specification](v0.2.md). The [v0.3 design](v0.3.md) defines M3 installation, listing, and uninstallation; the [v0.4 design](v0.4.md) defines M4 switching and current-version queries; the [v0.5 design](v0.5.md) defines M5 cross-platform delivery and release-scope validation. Planned work does not indicate completed delivery or a committed release date.

## Progress Overview

> **M2 and local M3 acceptance are complete**: M2 Windows checks and repaired three-platform CI passed. M3 install/list/uninstall passed local Windows checks, 120 tests and the official Node.js 22.0.0 smoke workflow; see [M3 evidence and platform limitations](m3-validation.md). M3 Unix and other architectures remain unverified.

| Metric | Current Status |
| --- | --- |
| Package version | `0.4.0` (does not indicate published binaries) |
| Current milestone | M5 In progress: 2 / 6 tasks complete. Windows T1 reuses verified M4 official A/B evidence plus exact-candidate CI; macOS T2 passed native Apple-silicon checks and the official A/B workflow; Linux T3 is next |
| Core features implemented | **5 / 5**: install, list, uninstall, use, and current have code and tests; local Windows M4 acceptance passed; full cross-platform delivery remains M5 |
| Active development tasks | M5 T1–T2 complete; T3 Linux native validation is next |
| Next step | Execute [v0.5 T3 Linux native validation](v0.5.md#platform-specific-acceptance). [M5 evidence](m5-validation.md) |
| Design decisions | Full versions only; fixed current link with Unix symlinks / Windows junctions and manual PATH configuration; [M2 foundations](v0.2.md), [M4 switching and recovery](v0.4.md) |
| Validation status | Exact `v0.4.0` candidate CI passed Check/Clippy/Test on Windows, macOS and Ubuntu; formatting passed. [M5 T1](m5-validation.md) verified production-source equivalence and reused [M4 Windows validation](m4-validation.md): 160 tests plus the official A/B smoke. [M5 T2](m5-validation.md#t2-macos-delivery-validation) passed all four checks, 163 tests and the official Node.js 22.0.0 / 24.0.0 workflow on macOS 15.7.9 arm64 / APFS; Linux official workflow and other architectures remain unverified |

Implementation counts reflect features users can actually use; placeholder commands do not count as implemented. Phases differ in effort, so task counts are not used to estimate an overall project completion percentage.

## Feature Status

| Capability | Implementation Status | Current Behavior | Code / Test Evidence |
| --- | --- | --- | --- |
| Help, version, and argument-count constraints | Implemented | Provides help and version output; invalid CLI usage exits with code `2` | [CLI definitions](../src/cli.rs), [CLI tests](../tests/cli.rs) |
| Target parsing | Implemented; M2 three-platform CI passed | All three target commands validate full versions before dispatch; invalid targets report shared diagnostics on stderr with exit code `2` | [Parser and unit tests](../src/target.rs), [CLI definitions](../src/cli.rs), [CLI tests](../tests/cli.rs) |
| Storage roots and paths | Implemented; M2 three-platform CI passed with recorded coverage limits | Native root resolution and read-only path construction enforce canonical directory boundaries | [Storage](../src/storage.rs), [tests](../src/storage/tests.rs) |
| Internal current-version state | Implemented; M2 three-platform CI passed with recorded coverage limits | Reads native current links; distinguishes no selection from invalid state; M4 adds completeness/locking before exposing it through `current` | [State reads](../src/storage.rs), [platform links](../src/storage/links.rs), [tests](../src/storage/tests.rs) |
| Complete selected state and M4 coordination | M4 T1 code/tests complete; Windows checks/tests passed; see [evidence](m4-t1-t3-validation.md) | Adds a read-only complete-selection query with shared locking; all existing writers reject reserved switch residue under the exclusive lock; integrated into current/use CLI | [Selection checks](../src/storage.rs), [coordination](../src/mutation.rs), [state/concurrency tests](../src/storage/selection_tests.rs), [T1 record](v0.4.md#t1-implementation-record) |
| Internal Unix selection | M4 T2 code/tests complete; formatting passed; Unix checks/tests Not run | Unix Storage::select prepares native symlinks, atomically replaces the current link, verifies and rolls back failures, and removes only identified links; used by use CLI; Windows has a separate T3 implementation | [Unix switching](../src/storage/switching.rs), [link identity](../src/storage/links.rs), [Unix tests](../src/storage/switching/tests.rs), [T2 record](v0.4.md#t2-implementation-record) |
| Internal Windows selection | M4 T3 code/tests complete; Windows checks/tests passed; see [evidence](m4-t1-t3-validation.md) | Windows Storage::select prepares junctions, backs up/publishes/restores with a possible entry-point gap, checks identity and confirmed removal, and preserves unexpected entries; used by use CLI | [Windows switching](../src/storage/switching_windows.rs), [link identity](../src/storage/links.rs), [Windows tests](../src/storage/switching_windows/tests.rs), [T3/dependency record](v0.4.md#t3-implementation-and-dependency-record) |
| Node.js distribution selection | T1 code and tests complete; Windows acceptance passed; other platforms unverified | Maps six build platforms to fixed official archive/checksum URLs; rejects unsupported platforms; integrated into install | [Distribution selection and tests](../src/distribution.rs), [dependency decision](v0.3.md#t1-dependency-decision) |
| Verified distribution downloads | T2 code and tests complete; Windows acceptance passed; other platforms unverified | Bounded synchronous HTTPS, no redirects/decoding, exact checksum selection and streaming SHA-256; integrated into install | [Downloads](../src/download.rs), [offline tests](../src/download/tests.rs), [dependency record](v0.3.md#t2-implementation-and-dependency-record) |
| Safe extraction and complete-installation checks | T3 code and tests complete; Windows acceptance passed; other platforms unverified | Native ZIP/tar decoding, containment/type/link policy, completion receipt and executable validation; integrated into install/list/uninstall | [Installation module](../src/installation.rs), [shared](../src/installation/tests.rs), [Unix](../src/installation/tests/unix.rs), [Windows](../src/installation/tests/windows.rs), [dependency record](v0.3.md#t3-implementation-and-dependency-record) |
| `install` | T4 code and tests complete; Windows acceptance passed; other platforms unverified | Verified installation under an OS lock, atomic rename commit, operation-only cleanup; complete duplicates succeed without downloading; incomplete destinations preserved | [Install lifecycle](../src/install.rs), [mutation helpers](../src/mutation.rs), [offline tests](../src/install/tests.rs), [CLI tests](../tests/cli.rs) |
| `uninstall` | T6 code and tests complete; Windows acceptance passed; other platforms unverified | Shared-lock/current-state protection, complete inactive installations detached before deletion; selected/invalid/missing/incomplete targets rejected; rename failure preserves original installation, cleanup failure reports detached leftovers | [Uninstall](../src/uninstall.rs), [offline tests](../src/uninstall/tests.rs), [CLI tests](../tests/cli.rs) |
| `use` | M4 T4 code/tests complete; Windows acceptance passed | Selects complete installed versions; exact using/already using output; operational errors leave stdout empty; no automatic installation | [Command dispatch](../src/lib.rs), [CLI regressions](../tests/selection_cli.rs) |
| `list` | T5 code and tests complete; Windows acceptance passed; other platforms unverified | Read-only complete installations sorted numerically; missing/empty storage succeeds without writes; canonical incomplete entries and I/O/boundary errors fail without partial stdout; does not inspect current state | [Inventory](../src/inventory.rs), [offline tests](../src/inventory/tests.rs), [CLI tests](../tests/cli.rs) |
| `current` | M4 T4 code/tests complete; Windows acceptance passed | Read-only complete-selection query; prints selected target or empty output; invalid state, residue and busy storage fail | [Command dispatch](../src/lib.rs), [CLI regressions](../tests/selection_cli.rs) |
| M4 workflow coverage and acceptance preparation | M4 T5 tests/docs complete; Windows checks/tests/smoke passed | Adds fresh offline install/select/query/uninstall, synthetic native entry/PATH execution and cross-process switch contention; manual PATH guidance and real A/B smoke procedure prepared | [Install workflow](../src/install/tests.rs), [CLI execution](../tests/selection_cli.rs), [acceptance mapping](v0.4.md#acceptance-mapping), [validation results](m4-validation.md) |
| M3 workflow coverage and acceptance preparation | T7 tests and documentation complete; Windows acceptance passed; other platforms unverified | Isolated offline fresh-install lifecycle and seeded CLI workflow fixtures; actual test-name acceptance mapping and a separate official HTTPS smoke procedure | [Lifecycle tests](../src/install/tests.rs), [CLI tests](../tests/cli.rs), [acceptance mapping](v0.3.md#acceptance-mapping), [validation record](m3-validation.md) |

“Implemented” only means that the code exists. Whether acceptance criteria have been met is recorded separately in the milestones and progress log.

## Development Roadmap

Milestone statuses: Not started → In progress → Awaiting validation → Complete. If work cannot proceed, mark it as “Blocked” and record the cause and the conditions for unblocking it. Mark work as “In progress” only when it has actually started.

| Milestone | Sequence | Status | Checklist Progress | Completion Criteria |
| --- | --- | --- | --- | --- |
| M1 CLI foundation | Before M2 | Awaiting validation | 5 / 6 | Help, version, argument constraints, and placeholder behavior match the documentation and pass validation |
| M2 Target parsing and local state | Before M3 | Complete | 6 / 6 | Valid targets can be parsed, invalid targets are rejected, and local state read/write rules are defined |
| M3 Node.js installation and queries | After M2 | Complete | 7 / 7 | Specified versions can be installed safely, query results match disk state, and installed versions can be uninstalled |
| M4 Version switching | After M3 | Complete under local Windows acceptance | 5 / 5 capability code items; 5 / 5 tasks | Complete installed versions can be selected and queried; the fixed entry point and controlled PATH execute the selected version; failure recovery is validated |
| M5 Cross-platform delivery | After M4 | In progress | 2 / 6 | Core workflows pass validation on each platform, and usage instructions and limitations are documented |

Checklist progress counts the checked items below. It reflects completed tasks, not effort or delivery percentages. M3 is planned in v0.3, M4 in v0.4, and M5 in [v0.5](v0.5.md); M5 has no preset package version or date. No planned version implies a release commitment.

### Phase 1: CLI Foundation

- [x] Set up a single-crate project and forbid unsafe Rust.
- [x] Provide help and version output.
- [x] Define the five version-management commands and their argument-count constraints.
- [x] Define exit behavior for placeholder commands and invalid usage.
- [x] Add CLI integration test code.
- [ ] Record the validation results required by the project.

### Phase 2: Target Parsing and Local State

- [x] Define tool names, version formats, and error-message rules.
- [x] Decide whether to support partial versions such as `node@22`, and define their resolution rules (not supported; full versions required).
- [x] Implement parsing and validation for `<tool>@<version>`.
- [x] Define the storage layout for installation directories, temporary directories, and current-version state.
- [x] Define the version-switching mechanism and platform-specific differences.
- [x] Add tests for parsing, path boundaries, and local state behavior.

Design evidence: [v0.2 T1](v0.2.md#t1-finalize-parsing-and-local-state-conventions), [parsing rules](v0.2.md#target-format), [local state](v0.2.md#local-state), and [M4 switching contract](v0.2.md#version-switching). These four completed design items correspond to one v0.2 task (T1), not four implementation tasks. The fifth completed checklist item is the [T2 shared parser](../src/target.rs), including parser unit test code, now integrated into the CLI by [T3](../src/cli.rs) with [CLI regression tests](../tests/cli.rs); T4–T6 add [storage/state implementation](../src/storage.rs), [native link reads](../src/storage/links.rs), [storage/state test code](../src/storage/tests.rs), and [acceptance mapping](v0.2.md#acceptance-mapping), completing the sixth checklist item. M2 validation starts only after T1–T6 are complete, as defined by the [v0.2 validation gate](v0.2.md#m2-validation-gate).

### Phase 3: Node.js Installation and Queries

Design: [v0.3](v0.3.md), with seven implementation tasks distinct from this capability checklist. M3 queries mean installed-version listing; `current` and `use` remain M4. T1–T7 code, tests, workflow coverage and documentation are complete (7 / 7 tasks). M3 local Windows acceptance passed: four checks, 120 tests and the official Node.js 22.0.0 smoke workflow; other platforms remain unverified. The [acceptance mapping](v0.3.md#acceptance-mapping) references actual test names, and [m3-validation.md](m3-validation.md) records pending checks, smoke procedure and coverage limitations.

- [x] Select the official distribution archive based on the operating system and architecture.
- [x] Download the distribution archive and verify its checksum.
- [x] Extract archives safely, preventing path traversal and symlinks from escaping the destination.
- [x] Finalize installations and clean up failures without exposing incomplete installations.
- [x] Implement `list` to display installed versions.
- [x] Implement `uninstall` and define the rules for uninstalling the currently active version.
- [x] Add tests for successful workflows, corrupted downloads, duplicate installations, and failure cleanup.

### Phase 4: Version Switching

Design: [v0.4](v0.4.md), with five implementation tasks distinct from this capability checklist. M3 local acceptance is complete. M4 T1–T5 code/tests/docs are complete (5 / 5 tasks), completing all five capability code items; M4 is Complete under local Windows acceptance. Use/current CLI regressions, fresh offline lifecycle, cross-process switch contention and synthetic fixed-entry/PATH execution coverage exist. [Actual acceptance mapping](v0.4.md#acceptance-mapping), README PATH guidance and [validation results](m4-validation.md) are ready. All four checks, 160 tests and the official 22.0.0 / 24.0.0 switching/PATH/uninstall smoke passed on Windows x86_64 / NTFS; [acceptance results and initial repairs](m4-validation.md). Unix/other architectures remain unverified; M5 owns cross-platform delivery.

- [x] Implement `use` to select an installed Node.js version; Windows CLI acceptance passed; other platforms unverified.
- [x] Implement `current` to display the selected complete version; Windows CLI acceptance passed; other platforms unverified.
- [x] Define behavior for targets that are not installed, no version being selected, and switching failures; CLI output/exit regressions passed on Windows.
- [x] Handle native updates and Windows file locking in internal switching (Unix atomic replacement; Windows backup/publish gap); CLI integrated and Windows acceptance passed; Unix platform validation pending.
- [x] Add internal tests for switching, state consistency, and preserving the previous state after failure; CLI/workflow tests, mapping and Windows acceptance complete; Unix/other architectures unverified.

### Phase 5: Cross-Platform Delivery

Design: [v0.5](v0.5.md), with six delivery tasks corresponding to this checklist. The minimum gate requires native Windows x86_64, macOS arm64, and Linux x86_64 GNU checks and official two-version workflows, followed by a security review, evidence-based documentation, and an explicit release decision. T1 Windows is complete through exact-candidate CI and a documented M4-equivalence decision; T2 macOS is complete through native Apple-silicon checks and the official two-version workflow. See [current evidence](m5-validation.md).

- [x] Validate the install → list → switch → query → uninstall workflow on Windows.
- [x] Validate the same workflow on macOS.
- [ ] Validate the same workflow on Linux.
- [ ] Complete a security review of paths, archive extraction, checksum verification, and untrusted remote data.
- [ ] Update installation instructions, command examples, and known limitations.
- [ ] Record validation results and confirm the release scope and version number.

## Development Workflow

1. Define the task scope, expected behavior, and acceptance criteria; record decisions for unresolved design questions first.
2. Mark the relevant item as “In progress” and implement only the minimum changes required for the current task.
3. Add tests for behavior changes and regression tests for bug fixes.
4. After implementation, mark the item as “Awaiting validation”, run the checks required by the project, and record the actual results.
5. Once acceptance criteria are met, mark the milestone as “Complete” and record validation evidence.

Whenever a task changes status, update the progress overview, relevant feature status, checklist items, and milestone counts together, and append a progress log entry. Implementation items can be checked off once they are in place, but an entire milestone must pass acceptance before it is marked as “Complete”. Prefer links to the relevant code, commits, PRs, or CI results as evidence. Explicitly label checks that were not run as “Not run”.

The project requires the following implementation validation commands. Listing them here documents the workflow requirements; it does not mean they have been executed.

```text
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

When work is blocked, record the cause and the conditions for unblocking it. Work that is not implemented or has not been validated must not be recorded as delivered.

## Progress Log

| Date | Item | Status and Results | Next Step or Blocker |
| --- | --- | --- | --- |
| 2026-09-18 | Establish the roadmap and progress baseline | Existing code and documentation reviewed; CLI skeleton and test code are present; validation was not run during this update | Define target parsing, storage layout, and the switching mechanism |
| 2026-09-18 | Add a trackable progress view | Added the progress overview, feature status, milestone counts, and code evidence; feature progress is unchanged; validation was not run | M1 validation results have yet to be recorded; M2 has not started |
| 2026-09-22 | Complete v0.2 T1 design conventions | M2 in progress: 4 / 6 roadmap checklist items, 1 / 6 v0.2 tasks; finalized parsing diagnostics, storage boundaries, current-link semantics, and platform switching contract; documentation only, no runtime changes; validation not run | Implement T2 shared parsing and test code; M2 validation waits for T1–T6; M1 validation remains unrecorded |
| 2026-09-22 | Complete v0.2 T2 shared parsing | M2 in progress: 5 / 6 roadmap checklist items, 2 / 6 v0.2 tasks; added typed parsing, canonical version rendering, ordered diagnostics, and parser unit test code in [src/target.rs](../src/target.rs); CLI behavior unchanged; all validation commands not run under the M2 gate | Implement T3 CLI validation; T4–T6 remain pending; M1 validation remains unrecorded |
| 2026-09-22 | Complete v0.2 T3 CLI validation | M2 in progress: 5 / 6 roadmap checklist items, 3 / 6 v0.2 tasks; integrated shared parsing into all three target commands and added CLI diagnostics, placeholder, and temporary-directory regression test code; no dependencies added; all validation commands not run under the M2 gate | Implement T4 storage roots and paths; T5–T6 remain pending; M1 validation remains unrecorded |
| 2026-09-22 | Complete v0.2 T4 storage paths | Added native roots, read-only typed paths, ancestor/boundary checks, and tests; Windows-only junction dependency and lockfile recorded; validation not run | Proceeded to T5 |
| 2026-09-22 | Complete v0.2 T5 internal state reads | Added native-link inspection, canonical version derivation, invalid-state rejection, and tests; CLI placeholders unchanged; validation not run | Proceeded to T6 |
| 2026-09-22 | Complete v0.2 T6 acceptance mapping | M2 awaiting validation: 6 / 6 roadmap checklist items and 6 / 6 v0.2 tasks; [acceptance mapping](v0.2.md#acceptance-mapping) references parser, CLI, root, boundary, and state tests; added isolated environment tests and existing-content preservation checks; all four validation commands Not run | Separate M2 validation gate is ready; record local validation evidence and unverified platform limitations; M1 validation remains unrecorded |
| 2026-09-22 | Run M2 validation on Windows | All four required checks passed on x86_64-pc-windows-msvc with Rust 1.98.1; 27 library and 17 CLI tests passed; repaired parser-test formatting after the initial fmt failure; [results and output](m2-validation-windows.md) | Local M2 acceptance passed; Linux/macOS and Unix-only tests not run (non-blocking); M1 milestone not separately reassessed |
| 2026-09-22 | Clarify M2 acceptance scope | M2 Complete: local Windows checks and 44 tests passed; Linux/macOS remain unverified but do not block M2 acceptance; corrected the earlier stricter interpretation | Next: M3; full cross-platform delivery validation remains in M5; documentation only, no checks rerun |
| 2026-09-22 | Bump package version to 0.2.0 | Updated Cargo.toml, the verslot entry in Cargo.lock, README, and version documentation; M2 local acceptance remains complete | Next: M3; validation evidence predates the metadata-only bump; checks not rerun and no release published |
| 2026-09-26 | Design v0.3 / M3 | Added [M3 specification](v0.3.md): distribution mapping, verified downloads, safe extraction, completion receipts, install/list/uninstall contracts, failure rules, dependency rationale, seven tasks, and acceptance plan; M3 remains Not started, 0 / 7 capabilities and 0 / 7 implementation tasks | Next: v0.3 T1; documentation only, checks Not run; package remains 0.2.0 and platform limitations remain unchanged |
| 2026-09-26 | Synchronize M2 CI evidence and design v0.4 / M4 | Linked the existing [repaired three-platform CI results](m2-validation-windows.md#pr-ci-repair-2026-09-26) in the current progress view; added [M4 design](v0.4.md) with explicit paths, use/current contracts, native link replacement, rollback, shared locking, unfinished-operation guards, five tasks and acceptance coverage | Next remains M3 T1; M4 Not started (0 / 5). Documentation only; no checks rerun, implementation, dependency changes, or version bump |
| 2026-09-26 | Implement v0.3 T1 distribution selection | M3 In progress: 1 / 7 capability items and 1 / 7 implementation tasks. Added [six-platform mapping, fixed official URLs, unsupported-target errors, and four unit tests](../src/distribution.rs); [dependency decision](v0.3.md#t1-dependency-decision) uses std for T1 and defers dependency installation to T2/T3 | Next: T2 verified downloads. All four validation commands Not run under the M3 gate; no CLI behavior changes, dependency changes, or version bump |
| 2026-09-26 | Implement v0.3 T2 verified downloads | M3 In progress: 2 / 7 capability items and 2 / 7 implementation tasks. Added [bounded HTTPS and streaming verification](../src/download.rs), [offline failure tests](../src/download/tests.rs), and [dependency rationale/review](v0.3.md#t2-implementation-and-dependency-record); added ureq 3.4.2 and sha2 0.11.0, resolved by `cargo fetch` | Next: T3 safe extraction and completeness. All four validation commands and smoke tests Not run under the M3 gate; CLI behavior and package version unchanged |
| 2026-09-26 | Implement v0.3 T3 safe extraction and completeness | M3 In progress: 3 / 7 capability items and 3 / 7 implementation tasks. Added [installation module](../src/installation.rs), native ZIP/tar decoding, deferred Unix link resolution, receipt/executable checks, and [offline fixtures](v0.3.md#t3-implementation-and-dependency-record); recorded zip/flate2/tar dependency rationale and security guidance | Next: T4 installation lifecycle and CLI. Dependencies fetched and new source formatted; all four validation commands/tests and smoke tests Not run under the M3 gate; no CLI behavior changes or version bump |
| 2026-09-26 | Implement v0.3 T4 installation lifecycle and CLI | M3 In progress: 4 / 7 capability items and 4 / 7 implementation tasks. Added [OS locking and bounded mutation paths](../src/mutation.rs), [install staging/commit/cleanup](../src/install.rs), [offline lifecycle and process-lock tests](../src/install/tests.rs), and install CLI regressions; complete duplicates skip downloads and invalid destinations are preserved | Next: T5 inventory/list. No dependencies or version bump; edited source formatted only. All four validation commands/tests and real-distribution smoke tests Not run under the M3 gate |
| 2026-09-27 | Implement v0.3 T5 inventory and list CLI | M3 In progress: 5 / 7 capability items and 5 / 7 implementation tasks; 2 / 5 core commands have code and tests. Added [read-only complete-installation inventory](../src/inventory.rs), [offline inventory/native I/O tests](../src/inventory/tests.rs), and list CLI regressions; numeric ordering, missing-root success, invalid-entry errors with empty stdout, current-state independence and lock-free reads | Next: T6 protected uninstallation. Updated README and v0.3 task/evidence records; no dependencies or version bump. All four validation commands/tests and real-distribution smoke tests Not run under the M3 gate |
| 2026-09-27 | Implement v0.3 T6 protected uninstallation | M3 In progress: 6 / 7 capability items and 6 / 7 implementation tasks; 3 / 5 core commands have code and tests. Added [uninstall lifecycle](../src/uninstall.rs), [native current/boundary/rename/delete and partial-cleanup tests](../src/uninstall/tests.rs), and CLI regressions; selected and invalid current state fail closed, only complete inactive installations are detached and removed | Next: T7 workflow coverage, acceptance mapping and validation preparation. Updated README and v0.3 task/evidence records; no dependencies or version bump. All four validation commands/tests and real-distribution smoke tests Not run under the M3 gate |
| 2026-09-27 | Complete v0.3 T7 workflow coverage and acceptance preparation | M3 Awaiting validation: 7 / 7 capability items and 7 / 7 implementation tasks; 3 / 5 core commands have code and tests. Added isolated offline lifecycle and seeded CLI workflow fixtures, actual test-name [acceptance mapping](v0.3.md#acceptance-mapping), and [M3 validation preparation/results record](m3-validation.md); synchronized README | Next: separate M3 acceptance checks and official-distribution smoke workflow. All checks/tests/smoke Not run; no runtime/dependency/version changes or release. M4 remains Not started |
| 2026-09-27 | Run M3 local Windows acceptance and repair initial failures | M3 Complete under local acceptance: Rust 1.98.1 / x86_64-pc-windows-msvc; four required checks passed; 100 library + 20 CLI tests passed; official 22.0.0 install/list/duplicate/absolute --version/uninstall/empty-list smoke passed. Repaired formatting, fixture digest encoding, Windows socket/lock/deletion assumptions and lock-entry type diagnostics; [initial and final evidence](m3-validation.md) | Next: M4. M3 Unix and other architectures unverified; no version bump or release. Initial sandbox PowerShell TLS fetch failed; official checksum fetched outside sandbox and compared successfully; production TLS verification retained |
| 2026-09-27 | Record repaired M3 CI and bump package to 0.3.0 | [Windows/Linux/macOS CI](https://github.com/verslot/verslot/actions/runs/36259879124) passed after Unix Clippy repairs in `59318c9`; updated Cargo.toml, only the verslot entry in Cargo.lock, README and version/validation documentation to 0.3.0 | Prepare PR #3 for review. CI evidence predates this metadata-only bump; no local checks or smoke rerun for the bump. Unix official smoke and other architectures remain unverified; no release published |
| 2026-09-28 | Complete M4 T1 selection state and coordination | M4 In progress: 1 / 5 implementation tasks; capability checklist 0 / 5 and core commands remain 3 / 5. Added complete selected-state reads, shared/exclusive locking and reserved-switch guard across writers, plus isolated state/concurrency/residue regression test code; [T1 implementation record](v0.4.md#t1-implementation-record) | Next: T2 Unix / T3 Windows switching. Source formatted and reviewed; all validation commands/tests Not run under the M4 gate; no dependency or version changes |
| 2026-09-28 | Complete M4 T2 Unix switching | M4 In progress: 2 / 5 implementation tasks; capability checklist 0 / 5 and core CLI commands remain 3 / 5. Added native symlink preparation, atomic replacement, verified rollback and identity-checked cleanup with native/injected-failure test code; [T2 implementation record](v0.4.md#t2-implementation-record) | Next: T3 Windows switching. Source formatted and reviewed; checks/tests Not run under the M4 gate; Linux/macOS behavior unverified; no dependency or version changes |
| 2026-09-28 | Complete M4 T3 Windows switching | M4 In progress: 3 / 5 implementation tasks; internal capability code checklist 2 / 5; core CLI commands remain 3 / 5. Added junction backup/publication/restore, identity-checked cleanup and confirmed removal, sharing/failure regression test code, plus a justified Windows-only dependency; [T3 record](v0.4.md#t3-implementation-and-dependency-record) | Next: T4 CLI integration. Source formatted and reviewed; dependency resolution/download succeeded after sandbox TLS failure; checks/tests Not run under the M4 gate; Windows native behavior unverified; no package version change |
| 2026-09-28 | Revalidate M4 T1–T3 on Windows | Four required checks passed on Rust 1.98.1 / x86_64-pc-windows-msvc; 130 library + 20 CLI tests passed. Repaired cleanup Clippy warnings and native sharing fixtures after an initial 127-pass/3-fail run; focused Windows switching suite passed 19 tests; [evidence and limits](m4-t1-t3-validation.md) | Continue T4. T2 Unix compilation/native tests and full M4 acceptance remain pending; no live distribution smoke or PATH changes |
| 2026-09-28 | Complete M4 T4 use/current CLI integration | M4 In progress: 4 / 5 tasks, 5 / 5 capability code items and 5 / 5 core commands. Integrated existing selection/query APIs, exact success/no-op/empty-query output and contextual errors; updated help and placeholder regressions; added isolated offline CLI selection, state, locking, residue and M3 integration tests | Next: T5 workflow coverage, acceptance mapping and documentation. All T4 validation commands/tests Not run; prior T1–T3 evidence remains separate; no dependencies or package version change |
| 2026-09-28 | Complete M4 T5 workflow/failure coverage and acceptance preparation | M4 Awaiting validation: 5 / 5 tasks and 5 / 5 capability code items. Added generated-archive fresh selection workflow, cross-process select/install/uninstall/current contention at native switch checkpoints, synthetic native fixed-entry/PATH execution and competing-PATH tests; [actual mapping](v0.4.md#acceptance-mapping), README/manual PATH and [M4 validation preparation](m4-validation.md) | Next: separate acceptance checks and official 22.0.0 / 24.0.0 smoke when requested. All T4/T5 checks/tests and full smoke Not run; prior T1–T3 Windows evidence remains separate; no runtime/dependency/version changes |
| 2026-09-28 | Complete local Windows M4 acceptance | Four required checks, 133 library + 19 CLI + 8 selection CLI tests (160 total) and official Node.js 22.0.0 / 24.0.0 A/B switch/current/direct-entry/PATH/no-op/protected-uninstall smoke passed on Rust 1.98.1 / x86_64-pc-windows-msvc / NTFS. Repaired formatting, borrowed expected output and a needless borrow in tests; retained sandbox lock/TLS failures and successful reruns; [full evidence](m4-validation.md) | M4 Complete under local Windows acceptance; next M5 cross-platform delivery. Unix/other architectures and other filesystems unverified; no dependency/version change, release or persistent PATH modification |
| 2026-09-30 | Bump package version to 0.4.0 and tag M4 | Updated Cargo.toml, only the verslot entry in Cargo.lock, README, roadmap and current v0.4 status; created the `v0.4.0` tag for the M4 implementation | M4 validation evidence predates this metadata-only bump; checks were not rerun. M5 cross-platform delivery remains next; no binaries were published |
| 2026-10-01 | Design v0.5 / M5 | Added [M5 specification](v0.5.md): minimum native platform matrix, isolated official A/B workflow, platform evidence rules, security review, documentation/release criteria, six tasks and acceptance gate | M5 remains Not started (0 / 6); documentation only, checks/workflows/security review Not run; no version change, tag, binary, or release publication. Next: T1–T3 native platform validation |
| 2026-10-01 | Complete M5 T1 Windows delivery validation and start T2 | Exact `v0.4.0` CI passed formatting and Windows/macOS/Linux Check, Clippy and Test steps. All production source hashes match retained M4 acceptance; T1 reuses the recorded Windows 22.0.0 / 24.0.0 A/B smoke. Local fmt passed; local Cargo resolution failed on Schannel/index availability without disabling TLS | M5 In progress (1 / 6). T2 candidate macOS CI passed but native Apple-silicon official workflow/environment evidence remains missing; T3 not started under the requested sequence. [Evidence](m5-validation.md) |
| 2026-10-01 | Complete M5 T2 macOS delivery validation | [Manual run 36777691817](https://github.com/verslot/verslot/actions/runs/36777691817) on merge commit `7dc4272` passed formatting, Check, Clippy, 163 tests, build, and the official Node.js 22.0.0 / 24.0.0 install/list/use/current/PATH/uninstall workflow on macOS 15.7.9 arm64 / APFS; recorded case-insensitive runner storage, exact official digests, payload preservation and clean residue | M5 In progress (2 / 6). T3 Linux x86_64 GNU native validation is next; macOS x86_64 and other mapped architectures remain unverified. [Evidence](m5-validation.md#t2-macos-delivery-validation) |

Append an entry for each subsequent update, noting the actual changes, validation results, and next steps. Include commit or issue links when available.

## Scope Constraints

Implement version management only. Node.js is the first target for end-to-end support; no other providers are currently planned. Keep a single crate, use stable Rust, prefer the standard library, isolate platform differences, and add dependencies only for actual requirements.

The scope excludes extension systems, task runners, environment-variable management, dotenv, secret management, shell scripts, telemetry, async runtimes, and GUIs.

## Design References

- [GitHub Projects best practices](https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/best-practices-for-projects): clarify dependencies, keep statuses updated, and link to actual work.
- [GitHub public roadmap](https://github.com/github/roadmap): distinguish feature stages and provide corresponding changelog entries upon delivery.
- [Atlassian agile roadmaps](https://www.atlassian.com/agile/product-management/roadmaps): organize the roadmap around goals and adjust it as work progresses.

This project uses a Markdown overview and checklists suited to its current size, without introducing an additional project-management system for now.
