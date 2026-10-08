# Verslot

A minimal tool version manager written in Rust. Version management. Nothing else.

`v0.5.0` is the first source release intended for actual use, following M1–M5
focused development. It manages official Node.js distributions with five commands.
No precompiled binaries or crates.io package are published.

## Install from source

Install Git, stable Rust with Cargo, and native build tools: MSVC C++ build tools
and Windows SDK on Windows, Xcode Command Line Tools on macOS, or a C compiler
and linker on Linux GNU. Cargo needs registry/dependency access. M5 used Rust
1.98.1; an older minimum Rust version is unverified.

These commands work in PowerShell and Bash. Use a new checkout:

```text
git clone --branch v0.5.0 --depth 1 https://github.com/verslot/verslot.git verslot
cargo install --path verslot --locked
verslot --version
verslot --help
```

Expected version: `verslot 0.5.0`. Add Cargo's bin directory to PATH (normally
`%USERPROFILE%\.cargo\bin` or `$HOME/.cargo/bin`). This is separate from the
Node.js PATH entry below. Source builds are not signed binary release artifacts.
See the [Release](https://github.com/verslot/verslot/releases/tag/v0.5.0) and
[CHANGELOG](CHANGELOG.md).

## Quick start

```text
verslot install node@22.0.0
verslot install node@24.0.0
verslot list
verslot use node@22.0.0
verslot current
verslot use node@24.0.0
verslot current
verslot uninstall node@22.0.0
```

The final selection is `node@24.0.0`. Configure PATH below to run it as `node`.
Installing does not select a version; switching requires an installed version.
A selected version cannot be uninstalled.

## Five commands

| Command | Behavior |
| --- | --- |
| `verslot install node@<version>` | Download the official archive, verify SHA-256, and install; complete duplicates are a no-op |
| `verslot uninstall node@<version>` | Remove a complete inactive installation |
| `verslot use node@<version>` | Select an installed version; selecting it again is a no-op |
| `verslot list` | Print complete installations, sorted numerically; empty storage prints nothing |
| `verslot current` | Print the selected complete version, or nothing if none is selected |

Only `node@major.minor.patch` is accepted, with three ASCII decimal components
in the `u32` range. Leading zeros, partial versions, aliases, prereleases and
build metadata are rejected. `list` and `current` take no positional arguments.
`verslot --help` and `verslot --version` exit successfully. Operational errors
exit 1 with empty stdout and diagnostics on stderr; invalid CLI usage exits 2.
The five commands do not execute installed Node.js.

## Configure Node.js PATH

Prepend the fixed directory once. Verslot does not edit PATH or shell profiles.
These examples affect the current shell only; persist the same entry in your
own shell configuration if desired.

| Platform | Storage root | Node.js PATH directory |
| --- | --- | --- |
| Windows | `%LOCALAPPDATA%\verslot` | `%LOCALAPPDATA%\verslot\current\node` |
| macOS / Linux | `$HOME/.verslot` | `$HOME/.verslot/current/node/bin` |

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

After the quick start, expect `v24.0.0`. Other executables, aliases, functions or
shell caches can override resolution; compare with the fixed executable path
if `node --version` disagrees with `verslot current`. Running processes retain
their original executable; selection affects subsequent launches.

## Platforms and known limitations

M5 native acceptance covers Windows x86_64 MSVC, macOS arm64 and Linux x86_64
GNU with official Node.js 22.0.0 / 24.0.0 workflows. See the historical
[M5 evidence and final decision](docs/archive/milestones/m5/m5-validation.md#final-acceptance-on-2026-10-04).
These workflows are retained evidence, not new v0.5.0 release runs.

| Platform | Status |
| --- | --- |
| Windows x86_64 MSVC | M5 accepted on Windows / NTFS |
| macOS arm64 | M5 accepted on macOS 15.7.9 / case-insensitive APFS |
| Linux x86_64 GNU | M5 accepted on Ubuntu 24.04 / non-root glibc |
| Windows arm64, macOS x86_64, Linux GNU arm64 | Archive mapping exists; native acceptance unverified |
| Linux musl, 32-bit targets, other systems/architectures | Unsupported; no fallback |

- Other filesystems, case-sensitive APFS, OS versions and mount policies remain
  unverified. Storage needs write permission and junction/symlink support;
  Unix executables need execute permission. The macOS non-UTF-8 on-disk fixture
  is excluded; native-byte tests do not replace filesystem coverage.
- Downloads use fixed official `https://nodejs.org/dist/` archives and
  `SHASUMS256.txt`, with TLS verification and exact archive SHA-256 matching.
  Redirects and content decoding are disabled. Limits are 1 MiB of checksums,
  512 MiB per archive and a 15-minute overall deadline. Mirrors and proxy
  configuration are outside the supported contract; corporate networks are
  unverified. Missing artifacts and network/certificate failures return errors.
- Checksums establish agreement with official distribution data, not independent
  publisher authenticity. Executing Node.js trusts that code. Receipts do not
  monitor integrity after installation. The [M5 security review](docs/archive/milestones/m5/m5-validation.md#t4-security-review)
  is point-in-time evidence.
- Mutations use a nonblocking exclusive lock; `current` uses a shared lock.
  Busy storage asks you to retry. Incomplete installations and invalid selection
  are preserved and rejected. Reserved `.node-next` / `.node-previous` entries
  require manual inspection; there is no unset, automatic repair or crash recovery.
- Unix replaces the selection symlink atomically. Windows junction publication
  may briefly leave the entry point absent; sharing violations, permissions and
  antivirus can interfere. Cleanup failure after a committed operation returns
  an error and retained paths; a completed switch keeps its new selection.
  Detached uninstall leftovers are not automatically restored. Power-loss safety
  and malicious same-user path races are outside the guarantee.

## Maintenance and historical records

Report bugs and scoped improvements through [GitHub Issues](https://github.com/verslot/verslot/issues).
[CONTRIBUTING](CONTRIBUTING.md) is the sole ongoing maintenance and release
procedure. [CHANGELOG](CHANGELOG.md) records user-visible changes.

See the [documentation index](docs/README.md) for reading guidance and archived records.

The [roadmap](docs/archive/milestones/roadmap.md) and M1–M5 design/acceptance records are frozen
history. M1's independent historical acceptance remains unrecorded and does
not block this release. See [M2](docs/archive/milestones/m2/m2-validation-windows.md),
[M3](docs/archive/milestones/m3/m3-validation.md), [M4](docs/archive/milestones/m4/m4-validation.md) and
[M5](docs/archive/milestones/m5/m5-validation.md) for detailed evidence and contracts.
