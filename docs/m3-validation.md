# M3 Validation Preparation and Results

Last updated: 2026-09-27

M3 implementation tasks T1–T7 have code, test code, and documentation. The
milestone is **Complete under local Windows acceptance**. Acceptance ran with
package `0.2.0`; the separately requested metadata-only bump now sets `0.3.0`.
No release was published. The checks and official-distribution
smoke workflow passed on 2026-09-27 after the repairs recorded below. Unix and
other architectures lack official-distribution smoke evidence for M3. Repaired
Windows/Linux/macOS CI passed subsequently, as recorded below.

## Required checks

Run these separately after implementation and record actual output. A failed
check reopens the affected task; record the fix and subsequent results without
overwriting the earlier failure evidence.

```text
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

| Evidence | Current result |
| --- | --- |
| Acceptance date and source revision | 2026-09-27; working tree based on `2fcf04e6794fe243f26525fead5aa18410934a81`, including T5–T7 and acceptance repairs; not a clean committed revision |
| OS, architecture, and Rust version | Windows; x86_64-pc-windows-msvc; rustc 1.98.1 (48a229cea 2026-09-01) |
| `cargo fmt --check` | Passed after formatting repairs; [log](m3-validation-logs/fmt.txt) is empty on success |
| `cargo check --all-targets` | Passed; [log](m3-validation-logs/check.txt) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed; [log](m3-validation-logs/clippy.txt) |
| `cargo test --all` | Passed: 100 library + 20 CLI tests, 0 failed/ignored; main/doc targets have 0 tests; [log](m3-validation-logs/test.txt) |
| Official-distribution smoke workflow | Passed on Windows x86_64 with Node.js 22.0.0; [log](m3-validation-logs/smoke.txt) |
| Acceptance decision | Local M3 acceptance passed; M3 Complete; other platforms explicitly unverified |

The [v0.3 acceptance mapping](v0.3.md#acceptance-mapping) links actual test
files and names. Passing evidence applies to the native Windows cases compiled
in this run; cfg-excluded Unix fixtures are not skipped tests or passing results.

## Initial failures and repairs

The initial fmt check failed on M3 formatting differences; `cargo fmt` repaired
them. The initial [check log](m3-validation-logs/check-initial.txt) shows two
SHA-256 fixture formatting errors with sha2 0.11; fixture digests now format
each byte explicitly. The initial [Clippy log](m3-validation-logs/clippy-initial.txt)
flagged permission restoration in a Windows fixture.

The initial [test run](m3-validation-logs/test-initial.txt) had 91 library passes
and 9 failures; CLI tests had not run because the library target failed:

- Accepted sockets inherited nonblocking mode on Windows. Both local HTTP
  fixtures now explicitly select blocking mode while retaining bounded socket
  timeouts; the existing transport and stalled-body tests passed afterward.
- Windows directory creation errors prevented the intended lock-file type
  diagnostic. `acquire_lock` now inspects an existing lock entry before
  exclusive creation, retaining the later recheck. The existing lock-directory
  and link rejection regression tests passed afterward.
- Windows locks prevent reading the lock file while held. The content
  preservation fixture reads it after unlocking, while still asserting
  contention under the lock.
- Modern Windows `remove_file` removed the read-only payload, so that fixture
  did not demonstrate deletion failure. The replacement opens the already
  detached payload exclusively and invokes real operation cleanup, asserting
  a native sharing failure, detached state, leftover contents, and subsequent
  cleanup. Its name is `windows_locked_detached_payload_reports_native_cleanup_failure`.

All four checks were rerun on the repaired source; the final tests passed.
The first smoke run installed successfully, but its extra PowerShell checksum
fetch failed during sandbox TLS initialization. Fetching the same official
SHASUMS256.txt outside the sandbox succeeded without disabling TLS validation;
the [saved official checksum file](m3-validation-logs/SHASUMS256.txt) was used
for receipt comparison in a fresh complete rerun. Production install itself
uses Rust TLS and succeeded inside the sandbox. The first smoke fixture and
[partial log](m3-validation-logs/smoke-initial.txt) remain isolated under target
for evidence; no user installation or selection was modified.

## Offline workflow scope

### Subsequent GitHub CI

[PR #3](https://github.com/verslot/verslot/pull/3) triggered the three-platform
matrix. The initial `ed6401a` run failed on Unix-only Clippy warnings. Commit
`59318c9` replaced manual modulo testing with `is_multiple_of`, collapsed the
tar NUL-suffix conditional, and used byte string fixtures without changing
archive behavior. The [repaired CI run](https://github.com/verslot/verslot/actions/runs/36259879124)
passed fmt and Windows/Linux/macOS check, Clippy and test jobs. This evidence
predates the metadata-only `0.3.0` bump; it does not establish a real Unix
official-distribution smoke workflow or support for untested architectures.

- `install::tests::offline_install_list_duplicate_uninstall_workflow_is_isolated`
  invokes `offline_workflow_child` with isolated HOME and LOCALAPPDATA. The
  private install preparation seam supplies a generated native archive and its
  SHA-256 digest. Real extraction, completeness, locking, commit, list,
  duplicate install, and uninstall run together. A corrupt archive leaves an
  empty list; successful cleanup preserves unrelated orphan operations.
- `offline_cli_duplicate_list_uninstall_workflow_returns_empty_inventory` in
  [tests/cli.rs](../tests/cli.rs) seeds a complete fixture and asserts exact
  stdout/stderr and exit success for list, duplicate install, uninstall, and an
  empty final list. It checks duplicate content preservation and no current
  state creation. It does not exercise a fresh production CLI download.
- [Download tests](../src/download/tests.rs) independently exercise transport,
  strict checksums, streaming verification, timeout/size bounds, and HTTP errors
  with local HTTP or injected readers. No production mirror or URL override is
  added. Routine fixtures never execute Node.js or contact nodejs.org.

The Windows offline tests and the complete real HTTPS CLI workflow both passed.

## Official-distribution smoke procedure

1. Record the acceptance platform, source revision, Rust version, and one full
   Node.js version (for example `22.0.0`). Use a newly created temporary base;
   set both HOME and LOCALAPPDATA to that base for every Verslot process.
   Restore any shell environment changes afterward. Never use the normal
   per-user installation root or a pre-existing smoke directory.
2. Run `verslot list`: require exit 0, empty stdout/stderr, and no created
   storage root. Native storage is `<base>/verslot` on Windows and
   `<base>/.verslot` on Unix.
3. Run `verslot install node@22.0.0`: require exit 0, exactly
   `installed node@22.0.0` plus newline, and empty stderr. Record the exact
   official archive URL and matching SHA-256 from `SHASUMS256.txt`; confirm
   that the installation receipt records the same target/archive/digest.
4. Run `verslot list`, then repeat install: require exactly `node@22.0.0`
   and `already installed node@22.0.0`, respectively, each with exit 0 and
   empty stderr. Record that the duplicate preserves receipt and payload.
5. Invoke the installed executable by absolute path with `--version`:
   `<root>/installs/node/22.0.0/node.exe` on Windows or
   `<root>/installs/node/22.0.0/bin/node` on Unix. Require exit 0 and record
   the actual version output, expected `v22.0.0`. Do not use PATH, `use`, or
   `current` as evidence. This is the explicit network smoke step that executes
   the installed official binary; routine tests do not do so.
6. Run `verslot uninstall node@22.0.0`: require exit 0, exactly
   `uninstalled node@22.0.0` plus newline, and empty stderr. Then list again:
   require exit 0 and empty stdout/stderr. Confirm the version directory is
   absent, tmp has no operation from this workflow, and current state was not
   created. The persistent `.mutation.lock` and empty directories may remain.
7. Attach actual command output and observations. If a step fails, retain the
   isolated fixture and error paths for investigation. Do not mark M3 Complete
   until the required checks and this workflow pass on the acceptance platform.

| Smoke evidence | Current result |
| --- | --- |
| Isolated base and recorded full version | `<workspace>/target/m3-smoke-1ff54f03c5274ad1b83ef286ca0386dc`; Node.js 22.0.0; full path in smoke log |
| Official archive/checksum URLs and SHA-256 | `https://nodejs.org/dist/v22.0.0/node-v22.0.0-win-x64.zip` and `https://nodejs.org/dist/v22.0.0/SHASUMS256.txt`; SHA-256 `32d639b47d4c0a651ff8f8d7d41a454168a3d4045be37985f9a810cf8cef6174`, matching receipt and official file |
| Fresh install / list / duplicate install | Passed; exact messages, exit 0 and empty stderr; receipt and node.exe SHA-256 unchanged on duplicate |
| Absolute executable `--version` output | Passed: `v22.0.0`, exit 0, empty stderr |
| Uninstall / empty list / filesystem observations | Passed; target absent, tmp empty, current absent; persistent lock/empty directories retained |

## Coverage boundaries

| Platform / architecture | M3 evidence |
| --- | --- |
| Windows x86_64 | Passed: four checks, 120 tests, native ZIP/junction/sharing/locking cases and official 22.0.0 smoke |
| Windows aarch64 | Not run; mapping code alone is not acceptance evidence |
| Linux GNU | ubuntu-latest native CI check/Clippy/tests passed on `59318c9`; official smoke and other architectures not run |
| macOS | macos-latest native CI check/Clippy/tests passed on `59318c9`; official smoke and other architectures not run; non-UTF-8 directory-name fixture excluded |

Privileged Unix runners may bypass permission rejection fixtures; record this
when reporting rename/read failure coverage. Case-alias rejection evidence also
depends on the actual filesystem. The TLS test asserts production HTTPS and
certificate settings; a real certificate-failure handshake fixture is not
provided. Real archive compatibility, bundled npm behavior, historical artifact
availability, and native executable compatibility outside the observed Windows
x86_64 Node.js 22.0.0 smoke remain unverified. Bundled npm contents were checked
by offline fixtures; npm execution was not tested. No automated transitive dependency audit has
been run. Same-user malicious filesystem races and power-loss recovery remain
outside the specified guarantee.

Continue M2's acceptance scope: local checks and the local M3 smoke workflow may
complete M3 while other platforms remain explicitly unverified. M2 results do
not validate M3 code. Full cross-platform switching/current workflows remain M5;
M4 starts after M3 acceptance. Acceptance does not bump the package version or
publish a release.
