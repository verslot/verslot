# Verslot

A minimal tool version manager written in Rust. Version management. Nothing else.

**Supported tools**

- Node.js
- Yarn

## Installation

Verslot is under active development. Install a precompiled binary when one is
available for your platform, or build from source.

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

### Install a precompiled binary

The first precompiled binary release is pending. When available, choose the
archive and its matching `.sha256` file from the same
[GitHub Release](https://github.com/verslot/verslot/releases). The supported
targets are Windows x86-64 (`x86_64-pc-windows-msvc`), macOS Apple Silicon
(`aarch64-apple-darwin`), and Linux x86-64 GNU (`x86_64-unknown-linux-gnu`).
Check the Release notes for tested OS versions and runtime requirements,
including the required glibc versions on Linux. If a Release has no binary for
your platform, use the source installation below.

Run the following examples in an empty working directory and replace `X.Y.Z`
with the published version. They download and check the archive before
extracting it to a new directory and running `setup`. A checksum detects a
mismatch with the published checksum; it does not independently authenticate
files downloaded from the same source.

**Windows x86-64 (PowerShell)**

```powershell
$version = 'X.Y.Z'
$target = 'x86_64-pc-windows-msvc'
$archive = "verslot-v$version-$target.zip"
$directory = Join-Path (Get-Location) "verslot-v$version-$target"
$base = "https://github.com/verslot/verslot/releases/download/v$version"
& {
    $ErrorActionPreference = 'Stop'
    Invoke-WebRequest "$base/$archive" -OutFile $archive
    Invoke-WebRequest "$base/$archive.sha256" -OutFile "$archive.sha256"
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-Content -Raw -LiteralPath "$archive.sha256").Trim() -cne "$digest  $archive") {
        throw 'Checksum verification failed'
    }
    New-Item -ItemType Directory -Path $directory | Out-Null
    Expand-Archive -LiteralPath $archive -DestinationPath $directory
    & (Join-Path $directory 'verslot.exe') setup
    if ($LASTEXITCODE -ne 0) { throw 'Setup failed' }
}
```

**macOS Apple Silicon or Linux x86-64 GNU (Bash)**

```bash
version=X.Y.Z
target=aarch64-apple-darwin  # On Linux use x86_64-unknown-linux-gnu
archive="verslot-v$version-$target.tar.gz"
directory="$PWD/verslot-v$version-$target"
base="https://github.com/verslot/verslot/releases/download/v$version"
curl -fL "$base/$archive" -o "$archive" &&
curl -fL "$base/$archive.sha256" -o "$archive.sha256" &&
if [ "$target" = aarch64-apple-darwin ]; then
  actual=$(shasum -a 256 "$archive")
else
  actual=$(sha256sum "$archive")
fi &&
expected=$(cat "$archive.sha256") &&
if [ "$actual" != "$expected" ]; then
  echo 'Checksum verification failed' >&2
  false
else
  mkdir "$directory" &&
  tar -xzf "$archive" -C "$directory" &&
  "$directory/verslot" setup
fi
```

Keep the extracted executable: use it when `setup` must refresh the shims. For
an upgrade, download and verify the new release and run `setup` from its newly
extracted executable. Then configure the shared PATH below.

### Install from source

Install Git, stable Rust with Cargo, and the native build tools listed above.
Build the current `main` branch in PowerShell or Bash.

```text
git clone --branch main --depth 1 https://github.com/verslot/verslot.git verslot
cargo install --path verslot --locked
```

For upgrades from v0.6.0, follow the
[v0.7.0 migration steps](CHANGELOG.md#migration-from-v060).

Run `setup` with the executable Cargo just installed:

```powershell
& "$env:USERPROFILE\.cargo\bin\verslot.exe" setup
```

```bash
"$HOME/.cargo/bin/verslot" setup
```

### Configure the shared PATH

After running `setup`, prepend the shared `shims` directory to `PATH`.

`setup` always provides the `verslot` entry point. It also creates shims for each
tool with at least one complete local installation, even before a version is
selected.

The shims share one Verslot executable through hard links where supported.
`setup` uses copies when hard links are unavailable.

**Windows (PowerShell)**

```powershell
$env:PATH = "$env:LOCALAPPDATA\verslot\shims;$env:PATH"
verslot --version
```

**macOS or Linux (Bash)**

```bash
export PATH="$HOME/.verslot/shims:$PATH"
hash -r
verslot --version
```

Add the same shim directory to your shell or user environment for future
sessions. Run `setup` from a new build to refresh the shims.

On Windows, a cleanup warning means the requested shims are active and a file
in the previous shim directory is still pending removal. Close the related
processes and rerun `setup` to finish cleanup. A refresh that needs to replace
the active shims can proceed once cleanup succeeds.

## Quick start

Install and select a version for each tool using `tool@version`. Yarn uses Node.js
from `PATH`, so this example selects Node.js first. Run it outside an existing
Yarn project to see the globally selected Yarn version.

```text
verslot install node@24
verslot use node@24
verslot install yarn@4.9.2
verslot use yarn@4.9.2
```

After installing the first version of a tool, rerun `setup` using the installed
executable from the installation steps above to add that tool's shims. On macOS
or Linux, run `hash -r` afterward if your shell caches command paths.

Check the selected versions through the shared PATH.

```text
node --version
yarn --version
```

After uninstalling a tool's last version, run the same `setup` command to remove
its shims. The shell then uses ordinary PATH lookup for those commands.

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
| `verslot doctor`                     | Diagnose selections and command paths |
| `verslot uninstall <tool>@<version>` | Remove an inactive version            |

Run `verslot --help` for the full command list or `verslot <command> --help` for
command-specific options.

Run `verslot unuse node` or `verslot unuse yarn` before uninstalling a selected
version, including the only installed version. This removes the tool's selection
link, keeping its installed versions, the other tool's selection and shared shims.
Repeating it succeeds, and damaged or missing installations do not prevent it.
A tool shim reports an error when its global selection is missing.

## Diagnose selections and PATH

Run `verslot doctor` for a short status or findings with recovery steps.
Use `verslot doctor --verbose` to include the storage and shim directories, each
tool's selection, the first PATH match for `verslot`, `node`, `npm`, `npx`,
`yarn`, and `yarnpkg`, and the Node.js path used by Yarn.

Doctor checks the shim directory and its presence on the current process's PATH
before inspecting selections. If the directory exists but is missing from PATH,
add the reported directory to PATH and open a new terminal.

When the shim directory or a required shim is missing, run `verslot setup` from
your installed Verslot executable. If setup refuses an existing shim directory,
inspect it and move it aside before retrying.

PATH results are static. On Windows the search uses PATH directory order and
`PATHEXT` extension order, with `.exe` as the fallback when no extensions are
configured. On Unix empty and relative PATH entries are relative to the current
directory, and candidates must have an execute bit. Shell aliases, functions,
command caches, and PowerShell-specific `.ps1` rules are outside this search.
Yarn uses its own Node.js lookup (`node.exe` on Windows). A system Node.js can
satisfy this check. A Verslot Node.js shim needs a valid Node.js selection.
On Unix the Yarn dependency check is unavailable when PATH is unset.

Both reports are read-only and go to stdout as plain text. Save the full report
with `verslot doctor --verbose > doctor.txt`. An unconfigured or healthy
environment exits `0`. Errors or checks that could not be completed exit `1`,
and invalid usage exits `2`.

## Documentation

- [Changelog](CHANGELOG.md)
- [Design decisions and historical records](docs/README.md)
- [Contributing guide](CONTRIBUTING.md)
- [Bug reports and feature requests](https://github.com/verslot/verslot/issues)

## License

This repository includes the [MIT License](LICENSE).
