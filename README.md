# Verslot

A minimal, extensible tool version manager written in Rust.

> Version management. Nothing else.

Verslot is currently under development.

## v0.2.0 scope

The current package version is `0.2.0`. This version implements shared target
validation, storage path boundaries, and internal current-version state reads.
The working tree also contains v0.3 T1–T7: verified Node.js installation,
safe extraction, OS locking, rename commit, failure cleanup, read-only
complete-installation listing, protected uninstallation, and offline workflow
tests. M3 local Windows acceptance passed (7 / 7 tasks): four required checks,
120 tests and the official Node.js 22.0.0 smoke workflow. Other platforms and
architectures remain unverified for M3. Switching and CLI current-version queries remain unimplemented.
See the [v0.3 design](docs/v0.3.md),
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

`use` and `current` still report `not implemented: ...`
on stderr and exit 1. Invalid targets and CLI usage report errors on stderr
and exit 2. The [acceptance results record](docs/m3-validation.md) contains
Windows check/test output, isolated smoke evidence, repairs and platform limitations.

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
