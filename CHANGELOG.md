# Changelog

## Unreleased

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
