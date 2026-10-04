# Verslot

A minimal, extensible tool version manager written in Rust.

> Version management. Nothing else.

Verslot is currently under development.

## Current implementation

The current package version is `0.4.0`; no Verslot binaries have been
published. All five Node.js commands are implemented. M5 records native
Windows x86_64, macOS arm64 and Linux x86_64 GNU checks and official Node.js
22.0.0 / 24.0.0 workflows, plus a security review with no recorded release
blocker. M5 is Complete for the minimum three-platform scope after final-source macOS acceptance on 2026-10-04. See [M5 evidence](docs/m5-validation.md),
[v0.5 design](docs/v0.5.md), and [roadmap](docs/roadmap.md).

## Install Verslot from source

Prerequisites: Git, stable Rust with Cargo, and native build tools: MSVC C++
build tools and a Windows SDK on Windows, Xcode Command Line Tools on macOS,
or a C compiler/linker on Linux GNU. Cargo needs registry/dependency access.
The native runs used Rust 1.98.1; an older minimum Rust version is unverified.

Use a new checkout pinned to the Linux validation commit, which includes the
Unix lock-release repair reviewed in T4. The `v0.4.0` tag predates that repair.
These commands work in PowerShell and Bash:

```text
git clone https://github.com/verslot/verslot.git verslot
git -C verslot checkout --detach 04c17a42b6f8099e0ef0ba13951abdd4511b201a
cargo install --path verslot --locked
verslot --version
verslot --help
```

Cargo installs into its bin directory (normally `%USERPROFILE%\.cargo\bin`
or `$HOME/.cargo/bin`). Put that directory on PATH or invoke `verslot.exe`
or `verslot` by its full path. This is separate from the Node.js PATH entry
below. The binary reports `verslot 0.4.0`; local builds are not published or
signed release artifacts. Final acceptance covers this pinned source through source-equivalent native evidence. The [final decision](docs/m5-validation.md#final-acceptance-on-2026-10-04) retains package version `0.4.0` and confirms the minimum three-target scope; no new tag or binaries were published.

## Platform evidence and known limitations

| Build platform | Evidence status | Recorded environment |
| --- | --- | --- |
| Windows x86_64 MSVC | Validated through candidate CI and retained M4 workflow | Windows / NTFS; [T1](docs/m5-validation.md#t1-windows-delivery-validation) |
| macOS arm64 | Validated | macOS 15.7.9 / case-insensitive APFS; [T2](docs/m5-validation.md#t2-macos-delivery-validation) |
| Linux x86_64 GNU | Validated | GitHub-hosted Ubuntu 24.04 / non-root glibc; [T3](docs/m5-validation.md#t3-linux-delivery-validation) |
| Windows arm64, macOS x86_64, Linux GNU arm64 | Mapped but unverified | Archive URL mapping only; no native acceptance |
| Linux musl, 32-bit targets, other operating systems/architectures | Unsupported | Installation fails without fallback |

Evidence covers the recorded environments and Node.js versions. Other
filesystems, case-sensitive APFS, other macOS/Linux versions and mount policies
remain unverified. Storage needs write permission and native junction/symlink
support; Unix executables need execute permission. Windows antivirus, endpoint
protection and file sharing can affect mutations. The macOS non-UTF-8 on-disk
fixture is excluded; native-byte tests do not replace filesystem coverage.
Windows candidate CI test totals were unavailable; retained M4 had 160 tests,
macOS 163, and Linux 164. See the linked evidence for environment details.

Installation accesses fixed official `https://nodejs.org/dist/` archive and
`SHASUMS256.txt` URLs. TLS verification remains enabled; redirects and content
decoding are disabled. The exact archive SHA-256 must match before extraction.
Downloads are bounded to 1 MiB of checksums, a 512 MiB archive and a 15-minute
overall deadline. Missing artifacts, network restrictions, timeouts and
certificate failures return errors. Custom mirrors and proxy configuration
are outside the supported contract; corporate network compatibility is
unverified. Retained Windows Cargo resolution failures did not disable TLS.

Checksums establish agreement with official distribution data, not independent
publisher authenticity. Executing installed Node.js trusts that official code;
Verslot does not execute it during its five commands. Receipts are not ongoing
integrity monitoring after installation. The [T4 review and advisory scan](docs/m5-validation.md#t4-security-review)
are point-in-time evidence, not a guarantee about future vulnerabilities.

## Commands

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
M5 also passed the same A/B workflow on macOS arm64 and Linux x86_64 GNU; other mapped architectures remain unverified. See [M5 evidence](docs/m5-validation.md).

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
