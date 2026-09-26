# Verslot Development Roadmap and Progress

Last updated: 2026-09-22

This document tracks development phases, tasks, and delivery progress. The CLI foundation is defined by the [v0.1 specification](v0.1.md) and [README](../README.md); implemented target validation and remaining M2 tasks are defined by the [v0.2 specification](v0.2.md). Later phases describe the planned development order; they do not indicate completed work or committed release dates.

## Progress Overview

> **M2 is complete with local acceptance passed**: Windows checks and 44 tests passed; Linux/macOS validation remains incomplete and does not block M2 acceptance. T1–T6 deliverables and test code are complete, including target validation, storage boundaries, internal state reads, and acceptance mapping; the CLI still cannot manage Node.js versions.

| Metric | Current Status |
| --- | --- |
| Package version | `0.2.0` (does not indicate a published release) |
| Current milestone | M2 complete: T1–T6 complete (6 / 6 tasks), local Windows acceptance passed; Linux/macOS unverified (non-blocking) |
| Core features implemented | **0 / 5**: install, uninstall, switch, list, and current-version queries are not implemented |
| Active development tasks | M2 complete; M3 not started; Linux/macOS validation remains a recorded limitation |
| Next step | Plan M3 Node.js installation and queries; retain unverified platform limitations |
| Design decisions | Full versions only; fixed current link with Unix symlinks / Windows junctions and manual PATH configuration; see [v0.2](v0.2.md) |
| Validation status | All four checks passed on Windows; 27 library + 17 CLI tests passed; [evidence](m2-validation-windows.md); Linux/macOS not run |

Implementation counts reflect features users can actually use; placeholder commands do not count as implemented. Phases differ in effort, so task counts are not used to estimate an overall project completion percentage.

## Feature Status

| Capability | Implementation Status | Current Behavior | Code / Test Evidence |
| --- | --- | --- | --- |
| Help, version, and argument-count constraints | Implemented | Provides help and version output; invalid CLI usage exits with code `2` | [CLI definitions](../src/cli.rs), [CLI tests](../tests/cli.rs) |
| Target parsing | Implemented; Windows validated, Unix pending | All three target commands validate full versions before dispatch; invalid targets report shared diagnostics on stderr with exit code `2` | [Parser and unit tests](../src/target.rs), [CLI definitions](../src/cli.rs), [CLI tests](../tests/cli.rs) |
| Storage roots and paths | Implemented; Windows validated, Unix pending | Native root resolution and read-only path construction enforce canonical directory boundaries | [Storage](../src/storage.rs), [tests](../src/storage/tests.rs) |
| Internal current-version state | Implemented; Windows validated, Unix pending | Reads native current links; distinguishes no selection from invalid state; not connected to the `current` command | [State reads](../src/storage.rs), [platform links](../src/storage/links.rs), [tests](../src/storage/tests.rs) |
| `install` | Placeholder only | Valid targets return `not implemented` with exit code `1`; invalid targets exit with code `2` | [Command dispatch](../src/lib.rs), [Exit handling](../src/main.rs) |
| `uninstall` | Placeholder only | Same as above; does not remove any installations | [Command dispatch](../src/lib.rs) |
| `use` | Placeholder only | Same as above; does not switch versions | [Command dispatch](../src/lib.rs) |
| `list` | Placeholder only | Returns `not implemented` with exit code `1`; does not read the list of installed versions | [Command dispatch](../src/lib.rs) |
| `current` | Placeholder only | Same as above; does not query the current version | [Command dispatch](../src/lib.rs) |

“Implemented” only means that the code exists. Whether acceptance criteria have been met is recorded separately in the milestones and progress log.

## Development Roadmap

Milestone statuses: Not started → In progress → Awaiting validation → Complete. If work cannot proceed, mark it as “Blocked” and record the cause and the conditions for unblocking it. Mark work as “In progress” only when it has actually started.

| Milestone | Sequence | Status | Checklist Progress | Completion Criteria |
| --- | --- | --- | --- | --- |
| M1 CLI foundation | Before M2 | Awaiting validation | 5 / 6 | Help, version, argument constraints, and placeholder behavior match the documentation and pass validation |
| M2 Target parsing and local state | Before M3 | Complete | 6 / 6 | Valid targets can be parsed, invalid targets are rejected, and local state read/write rules are defined |
| M3 Node.js installation and queries | After M2 | Not started | 0 / 7 | Specified versions can be installed safely, query results match disk state, and installed versions can be uninstalled |
| M4 Version switching | After M3 | Not started | 0 / 5 | Installed versions can be selected, and the version actually executed matches the current state |
| M5 Cross-platform delivery | After M4 | Not started | 0 / 6 | Core workflows pass validation on each platform, and usage instructions and limitations are documented |

Checklist progress counts the checked items below. It reflects completed tasks, not effort or delivery percentages. Later milestones have no preset version numbers or dates; details will be refined as development approaches.

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

- [ ] Select the official distribution archive based on the operating system and architecture.
- [ ] Download the distribution archive and verify its checksum.
- [ ] Extract archives safely, preventing path traversal and symlinks from escaping the destination.
- [ ] Finalize installations and clean up failures without exposing incomplete installations.
- [ ] Implement `list` to display installed versions.
- [ ] Implement `uninstall` and define the rules for uninstalling the currently active version.
- [ ] Add tests for successful workflows, corrupted downloads, duplicate installations, and failure cleanup.

### Phase 4: Version Switching

- [ ] Implement `use` to select an installed Node.js version.
- [ ] Implement `current` to display the currently active version.
- [ ] Define behavior for targets that are not installed, no version being selected, and switching failures.
- [ ] Handle atomic updates and Windows file locking.
- [ ] Add tests for switching, state consistency, and preserving the previous state after failure.

### Phase 5: Cross-Platform Delivery

- [ ] Validate the install → list → switch → query → uninstall workflow on Windows.
- [ ] Validate the same workflow on macOS.
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

Append an entry for each subsequent update, noting the actual changes, validation results, and next steps. Include commit or issue links when available.

## Scope Constraints

Implement version management only. Node.js is the first target for end-to-end support; no other providers are currently planned. Keep a single crate, use stable Rust, prefer the standard library, isolate platform differences, and add dependencies only for actual requirements.

The scope excludes extension systems, task runners, environment-variable management, dotenv, secret management, shell scripts, telemetry, async runtimes, and GUIs.

## Design References

- [GitHub Projects best practices](https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/best-practices-for-projects): clarify dependencies, keep statuses updated, and link to actual work.
- [GitHub public roadmap](https://github.com/github/roadmap): distinguish feature stages and provide corresponding changelog entries upon delivery.
- [Atlassian agile roadmaps](https://www.atlassian.com/agile/product-management/roadmaps): organize the roadmap around goals and adjust it as work progresses.

This project uses a Markdown overview and checklists suited to its current size, without introducing an additional project-management system for now.
