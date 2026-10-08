# M4 Validation Preparation and Results

Last updated: 2026-09-28

T1–T5 code, test code, acceptance mapping and documentation exist. M4 is
**Complete under local Windows acceptance**. Package version remains `0.3.0`;
no release or version bump is part of acceptance. The accepted implementation is an
uncommitted working tree based on `ca320a898bedc64c5ece9416be11c0b580565841`.
It includes T1–T5 and the acceptance repairs below; [source SHA-256 hashes](evidence/source-sha256.txt)
identify the tested Cargo/source/test files, and [environment metadata](evidence/environment.json)
records the platform and built executable hash.

The user requested no checks during development, then explicitly requested M4
acceptance. All four checks, 160 tests and the official two-version smoke
**passed on 2026-09-28** after the repairs below.
[T1–T3 Windows evidence](m4-t1-t3-validation.md) is historical partial evidence
and predates the T4/T5 additions; the final full run includes them. M2/M3 CI
results do not validate M4. Unix and other architectures remain unverified.

## Required checks and evidence

Run separately when validation is requested; retain stdout/stderr, exit codes,
initial failures and subsequent fixes. A failure reopens the affected task.

```text
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

| Evidence | Current result |
| --- | --- |
| Acceptance date / exact source revision and local changes | 2026-09-28; working tree based on `ca320a898bedc64c5ece9416be11c0b580565841`, T1–T5 plus acceptance repairs; [source manifest](evidence/source-sha256.txt) |
| OS, filesystem, architecture and Rust version at acceptance | Windows NT 10.0.26200.0; NTFS; x86_64-pc-windows-msvc; rustc 1.98.1 (48a229cea 2026-09-01); [environment](evidence/environment.json) |
| `cargo fmt --check` | Passed, exit 0 after formatting repairs; [log](evidence/fmt.txt) empty on success |
| `cargo check --all-targets` | Passed, exit 0 after test-helper lifetime repair; [log](evidence/check.txt) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed, exit 0 after needless-borrow repair; [log](evidence/clippy.txt) |
| `cargo test --all` | Passed, exit 0: 133 library + 19 CLI + 8 selection CLI = 160 tests, 0 failed/ignored; main/doc targets contain 0 tests; [log](evidence/test.txt) |
| Official-distribution A/B smoke | Passed on Windows x86_64 with 22.0.0 and 24.0.0; [raw output](evidence/smoke.txt) |
| Acceptance decision | Local Windows M4 acceptance passed; M4 Complete; cross-platform delivery remains M5 |

The [acceptance mapping](v0.4.md#acceptance-mapping) names actual tests. Record
cfg-excluded platform tests separately from failures, ignored tests and passes.

## Initial failures and repairs

1. Initial [fmt](evidence/fmt-initial.txt) rejected formatting in
   src/install/tests.rs and tests/selection_cli.rs. Cargo fmt repaired these
   assertions/argument lists; the subsequent fmt check passed.
2. Initial [all-target check](evidence/check-initial.txt) rejected
   Fixture::assert_current passing a borrowed expected string to assert_cmd,
   which requires an owned or static output predicate. It now passes
   stdout.to_owned(); the all-target check passed afterward.
3. Initial [Clippy](evidence/clippy-initial.txt) rejected an unnecessary
   borrow of a temporary Windows junction handle. The information call now
   consumes that temporary directly. Clippy passed; the full tests compiled
   and exercised the repaired code. No production switching algorithm changed.
4. The initial [test attempt](evidence/test-initial.txt) could not open
   target/debug/.cargo-artifact-lock due to sandbox access denial and ran no
   tests. The requested tests were rerun outside the sandbox and all 160 passed.
5. The initial [smoke](evidence/smoke-initial.txt) successfully installed
   A, then PowerShell Invoke-WebRequest failed TLS authentication while fetching
   the independent official checksum file in the sandbox. The partial fixture
   was retained. The full smoke was rerun outside the sandbox with a fresh base,
   default TLS validation and no verification bypass, and passed. Production
   Rust HTTPS installation succeeded on both attempts.

The [smoke harness](evidence/smoke.ps1) records raw exits/stdout/stderr
and checks exact CLI output. Official Windows Node version lines use CRLF,
recorded verbatim; the harness normalizes only CRLF to LF for line comparison.
The owned success/partial fixtures remain under target for evidence. The
successful fixture ends with B selected; neither the user's persistent PATH
nor their normal Verslot root was changed, and no active uninstall was added.

## Offline workflow scope

- `install::tests::offline_install_switch_query_uninstall_workflow_is_isolated`
  starts a child with HOME/LOCALAPPDATA overrides. Its private M3 preparation
  seam supplies generated native archives and SHA-256 digests; real extraction,
  installation commit, list, duplicate install, selection, query, active
  uninstall rejection and inactive uninstall run together. It checks bundled
  payloads and absence of operation/switch residue. It does not download.
- `fixed_entry_and_controlled_path_execute_selected_payload_after_switch` in
  [selection CLI tests](../../../../tests/selection_cli.rs) copies the native Rust test
  binary into both complete installations as node/node.exe. The child test
  `synthetic_node_reports_its_receipt` reads its own adjacent receipt and prints
  a version marker. First A and then B are launched by the fixed absolute entry
  and by executable name with an isolated PATH. A's executable directory is
  subsequently prepended as a competitor: execution identifies A, while current
  still reports B. Arguments select a test-harness helper, not Node's
  `--version`; this establishes synthetic lookup/execution coverage only.
- Unix preparation/publication failure and Windows publication-gap tests start
  `storage::selection_tests::mutations_during_switch_child` while the parent's
  native select operation holds its exclusive lock. The other process attempts
  select, current, duplicate install and uninstall, requiring busy errors.
  Existing CLI locking regressions check command exit/output behavior.
- Existing T1–T4 state/residue and native/injected-failure tests remain the
  recovery coverage. Test environment switches and checkpoints remain private
  to test code; no production mirror or failure controls are added.

These routines execute only owned synthetic payloads, with no live downloads,
user Node.js, persistent PATH changes or compiler invoked by a test fixture.
All native Windows routines above passed in the final full test run. Unix-only
tests are cfg-excluded on this host, not passing or ignored Windows tests.

## Official-distribution smoke procedure

This separate acceptance step accesses official HTTPS sources and executes
installed Node.js binaries. Acceptance used full versions A = **22.0.0** and
B = **24.0.0**; record any substituted full versions rather than using aliases.
Record platform/architecture, toolchain, filesystem, absolute Verslot binary
path, source revision, timestamps, official archive/checksum URLs and receipt
digests. An unavailable artifact or failure is evidence to investigate, not a
reason to bypass checksum/TLS verification.

1. Start a fresh child shell without profiles and create a uniquely named
   temporary base. Set HOME and LOCALAPPDATA to that base for every Verslot
   invocation. Use Verslot by absolute path. Keep the shell working directory
   at the empty base, outside any directory containing competing executables.
   Never reuse the normal user root or an earlier smoke fixture.
2. Run list and current: each must exit 0 with empty stdout/stderr and create
   no storage. Install A and B: each must exit 0 with exactly
   `installed node@22.0.0` / `installed node@24.0.0` plus newline and empty stderr.
   Record official archive and SHASUMS256.txt URLs and verify receipts match
   their target, archive and official digest. List must print A then B; current
   must remain empty after both installs.
3. Select A: require `using node@22.0.0`, exit 0, empty stderr. Current must
   print `node@22.0.0`. Invoke the fixed absolute executable with `--version`:
   require `v22.0.0`, exit 0 and empty stderr. Save receipt/payload hashes to
   compare after switching.
4. In this child shell only, prepend the fixed entry directory to PATH. Clear
   any command cache and record command resolution: PowerShell
   `Get-Command node -CommandType Application` / `where.exe node`, or Bash
   `hash -r` / `command -v node`. Confirm the first executable is the fixed
   entry. Run `node --version`: require `v22.0.0`, exit 0 and empty stderr.
5. Select B: require `using node@24.0.0`. Repeat current, direct-entry,
   command-resolution and PATH checks: outputs must identify B. Select B
   again: require `already using node@24.0.0` and exit 0. Compare link identity
   if recorded; do not treat timestamp-only observations as identity proof.
6. In the same child shell, temporarily prepend A's installed executable
   directory ahead of the fixed entry, clear caches and record resolution.
   `node --version` must identify A while current still prints B. Restore the
   controlled PATH with the fixed entry first and require B again.
7. Uninstall B: require exit 1, empty stdout and the diagnostic
   `cannot uninstall current version: node@24.0.0`. Uninstall A: require
   `uninstalled node@22.0.0`, exit 0, empty stderr. List/current must show only
   B. Direct-entry and controlled-PATH execution must still return `v24.0.0`.
   Confirm A is absent, B's receipt/payload hashes are unchanged, no reserved
   siblings exist, and tmp contains no operation from this workflow.
8. Save actual command output, exits and observations. On failure retain the
   owned fixture and exact residue paths for investigation. On success close
   child processes/handles before optionally cleaning this uniquely owned
   fixture. Remove native current links without traversing their targets, and
   verify the resolved fixture path is the one created in step 1 before any
   recursive cleanup. This harness cleanup is not active-version uninstall;
   the workflow ends with B selected because Verslot has no unset command.
   Exit the child shell to discard its environment overrides.

| Platform | Isolated storage root | Fixed executable | PATH entry | Competing A directory |
| --- | --- | --- | --- | --- |
| Windows | `<base>\verslot` | `<root>\current\node\node.exe` | `<root>\current\node` | `<root>\installs\node\22.0.0` |
| macOS / Linux | `<base>/.verslot` | `<root>/current/node/bin/node` | `<root>/current/node/bin` | `<root>/installs/node/22.0.0/bin` |

Use the [README shell examples](../../../../README.md#select-nodejs-and-configure-path-manually)
only inside that child, with its overridden native root. Do not edit the user's
persistent PATH, shell profiles or existing Node.js installations.

| Smoke evidence | Current result |
| --- | --- |
| Fresh isolated base / exact A and B versions | `D:\code\verslot\target\m4-smoke-0c7a734c06644092b98a0aca3a12fa15`; 22.0.0 / 24.0.0; success fixture retained |
| Official URLs, SHA-256 and receipt comparisons | Passed; official win-x64 archives and SHASUMS256.txt matched receipts; exact URLs/digests in [smoke log](evidence/smoke.txt); [A checksums](evidence/SHASUMS256-22.0.0.txt), [B checksums](evidence/SHASUMS256-24.0.0.txt) |
| Install/list and no selection after installs | Passed; exact installed messages, A/B numeric list, empty current and no storage creation by initial queries |
| First select / current / fixed-entry execution | Passed; using/current identify A, fixed node.exe returns `v22.0.0` |
| PATH resolution and execution for A and B | Passed; Get-Command/where.exe identify fixed node.exe first, node --version returns `v22.0.0` then `v24.0.0` |
| Same-version no-op and competing PATH/current distinction | Passed; `already using node@24.0.0`; competing A returns `v22.0.0` while current reports B; controlled PATH restored to B. Native no-op identity verified separately by tests |
| Active uninstall rejection / inactive uninstall / retained B execution | Passed; uninstall B exit 1 with empty stdout and active-version error; uninstall A succeeds; list/current and direct/PATH launches retain B |
| Payload preservation, tmp and reserved-sibling observations | Passed; A/B receipt/node.exe hashes unchanged after switch/no-op, B unchanged after A removal; A absent, tmp empty, no reserved siblings |

Official archive digests verified against receipts:

| Version / archive | SHA-256 |
| --- | --- |
| 22.0.0 / node-v22.0.0-win-x64.zip | `32d639b47d4c0a651ff8f8d7d41a454168a3d4045be37985f9a810cf8cef6174` |
| 24.0.0 / node-v24.0.0-win-x64.zip | `3d0fff80c87bb9a8d7f49f2f27832aa34a1477d137af46f5b14df5498be81304` |

## Coverage boundaries and acceptance policy

| Platform / architecture | Evidence |
| --- | --- |
| Windows x86_64 / NTFS | Passed: four checks, 160 tests, native junction/sharing/locking/recovery, CLI and synthetic entry/PATH tests, official 22.0.0 / 24.0.0 smoke |
| Windows aarch64 | Not run |
| Linux GNU / macOS | M4 compilation, Clippy, native switching, execution and official smoke Not run |
| Other architectures / filesystems | Not run; distribution mapping is not execution proof |

Privileged Unix runners may bypass permission rejection; record whether that
native fixture exercised the error. Antiviruses, loaded Node.js images,
filesystem-specific sharing, multi-file reader consistency and abrupt process
death at each publication checkpoint remain unverified. Existing tests establish
OS lock release on process exit/termination and detect retained residue/gaps;
they do not establish crash recovery. Unexpected-entry safety is exercised with
controlled swaps, not hostile same-user race resistance. Bundled npm execution
and power-loss durability are outside this acceptance contract.

Local checks and the complete local official-distribution smoke completed M4
under the existing milestone policy; other platforms remain explicitly
unverified and full cross-platform delivery remains M5. The package remains
0.3.0 and no release was published. Keep unexecuted platform checks marked
Not run; any later acceptance failure reopens the affected task.
