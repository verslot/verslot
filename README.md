# Verslot

A minimal tool version manager written in Rust. Version management. Nothing else.

`v0.6.0` manages official Node.js distributions, including remote release
discovery and local installation, selection and removal.
No precompiled binaries or crates.io package are published.

Unreleased builds add a unified shim directory for Verslot and managed commands.
Run `verslot setup` and configure the single PATH entry described below. The
released `v0.6.0` still uses the Node-specific PATH configuration.

## Install from source

Install Git, stable Rust with Cargo, and native build tools: MSVC C++ build tools
and Windows SDK on Windows, Xcode Command Line Tools on macOS, or a C compiler
and linker on Linux GNU. Cargo needs registry/dependency access. M5 used Rust
1.98.1; an older minimum Rust version is unverified.

These commands work in PowerShell and Bash. Use a new checkout:

```text
git clone --branch v0.6.0 --depth 1 https://github.com/verslot/verslot.git verslot
cargo install --path verslot --locked
verslot --version
verslot --help
```

Expected version: `verslot 0.6.0` (with three leading spaces). Add Cargo's bin
directory to PATH (normally `%USERPROFILE%\.cargo\bin` or `$HOME/.cargo/bin`).
This is separate from the Node.js PATH entry below. Source builds are not signed
binary release artifacts.
See the [Release](https://github.com/verslot/verslot/releases/tag/v0.6.0) and
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

## Commands

| Command | Behavior |
| --- | --- |
| `verslot setup` | Unreleased: create or refresh the unified command entry points and print the PATH directory |
| `verslot install node@<version>` | Download the official archive, verify SHA-256, and install; Unreleased builds also accept a major or major.minor |
| `verslot uninstall node@<version>` | Remove a complete inactive installation |
| `verslot use node@<version>` | Select an installed version; selecting it again is a no-op; Unreleased builds also accept a major or major.minor |
| `verslot list` / `verslot ls` | Print complete installations, sorted numerically; show a Tool/Version table in terminals |
| `verslot list-remote node` / `verslot ls-remote node` | Discover official releases available for this build's platform |
| `verslot current` | Print the selected complete version, or nothing if none is selected |

`uninstall` requires `node@major.minor.patch`. In Unreleased builds, `install` and
`use` also accept `node@major` and `node@major.minor`; v0.6.0 requires full versions.
Version components must be ASCII decimals in the `u32` range, without leading
zeros. Aliases, prereleases and build metadata are rejected.
`list` (alias `ls`) and `current` take no positional arguments.
`verslot --help` and `verslot --version` exit successfully. Operational errors
exit 1 with empty stdout and diagnostics on stderr; invalid CLI usage exits 2.
Management commands do not execute installed Node.js. Running `node`, `npm` or
`npx` through an Unreleased shim executes the selected installation.

Partial `use` targets select the numerically newest installed version in the
requested series:

```text
verslot use node@18       # newest installed 18.x.y
verslot use node@18.1     # newest installed 18.1.y
verslot use node@18.1.0   # exactly 18.1.0
```

Supplied components match exactly: `18.1` does not match `18.10`, and `1` does not
match `18`. This differs from the textual prefixes used by remote discovery.
Results and `current` show the resolved full version. Each invocation checks
local installations again, without downloading. No matches or a corrupt matching
installation produce an error and preserve the selection; corrupt installations
are not silently skipped in favor of an older version.

Partial `install` requests install the numerically newest remotely available
official release in the requested series for this platform:

```text
verslot install node@18       # newest remotely available 18.x.y
verslot install node@18.1     # newest remotely available 18.1.y
verslot install node@18.1.0   # exactly 18.1.0; skip version discovery
```

Partial installation uses the same component matching as `use` and shows
`Resolving <request>` while retrieving a fresh official index. In a terminal,
this spinner appears after about 200 ms, so fast resolution skips the transient
message; redirected output records the stage immediately. It checks the remote
index even when an older matching version is installed. If the resolved full
version is already installed, the command prints `Already installed <full-target>`
without downloading its checksum or archive. Exact duplicate requests skip all
network access and progress output. Failed discovery or no matching release
leaves storage and selection unchanged. Installation does not select the version;
run `use` afterwards to select an installed version.

Every emitted text line reserves two terminal columns for a lightweight symbol,
followed by one space. Successful operations use `✔` and errors use `✘`.
Lines without a symbol, including repeated operations, help, query data, diagnostic
continuations and blank lines, use three leading spaces. Redirected text uses the
same column without ANSI color; completely empty output stays empty.

Successful operations print one sentence: `Installed node@22.0.0`,
`Selected node@22.0.0`, or `Uninstalled node@22.0.0`. Repeated operations print
`Already installed ...` or `Already selected ...`. In interactive terminals,
`list` shows an aligned `Tool`/`Version` table with purple italic headers, cyan
tool names, green for the selected version and gray for other versions. Empty
local queries print `No installed versions` or `No version selected`. Redirected local queries
retain version lines with the blank symbol column and empty output for empty
results. Listing remains available when selection state cannot be read; in that
case no version is highlighted.

Installation progress uses one updating stderr line for fetching checksums,
downloading, verifying and installing, with the resolved complete target in each
stage and in the download bar or byte-only spinner. Interactive terminals use a cyan dot
spinner (`⠋ ⠙ ⠹`). Downloads with a known total show the original 24-character
block bar (`█` filled, `░` remaining), percentage and bytes/total. Unknown or
inconsistent totals use the spinner and downloaded bytes. The spinner keeps
animating while reads wait. Uninstalling uses the same dot spinner.
Progress clears before the final result or error. The spinner occupies the same
two-column symbol field; download bars use a blank symbol field. Redirected output
uses plain stage lines and byte snapshots, throttled to once per second except initial and
final snapshots. Progress reporting is best-effort and does not affect results.

Operational diagnostics start with `error:`, followed by an indented cause when
needed. Missing selection targets and attempts to uninstall the selected version
include a short recovery suggestion. Cleanup failures retain actual operation
state and paths. Result text uses the terminal's default text color; success
symbols are green. Error symbols and
labels are red on terminals; redirected output, nonempty `NO_COLOR`, or
`TERM=dumb` disable color. Terminal lists keep their two-column layout when
color is disabled.

## Upgrade from v0.5.0

The CLI text output contract changes in v0.6.0. Scripts must allow for the symbol
column, three leading spaces on plain data lines and capitalized result wording.
Capture stdout and stderr separately to keep progress out of query data.
Redirect local queries to retain plain lines instead of terminal tables or
empty-query messages. See [migration notes](CHANGELOG.md#migration-from-v050).
Storage layout and PATH configuration are unchanged; no data migration is needed.

## Discover Node.js versions

Remote discovery is available starting with `v0.6.0`:

```text
verslot ls-remote node
verslot ls-remote node@18
verslot ls-remote node@18.1
verslot ls-remote node@18.1.0
verslot ls-remote node@lts
```

`list-remote` and its visible alias `ls-remote` require exactly one query.
`node` lists all formal three-component releases with an official archive for
the current Verslot build's system and architecture, including old releases.
Numeric queries accept one to three ASCII decimal components, each in the `u32`
range without leading zeros. They match the start of the version text:
`node@18.1` includes both `18.1.0` and `18.10.0`, and `node@1` can include
`10.x`, `18.x` and `19.x`.

`node@lts` lists every release carrying an LTS marker in the official index,
including releases from series that have reached end of life. Earlier releases
from the same series without an LTS marker are excluded. Prereleases, nightly
builds, io.js and versions with build metadata are excluded from all queries.

Results are complete, deduplicated and sorted numerically from oldest to newest.
Each line contains `v<version>` with the logger's three leading spaces, in both
terminals and redirected output. Empty results succeed with empty stdout.

During retrieval and processing, stderr shows `Fetching versions` using the
existing dot spinner in terminals, or a single logger-prefixed plain stage line
when redirected. The spinner clears before results or errors. Loading feedback
is best-effort; stdout contains only version data.

Each query fetches the complete official `https://nodejs.org/dist/index.tab`
before filtering; no cache or local installation is required. Responses are
limited to 4 MiB with a 60-second query deadline. TLS verification and the existing
no-redirect policy apply. Fetch or parse failures exit 1 with a reason on stderr
and empty stdout; invalid arguments exit 2. Discovery uses archive availability
metadata; installation still downloads and verifies the actual archive.

## Configure unified command PATH (Unreleased)

From a current source checkout, install with `cargo install --path . --locked`.
Run the newly installed executable's `setup` command, then prepend one directory
to PATH. The examples use Cargo's default installation location; with a custom
Cargo home or `--root`, call the executable from that installation's `bin`.

| Platform | Unified PATH directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\verslot\shims` |
| macOS / Linux | `$HOME/.verslot/shims` |

PowerShell:

```powershell
& "$env:USERPROFILE\.cargo\bin\verslot.exe" setup
$env:PATH = "$env:LOCALAPPDATA\verslot\shims;$env:PATH"
verslot --version
```

Bash:

```bash
"$HOME/.cargo/bin/verslot" setup
export PATH="$HOME/.verslot/shims:$PATH"
hash -r
verslot --version
```

Persist this one directory in your own environment configuration. `setup` does
not modify PATH, the registry or shell profiles. The directory provides `verslot`,
`node`, `npm` and `npx` (native `.exe` entries on Windows); Cargo's bin directory
and tool-specific directories are unnecessary for daily Verslot use. Additional
supported tools will use the same directory. Yarn is a separate follow-up in
[Issue #30](https://github.com/verslot/verslot/issues/30).

Install and select Node as in the quick start, then run `node --version`, `npm
--version` or `npx --version`. Each launch resolves the global selection. Missing
or damaged selections fail on stderr without running a system Node or downloading
anything. npm and npx use the selected Node with that distribution's bundled JS
entry point; a historical distribution without npx reports that missing command
without invalidating Node. Global npm overrides and arbitrary commands installed
with `npm install -g` are outside this shim contract.

Shims inherit arguments, working directory, environment and stdin/stdout/stderr;
tool output has no Verslot prefix. Tool exit status and Ctrl+C cleanup are
preserved. Running tools do not hold the storage lock: `use` affects subsequent
launches while existing processes continue. Explicitly uninstalling an inactive
version does not protect processes still using its installation.

After upgrading, invoke the **newly installed executable by its full path** with
`setup` again to refresh every entry, including `verslot` itself. Updating Cargo's
copy alone leaves the existing shims unchanged. No PATH adjustment is needed.
`setup` verifies an ownership receipt and executable checksums before replacing
an existing shim directory; unknown or modified entries are preserved and cause
an error. Entries persist even after the last Node version is uninstalled.

Setup stages the complete directory, retains the previous directory during
publication and rolls back if publication fails. The explicit refresh may
briefly leave the command directory absent. On Windows, in-use executables can
prevent replacement or cleanup; close related processes and retry from the new
executable. If publication succeeded but cleanup failed, diagnostics say so and
retain `.shims-previous` for a later verified cleanup. An interrupted publication
with only the previous directory remaining requires inspection before retrying.

## Configure Node.js PATH (released v0.6.0)

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
These workflows are retained evidence, not acceptance reruns for v0.6.0.

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
