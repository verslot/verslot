# Verslot Development Roadmap and Progress

Last updated: 2026-09-18

This document tracks development phases, tasks, and delivery progress. The current scope is defined by the [v0.1 specification](v0.1.md) and [README](../README.md). Later phases describe the planned development order; they do not indicate completed work or committed release dates.

## Progress Overview

> The project is currently in the **CLI foundation phase**: the command skeleton is implemented, but it cannot yet manage Node.js versions.

| Metric | Current Status |
| --- | --- |
| Package version | `0.1.0` (does not indicate a published release) |
| Current milestone | M1 CLI foundation: implementation is in place; validation results have yet to be recorded |
| Core features implemented | **0 / 5**: install, uninstall, switch, list, and current-version queries are not implemented |
| Active development tasks | No active feature tasks are currently recorded |
| Next step | M2: define target parsing rules, local storage layout, and the version-switching mechanism |
| Open decisions | Whether to support partial versions (such as `node@22`), and how to switch versions on each platform |
| Validation status | CLI integration test code exists; checks were not run during this update, and no validation results are recorded yet |

Implementation counts reflect features users can actually use; placeholder commands do not count as implemented. Phases differ in effort, so task counts are not used to estimate an overall project completion percentage.

## Feature Status

| Capability | Implementation Status | Current Behavior | Code / Test Evidence |
| --- | --- | --- | --- |
| Help, version, and argument-count constraints | Implemented | Provides help and version output; invalid CLI usage exits with code `2` | [CLI definitions](../src/cli.rs), [CLI tests](../tests/cli.rs) |
| Target parsing | Not implemented | Accepts targets as raw strings without validating the tool or version format | [CLI definitions](../src/cli.rs), [Deferred parsing tests](../tests/cli.rs) |
| `install` | Placeholder only | Returns `not implemented` with exit code `1` | [Command dispatch](../src/lib.rs), [Exit handling](../src/main.rs) |
| `uninstall` | Placeholder only | Same as above; does not remove any installations | [Command dispatch](../src/lib.rs) |
| `use` | Placeholder only | Same as above; does not switch versions | [Command dispatch](../src/lib.rs) |
| `list` | Placeholder only | Same as above; does not read the list of installed versions | [Command dispatch](../src/lib.rs) |
| `current` | Placeholder only | Same as above; does not query the current version | [Command dispatch](../src/lib.rs) |

“Implemented” only means that the code exists. Whether acceptance criteria have been met is recorded separately in the milestones and progress log.

## Development Roadmap

Milestone statuses: Not started → In progress → Awaiting validation → Complete. If work cannot proceed, mark it as “Blocked” and record the cause and the conditions for unblocking it. Mark work as “In progress” only when it has actually started.

| Milestone | Sequence | Status | Checklist Progress | Completion Criteria |
| --- | --- | --- | --- | --- |
| M1 CLI foundation | Current | Awaiting validation | 5 / 6 | Help, version, argument constraints, and placeholder behavior match the documentation and pass validation |
| M2 Target parsing and local state | Next | Not started | 0 / 6 | Valid targets can be parsed, invalid targets are rejected, and local state read/write rules are defined |
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

- [ ] Define tool names, version formats, and error-message rules.
- [ ] Decide whether to support partial versions such as `node@22`, and define their resolution rules.
- [ ] Implement parsing and validation for `<tool>@<version>`.
- [ ] Define the storage layout for installation directories, temporary directories, and current-version state.
- [ ] Define the version-switching mechanism and platform-specific differences.
- [ ] Add tests for parsing, path boundaries, and local state behavior.

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

Append an entry for each subsequent update, noting the actual changes, validation results, and next steps. Include commit or issue links when available.

## Scope Constraints

Implement version management only. Node.js is the first target for end-to-end support; no other providers are currently planned. Keep a single crate, use stable Rust, prefer the standard library, isolate platform differences, and add dependencies only for actual requirements.

The scope excludes extension systems, task runners, environment-variable management, dotenv, secret management, shell scripts, telemetry, async runtimes, and GUIs.

## Design References

- [GitHub Projects best practices](https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/best-practices-for-projects): clarify dependencies, keep statuses updated, and link to actual work.
- [GitHub public roadmap](https://github.com/github/roadmap): distinguish feature stages and provide corresponding changelog entries upon delivery.
- [Atlassian agile roadmaps](https://www.atlassian.com/agile/product-management/roadmaps): organize the roadmap around goals and adjust it as work progresses.

This project uses a Markdown overview and checklists suited to its current size, without introducing an additional project-management system for now.
