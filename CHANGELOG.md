# Changelog

## Unreleased

## v0.5.0 — 2026-10-04

First source release intended for actual use after M1–M5 focused development.
No precompiled binaries or crates.io publication.

### Changes since v0.4.0

- Fix Unix lock release before successful no-op selection and successful
  switching returns, allowing subsequent operations to observe the released lock.
  This M5 repair is included in v0.5.0; the v0.4.0 tag predates it.
- Complete M5 native acceptance for Windows x86_64 MSVC, macOS arm64 and Linux
  x86_64 GNU, including official Node.js 22.0.0 / 24.0.0 workflows and a security
  review. Retained [M5 evidence](docs/m5-validation.md) is historical validation,
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
