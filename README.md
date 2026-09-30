# Verslot

A minimal, extensible tool version manager written in Rust.

> Version management. Nothing else.

Verslot is currently under development.

## Current implementation

The current package version is `0.3.0`. This version implements shared target
validation, storage path boundaries, and internal current-version state reads.
The working tree also contains v0.3 T1–T7: verified Node.js installation,
safe extraction, OS locking, rename commit, failure cleanup, read-only
complete-installation listing, protected uninstallation, and offline workflow
tests. M3 local Windows acceptance passed (7 / 7 tasks): four required checks,
120 tests and the official Node.js 22.0.0 smoke workflow. The repaired
Windows/Linux/macOS CI matrix also passed before this metadata-only version
bump; official-distribution smoke evidence remains Windows x86_64 only.
The working tree now also contains M4 T1–T5: version switching, complete
selected-version queries, recovery/locking tests and acceptance preparation.
M4 is **Complete under local Windows acceptance**: four required checks,
160 tests and the official Node.js 22.0.0 / 24.0.0 switching/PATH/uninstall
smoke passed on Windows x86_64 / NTFS. Initial failures, repairs and final
evidence are recorded in [M4 validation](docs/m4-validation.md). Unix switching
and other architectures remain unverified; cross-platform delivery remains M5.
See the [v0.4 design and acceptance mapping](docs/v0.4.md),
[M4 validation results](docs/m4-validation.md), [v0.3 design](docs/v0.3.md),
[v0.2 specification](docs/v0.2.md), and
[roadmap](docs/roadmap.md) for scope and progress.

```text
verslot --version
verslot --help
verslot install <tool>@<version>
verslot uninstall <tool>@<version>
verslot use <tool>@<version>
verslot list
verslot current
```

`install`, `uninstall`, and `use` require exactly one target in the form
`node@major.minor.patch`, such as `node@22.0.0`. Only full versions with three
ASCII decimal components in the `u32` range are accepted; leading zeros,
partial versions, aliases, prereleases, and build metadata are rejected.
`list` and `current` accept no positional arguments.

Help and version requests exit successfully. `install node@22.0.0` downloads the
official distribution for this build's platform, verifies SHA-256, and commits
a complete installation. Success prints `installed node@22.0.0`; a complete
duplicate prints `already installed node@22.0.0` without downloading. Existing
incomplete installations are preserved and rejected. Installing does not
select a version or configure PATH. Operational failures exit 1 with errors on
stderr and empty stdout; a committed installation is preserved if temporary
cleanup fails, and the error reports the leftover path.

`list` prints one complete installation per line, such as `node@22.0.0`, sorted
numerically by major, minor, and patch. Missing or empty storage returns empty
stdout without creating directories. Noncanonical names are ignored; incomplete
canonical entries, I/O failures, and unsafe boundaries report errors without
partial stdout. Listing does not inspect current state, acquire the mutation
lock, access the network, or execute Node.js.

`uninstall node@22.0.0` removes a complete inactive installation under the shared
mutation lock and prints `uninstalled node@22.0.0`. A selected version, invalid
current state, incomplete installation, or missing target is rejected. The
installation is first renamed into a private operation directory; rename failure
preserves the original installation. If deletion then fails, the version is
already detached and the error reports the remaining cleanup path. Current state,
other installations, and orphan operations are preserved; cleanup is not retried
automatically and partially deleted installations are not restored.

`use node@22.0.0` selects an already complete installation and prints
`using node@22.0.0`. Repeating it prints `already using node@22.0.0` without
replacing the link. Missing/incomplete targets and invalid existing selection
are errors; use never downloads or repairs installations. `current` prints the
selected target, such as `node@22.0.0`, or empty stdout when none is selected.
It checks the installation under a shared lock, creates nothing, and reports
Verslot's selection even if PATH resolves a different executable.

Install, uninstall and use share a non-blocking exclusive mutation lock;
current uses a non-blocking shared lock. Busy storage returns an error that
asks you to retry after the active operation finishes. Reserved entries
`current/.node-next` or `current/.node-previous` indicate an unfinished switch;
all mutations and current reject them, while list remains available. Inspect
the links and referenced installations before manual recovery. Verslot does
not promote a backup, remove unexplained residue, or provide an unset/repair
command.

Operational errors exit 1 with empty stdout and diagnostics on stderr.
Invalid targets and CLI usage report errors on stderr and exit 2.
The [M3 acceptance results record](docs/m3-validation.md) contains
Windows check/test output, isolated smoke evidence, repairs and platform limitations.

## Select Node.js and configure PATH manually

Install both full versions before switching:

```text
verslot install node@22.0.0
verslot install node@24.0.0
verslot use node@22.0.0
verslot current
verslot use node@24.0.0
verslot current
verslot uninstall node@22.0.0
```

The last current query prints `node@24.0.0`; uninstalling 24.0.0 is rejected
while it is selected. List remains a numerically sorted inventory without
active-version markers. Switching preserves installation receipts and payloads.

Prepend the fixed directory to PATH once. These examples change only the
current shell's environment; Verslot does not edit PATH or shell profiles.

| Platform | Storage root | Fixed executable | PATH directory |
| --- | --- | --- | --- |
| Windows | `%LOCALAPPDATA%\verslot` | `%LOCALAPPDATA%\verslot\current\node\node.exe` | `%LOCALAPPDATA%\verslot\current\node` |
| macOS / Linux | `$HOME/.verslot` | `$HOME/.verslot/current/node/bin/node` | `$HOME/.verslot/current/node/bin` |

PowerShell:

```powershell
$env:PATH = "$env:LOCALAPPDATA\verslot\current\node;$env:PATH"
Get-Command node -CommandType Application
node --version
```

Bash:

```bash
export PATH="$HOME/.verslot/current/node/bin:$PATH"
hash -r
command -v node
node --version
```

After the example switch, the expected Node.js output is `v24.0.0`. A competing
executable, shell alias/function or cached command may override resolution;
compare with the fixed executable path if node and current disagree.
Running processes continue using their original executable; selection affects
subsequent launches.

Unix replaces the symlink atomically. Windows uses junction backup/publication
and may briefly leave the fixed entry point absent. Sharing violations,
permissions or antivirus interference can prevent switching or rollback.
After a verified switch, cleanup failure keeps the new selection but exits 1
with `switch completed but cleanup failed` and the residual path; no success
line is printed. Rollback failures report the original error, recovery error
and retained paths. There is no crash recovery or power-loss guarantee, and
cooperative locks do not protect against malicious same-user path races.

The [isolated two-version M4 smoke procedure](docs/m4-validation.md#official-distribution-smoke-procedure)
records direct-entry and controlled-PATH execution separately from routine
offline synthetic tests. It passed on Windows x86_64 with 22.0.0 / 24.0.0;
Unix and other architectures remain unverified.

The official-distribution smoke workflow passed locally on Windows x86_64:

```text
verslot list
verslot install node@22.0.0
verslot list
verslot install node@22.0.0
# Invoke <root>/installs/node/22.0.0/node.exe (Windows) or bin/node (Unix) with --version.
verslot uninstall node@22.0.0
verslot list
```

Use a new isolated HOME/LOCALAPPDATA base for every Verslot process in that
workflow. It accesses the network and explicitly executes the official binary;
routine offline tests do neither. The offline lifecycle test uses a generated
archive, while the CLI workflow test starts with a complete seeded fixture.

M2 local acceptance passed on Windows (44 tests), followed by the repaired
Windows/Linux/macOS CI matrix. That evidence predates M3 and does not validate
its code. The package version does not imply a published release; see the
[M2 validation record](docs/m2-validation-windows.md#pr-ci-repair-2026-09-26).

## Development

Install stable Rust and the native build tools for your platform (Windows,
macOS, or Linux), then run:

```text
cargo run -- --help
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```
