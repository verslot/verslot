# Verslot

A minimal tool version manager written in Rust. Version management. Nothing else.

**Supported tools**

- Node.js
- Yarn

## Installation

Verslot is under active development. To build from source, install Git, stable
Rust with Cargo, and the native build tools for your platform.

### Platforms and known limitations

The Node.js installation and switching workflow was validated on these platforms
during M5.

| Platform              | Build tools                          |
| --------------------- | ------------------------------------ |
| Windows x86-64 (MSVC) | MSVC C++ Build Tools and Windows SDK |
| macOS arm64           | Xcode Command Line Tools             |
| Linux x86-64 (GNU)    | C compiler and linker                |

See the [M5 validation record](docs/archive/milestones/m5/m5-validation.md#final-acceptance-on-2026-10-04)
for platform coverage and limitations.

### Install from source

Build the current `main` branch in PowerShell or Bash.

```text
git clone --branch main --depth 1 https://github.com/verslot/verslot.git verslot
cargo install --path verslot --locked
```

### Configure the shared PATH

Run `setup` with the executable Cargo just installed, then prepend the shared
`shims` directory to `PATH`. The examples use Cargo's default install location.

The shims share one Verslot executable through hard links where supported.
`setup` uses copies when hard links are unavailable.

**Windows (PowerShell)**

```powershell
& "$env:USERPROFILE\.cargo\bin\verslot.exe" setup
$env:PATH = "$env:LOCALAPPDATA\verslot\shims;$env:PATH"
verslot --version
```

**macOS or Linux (Bash)**

```bash
"$HOME/.cargo/bin/verslot" setup
export PATH="$HOME/.verslot/shims:$PATH"
hash -r
verslot --version
```

Add the same shim directory to your shell or user environment for future
sessions. Run `setup` from a new build to refresh the shims.

## Quick start

Install and select a version for each tool using `tool@version`. Yarn uses Node.js
from `PATH`, so this example selects Node.js first. Run it outside an existing
Yarn project to see the globally selected Yarn version.

```text
verslot install node@24
verslot use node@24
verslot install yarn@4.9.2
verslot use yarn@4.9.2
node --version
yarn --version
```

Both `install` and `use` accept a full version, a major version such as `24`,
or a `major.minor` series such as `24.1`. `uninstall` requires a full version.

## Common commands

| Command                              | What it does                          |
| ------------------------------------ | ------------------------------------- |
| `verslot ls-remote <tool>`            | List available releases for a tool    |
| `verslot install <tool>@<version>`   | Install a tool version                |
| `verslot use <tool>@<version>`       | Select an installed version           |
| `verslot unuse <tool>`              | Clear a tool's global selection       |
| `verslot ls`                         | List installed versions for all tools |
| `verslot current`                    | Show selected versions                |
| `verslot uninstall <tool>@<version>` | Remove an inactive version            |

Run `verslot --help` for the full command list or `verslot <command> --help` for
command-specific options.

Run `verslot unuse node` or `verslot unuse yarn` before uninstalling a selected
version, including the only installed version. This removes the tool's selection
link, keeping its installed versions, the other tool's selection and shared shims.
Repeating it succeeds, and damaged or missing installations do not prevent it.
Commands for an unselected tool report the existing no-selection error.

## Documentation

- [Changelog](CHANGELOG.md)
- [Design decisions and historical records](docs/README.md)
- [Contributing guide](CONTRIBUTING.md)
- [Bug reports and feature requests](https://github.com/verslot/verslot/issues)

## License

This repository includes the [MIT License](LICENSE).
