# Documentation

## Current development

- [README](../README.md): installation, commands, supported platforms and limitations.
- [AGENTS.md](../AGENTS.md): rules for AI coding agents.
- [CONTRIBUTING](../CONTRIBUTING.md): the sole ongoing maintenance and release procedure.
- [GitHub Issues](https://github.com/verslot/verslot/issues): the only backlog for new work.
- [CHANGELOG](../CHANGELOG.md): user-visible changes by release.

Read the current guidance first, then the design sections relevant to the task.
Historical task lists are not the current backlog, and historical validation
does not establish that the current source has passed checks.

## Frozen milestone records

The [M1–M5 roadmap](archive/milestones/roadmap.md), designs and acceptance records
are frozen history. Archiving changes their location, not their conclusions.
Design contracts remain reference material until explicitly superseded; consult
the relevant sections alongside current code and tests.

| Milestone | Design | Validation and evidence |
| --- | --- | --- |
| M1: CLI foundation | [v0.1](archive/milestones/m1/v0.1.md) | Independent historical acceptance is unrecorded |
| M2: target parsing and local state | [v0.2](archive/milestones/m2/v0.2.md) | [Windows validation](archive/milestones/m2/m2-validation-windows.md) |
| M3: installation, listing and uninstallation | [v0.3](archive/milestones/m3/v0.3.md) | [Validation](archive/milestones/m3/m3-validation.md), [evidence](archive/milestones/m3/evidence/) |
| M4: switching and current-version queries | [v0.4](archive/milestones/m4/v0.4.md) | [Validation](archive/milestones/m4/m4-validation.md), [T1–T3 validation](archive/milestones/m4/m4-t1-t3-validation.md), [evidence](archive/milestones/m4/evidence/) |
| M5: cross-platform delivery | [v0.5](archive/milestones/m5/v0.5.md) | [Validation](archive/milestones/m5/m5-validation.md), [macOS evidence](archive/milestones/m5/evidence/m5-macos-37205835491/) |

For storage and target rules, start with [M2 local state](archive/milestones/m2/v0.2.md#local-state)
and [target format](archive/milestones/m2/v0.2.md#target-format). For installation
integrity, see [M3 completeness](archive/milestones/m3/v0.3.md#storage-and-complete-installations).
For switching, locking and recovery, see [M4](archive/milestones/m4/v0.4.md).

Raw logs, checksums, environment records, screenshots and smoke scripts are
retained unchanged beside their milestones. Scripts preserve historical paths
and execution assumptions; they are evidence, not the current validation procedure.
Use CONTRIBUTING for new work and release validation.
