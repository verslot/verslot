# Verslot

A minimal tool version manager written in Rust. Version management. Nothing else.

**Supported tools**

- Node.js
- Yarn

## Installation

Verslot is distributed as source. Install Git, stable Rust with Cargo, and the
native build tools for your platform.

### Platforms and known limitations

The Node.js installation and switching workflow was validated on these platforms
for v0.5.0.

| Platform              | Build tools                          |
| --------------------- | ------------------------------------ |
| Windows x86-64 (MSVC) | MSVC C++ Build Tools and Windows SDK |
| macOS arm64           | Xcode Command Line Tools             |
| Linux x86-64 (GNU)    | C compiler and linker                |

For other architectures and limitations, see the
[v0.6.0 platform notes](https://github.com/verslot/verslot/blob/v0.6.0/README.md#platforms-and-known-limitations).

### Install from source

Build the current `main` branch in PowerShell or Bash.

```text
git clone --branch main --depth 1 https://github.com/verslot/verslot.git verslot
cargo install --path verslot --locked
```

### Configure the shared PATH

Run `setup` with the executable Cargo just installed, then prepend the shared
`shims` directory to `PATH`. The examples use Cargo's default install location.

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
sessions. After upgrading, run `setup` again from the newly installed executable
to refresh the shims.

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
| `verslot ls`                         | List installed versions for all tools |
| `verslot current`                    | Show selected versions                |
| `verslot uninstall <tool>@<version>` | Remove an inactive version            |

Run `verslot --help` for the full command list or `verslot <command> --help` for
command-specific options.

## Documentation

- [Changelog and upgrade notes](CHANGELOG.md)
- [Design decisions and historical records](docs/README.md)
- [Contributing guide](CONTRIBUTING.md)
- [Bug reports and feature requests](https://github.com/verslot/verslot/issues)

For the tagged source release, see the
[v0.6.0 installation guide](https://github.com/verslot/verslot/blob/v0.6.0/README.md).

## License

This repository includes the [MIT License](LICENSE).
