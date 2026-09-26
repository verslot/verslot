# Verslot

A minimal, extensible tool version manager written in Rust.

> Version management. Nothing else.

Verslot is currently under development.

## v0.2.0 scope

The current package version is `0.2.0`. This version implements shared target
validation, storage path boundaries, and internal current-version state reads.
The working tree also contains v0.3 T1–T4: verified Node.js installation,
safe extraction, OS locking, rename commit, and failure cleanup. This M3 code
and its tests have not been validated. Uninstallation, switching, and CLI
queries remain unimplemented. See the [v0.3 design](docs/v0.3.md),
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

`uninstall`, `use`, `list`, and `current` still report `not implemented: ...`
on stderr and exit 1. Invalid targets and CLI usage report errors on stderr
and exit 2. M3 validation and real-distribution smoke testing remain Not run.

M2 local acceptance passed on Windows (44 tests). Linux/macOS validation remains
incomplete and does not block M2 acceptance. The package version does not imply
a published release; see the [validation record](docs/m2-validation-windows.md).

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
