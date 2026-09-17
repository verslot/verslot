# Verslot

A minimal, extensible tool version manager written in Rust.

> Version management. Nothing else.

Verslot is currently under development.

## v0.1 scope

This release establishes the CLI contract only. Target parsing and provider
behavior, including Node.js installation, are not implemented yet.

```text
verslot --version
verslot --help
verslot install <tool>@<version>
verslot uninstall <tool>@<version>
verslot use <tool>@<version>
verslot list
verslot current
```

`install`, `uninstall`, and `use` require exactly one positional target. The
target is accepted as an opaque string in this release; its format is not
validated. `list` and `current` accept no positional arguments.

Help and version requests exit successfully. The five version-management
commands report `not implemented: ...` on stderr and exit with code 1 without
changing any tool installations. Invalid CLI usage exits with code 2.

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
