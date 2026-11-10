# Changelog

## Unreleased

- Build verified precompiled archives for Windows x86-64, macOS Apple Silicon
  and Linux x86-64 GNU from version tags, and prepare a Release draft with
  SHA-256 checksums and installation instructions (#12). The first binary
  release has not yet been published.

## v0.7.1 — 2026-11-07

- Report a missing shim directory on PATH in `verslot doctor` before checking
  version selections, including when no tools are selected.

## v0.7.0 — 2026-11-06

Source release with unified command shims, Yarn version management, and PATH
diagnostics through `verslot doctor`.

### Changes since v0.6.0

- Generate tool shims only for tools with complete local installations (#42).

- Report pending cleanup of previous Windows shims as a warning when the
  requested shim set is already active.

- Add `verslot doctor` for read-only checks of selections, shim paths, PATH
  shadowing, and the Node.js dependency of selected Yarn. Show a short status
  or actionable findings by default and include path details with `--verbose`
  (#38).

- Add `verslot unuse node` and `verslot unuse yarn` to clear a global selection
  while keeping installations and shared shims. Allow damaged or dangling
  selections to be cleared, and point selected-version uninstall errors to
  `unuse` (#37).

- Avoid hard-coded tool lists in argument diagnostics and explain that `unuse`
  accepts a tool name without a version.

- Add independent Yarn version discovery, installation, global selection and
  removal for official Classic `yarn` 1.x and modern `@yarnpkg/cli-dist` stable
  packages. Verify registry SHA-512 before safe tar extraction on every platform.
  Add `yarn` / `yarnpkg` to the unified shims. They use Node from `PATH` and
  leave project version delegation and compatibility checks to Yarn. Show all
  tools in `current` / `list`, with independent selection highlighting (#30).

- Show complete help commands in argument errors, including the relevant
  subcommand, and consistently display the executable name as `verslot`.

- Add `verslot setup` and a single shim PATH directory providing Verslot, Node.js
  and the selected distribution's bundled npm/npx. Preserve native process I/O,
  arguments, exit status and Ctrl+C cleanup; fail without system fallback or
  automatic installation. Refresh entry points with the newly installed
  executable after upgrades; version selection does not rewrite them (#29).

- Display setup PATH instructions using standard Windows drive or UNC notation
  instead of the internal extended-length path prefix (#29).

- Share one executable across command shims with hard links on Windows, Linux,
  and macOS. Running `verslot setup` migrates verified shim directories and
  uses copies when hard links are unavailable (#34).

- Show `Resolving <request>` during partial installation and include the resolved
  complete target in subsequent installation progress. Delay the terminal
  resolution spinner by about 200 ms to avoid flicker, while logging redirected
  resolution immediately (#27).

- Allow `use` and `install` requests with `node@<major>` or `node@<major>.<minor>`.
  Match supplied components exactly and resolve the newest installed version for
  selection or the newest remotely available version for installation. Report
  complete resolved versions, show remote resolution progress for partial
  installation, and retain exact targets for uninstall (#27).

- Release Unix storage locks when an operation ends, including after switch
  rollback (#33, #44).

### Migration from v0.6.0

Run `setup` from the newly installed Verslot executable. Replace the old Node.js
PATH entry with the shared shim directory. The old entry was
`%LOCALAPPDATA%\verslot\current\node` on Windows and
`$HOME/.verslot/current/node/bin` on macOS and Linux.
See [platform-specific setup commands](README.md#configure-the-shared-path).

Rerun `setup` after installing a tool's first version or uninstalling its last
version to synchronize its shims. Scripts that consume redirected `list` or
`current` output should filter by tool prefix, such as `node@`.

## v0.6.0 — 2026-11-02

Source release with remote Node.js version discovery and CLI output changes.
No precompiled binaries or crates.io publication.

### Changes since v0.5.0

- Show a `Fetching versions` spinner on stderr during remote discovery, clear it
  before results or errors, and use a plain stage line when redirected (#21).
- Add `list-remote` with the visible alias `ls-remote` for official Node.js
  release discovery, numeric text-prefix queries and historical LTS filtering.
  Fetch a fresh index for every query, filter by the current build's archive
  support, and print unique `v<version>` lines in numeric ascending order (#21).
- Remove the logger's unchanged-result dot; repeated operations use the empty
  symbol field (#20).
- Add `verslot ls` as a visible alias for `verslot list` (#19).
- Standardize all CLI output through an internal logger with a two-column
  lightweight symbol field and one separating space. Prefix ordinary and
  continuation lines with an empty field, including help, version, queries and
  redirected progress. Preserve existing wording, empty output and exit codes.
- Capitalize CLI results and use `Selected` for version selection. Add terminal
  empty-query messages while keeping redirected
  query output plain. Format operational errors with recovery suggestions,
  keep success results in the default text color, color error labels on supported
  terminals, and honor `NO_COLOR` and `TERM=dumb` for errors and progress.
  Shorten progress stage labels.
- Show terminal `list` output as an aligned `Tool`/`Version` table with colored
  headers, tool names and selected versions. Keep redirected version lines plain.
- Use a cyan block bar for terminal download progress and cyan stage spinners.
- Show a leading spinner during uninstall on stderr, with plain text for
  redirected output, and clear it on success or failure.
- Show installation stages on stderr with an ASCII terminal spinner and plain
  stage lines for redirected output (#15). Preserve final stdout results,
  errors and duplicate-installation behavior.
- Show live archive byte counts before the download completes (#16), including
  responses without a known total. Throttle redirected intermediate byte updates
  to once per second, retain immediate snapshots, and preserve installation
  outcomes when progress output fails.
- Add total-aware download progress with a bar, bytes/total and percentage when
  the response supplies a usable total (#17). Fall back to byte-only output for
  unknown, invalid or inconsistent totals. Use a leading dot spinner, replacing
  the initial ASCII rotation, and keep redirected output plain text.

### Migration from v0.5.0

The CLI text output contract has changed. All emitted lines now reserve a
two-column symbol field and a separating space; plain data, help and version
lines have three leading spaces, including when redirected. Success messages
are capitalized and selection uses `Selected` instead of `using`. Scripts that
compare exact lines must adapt to the prefix and wording. Capture stdout and
stderr separately because progress is now written to stderr.

Interactive `list` output is a table, and empty local queries show a message.
Redirect local queries to retain plain version lines and empty output for no
results. Remote queries always print plain `v<version>` lines and require a
query such as `node`, `node@18.1` or `node@lts`. Installation targets, storage
layout, PATH configuration and exit codes are unchanged; no storage migration
is required.

## v0.5.0 — 2026-10-04

First source release intended for actual use after M1–M5 focused development.
No precompiled binaries or crates.io publication.

### Changes since v0.4.0

- Fix Unix lock release before successful no-op selection and successful
  switching returns, allowing subsequent operations to observe the released lock.
  This M5 repair is included in v0.5.0; the v0.4.0 tag predates it.
- Complete M5 native acceptance for Windows x86_64 MSVC, macOS arm64 and Linux
  x86_64 GNU, including official Node.js 22.0.0 / 24.0.0 workflows and a security
  review. Retained [M5 evidence](docs/archive/milestones/m5/m5-validation.md) is historical validation,
  not a rerun performed for this release.
- Pin source installation to v0.5.0, organize usage and limitations in README,
  and move ongoing maintenance to Issues and CONTRIBUTING.
- Update the package version to 0.5.0 without upgrading dependencies or changing
  CLI arguments/output contracts, storage format or public API.

### Available capabilities and scope

The existing five commands install, uninstall, select, list and query full
Node.js versions. Installation verifies the official archive's SHA-256;
selection uses a fixed junction/symlink entry point with manual PATH setup.
These capabilities were implemented before this release, not all added in 0.5.0.

M5 accepted Windows x86_64 MSVC, macOS arm64 and Linux x86_64 GNU. Windows arm64,
macOS x86_64 and Linux GNU arm64 remain mapped but unverified; musl, 32-bit and
other targets are unsupported. Network, filesystem, Windows locking and manual
recovery limitations remain as documented in [README](README.md#platforms-and-known-limitations).
M1 independent historical acceptance remains unrecorded and is not a release
blocker. Historical design and acceptance conclusions are retained unchanged.
