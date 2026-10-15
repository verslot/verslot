# Installation progress

Design for [Issue #13](https://github.com/verslot/verslot/issues/13).
Implementation specification: [Issue #14](https://github.com/verslot/verslot/issues/14).
Status: accepted on 2026-10-13; stage feedback (#15) and live byte progress (#16)
are implemented. Total-aware progress (#17) is implemented locally; complete
macOS/Linux native terminal and redirected acceptance remains pending.
GitHub Issues remain the only backlog. The implementation sections below record
the scope and results actually exercised, not full cross-platform acceptance.

## Agreed scope

Improve installation feedback without changing download sources, TLS, checksum
verification, limits, timeouts, extraction safety, locking or commit semantics.
Use an animation library for terminal feedback. Progress is best-effort stderr
output; it must not change stdout results or installation exit codes.
The initial design task produced documentation only; implementation is delivered
in the three linked Issue slices.

Keep SHA-256 computation in the streaming copy loop. Show checksum retrieval,
archive download, digest verification and extraction/installation as distinct
messages. A displayed download percentage is not an installation success signal.
Missing, invalid or observably inconsistent totals show only downloaded bytes.

## Existing behavior and boundaries

- `src/download.rs`: `download_distribution` fetches the checksum list and then
  the archive. `copy_bounded` reads in 64 KiB chunks, checks limits, writes bytes
  and updates SHA-256. The resulting digest is compared after the copy returns.
  Both requests share the existing 15-minute deadline.
- `src/install.rs`: an existing complete installation returns before `prepare`.
  New installations download, extract, validate staging, rename into place and
  clean up. Cleanup failure after rename still returns an error.
- `src/lib.rs` prints the final successful result; `src/main.rs` prints errors.
  Preserve `installed node@<version>` and `already installed node@<version>` on
  stdout. Failure still has empty stdout and the existing nonzero exit code.
- `src/download/tests.rs` already has in-memory readers/writers and local HTTP
  fixtures. `src/install/tests.rs` has private preparation and cleanup seams;
  `tests/cli.rs` covers duplicate installation with empty stderr.

The [M3 contract](../archive/milestones/m3/v0.3.md) remains the reference for
installation integrity and stdout behavior. Its warning against an unjustified
progress dependency is addressed by the explicit request for library animation
and the dependency rationale below. Frozen milestone records are not edited.

## Display contract

Use one animated stderr line on an interactive terminal. Use plain stderr lines
when redirected or when terminal animation is unavailable. Detect stderr, not
stdout. Never force terminal controls into redirected output.

| Operation | Interactive terminal | Plain output |
| --- | --- | --- |
| Fetch checksum list | Spinner and stage message before request | Immediate stage line |
| Download with usable total | Spinner, bar, bytes/total and percentage | Bytes/total and percentage |
| Download without usable total | Spinner and cumulative bytes | Cumulative bytes only |
| Verify SHA-256 | Stage message before digest comparison | Immediate stage line |
| Extract/install | Spinner and stage message before extraction | Immediate stage line |
| Success or failure | Stop animation before final result/diagnostic | Stop progress before final result/diagnostic |

Animation interval: 100 ms, including while a network read blocks.
The spinner indicates activity, not byte advancement. Byte positions advance
only after a chunk is successfully written. Plain byte updates occur at most
once per second and only when bytes change; initial/final byte snapshots and
stage transitions bypass throttling. No speed, ETA, color requirement or new
CLI flags are included. Keep CLI messages in English, matching existing output.

Use a single unambiguous positive Content-Length only when it describes the
same archive bytes being counted. Reject ambiguous, unparseable or inapplicable
lengths for display purposes. A length beyond the existing archive limit is not
a usable display total. If observed bytes exceed the advertised total, drop the
percentage instead of clamping it to a misleading 100%. This is presentation
logic: existing transport errors, size enforcement and checksum verification
remain authoritative. Reaching the total does not establish EOF or success.

No spinner, stage or byte output is created for a duplicate installation.
On failure, do not invent a completion snapshot or a success message. End the
animated line before returning the existing error, including cleanup errors.

## Dependency choice

Use `indicatif` for the terminal spinner and progress bar; evaluate the 0.18
series at implementation time and record the resolved version in Cargo.lock.
The [ProgressBar documentation](https://docs.rs/indicatif/0.18.6/indicatif/struct.ProgressBar.html)
provides spinner, progress-bar and steady-tick APIs. Steady ticking uses a
background thread; downloading and installation remain synchronous, without an
async runtime. Stop that ticker on every exit path.

1. Need: the requested library animation must cover both indeterminate stages
   and determinate byte progress.
2. Standard library: it supplies I/O and timing, but no terminal animation
   component. Hand-written drawing would require maintaining that behavior.
3. Choice: one library covers both display forms and stderr rendering; avoid
   MultiProgress, parallel download machinery and unrelated integrations.
4. Smaller alternative: plain lines need no dependency, but do not meet the
   animation request. A spinner-only library would still require separate
   determinate progress rendering. Use `default-features = false` with fixed
   single-cell dot frames and ASCII bars; no optional integrations are required
   for core animation. The dot frames supersede the original ASCII rotation at
   the user's request during #17 implementation.
   The library still brings required transitive dependencies, including console,
   unit-prefix and portable-atomic; review the resolved lockfile when implementing.
   See the [upstream manifest](https://github.com/console-rs/indicatif/blob/0.18.6/Cargo.toml).

The library can hide output on non-interactive targets, so plain reporting must
be an explicit fallback rather than calls to a hidden progress bar's `println`.
Progress-write errors must not become installation errors. Plain reporting stops
after a failed write or flush. Indicatif's normal updates discard drawing errors;
the accepted terminal policy accepts that behavior rather than adding a custom
terminal adapter solely to detect the first failure. Thus immediate cessation
after a terminal drawing error is not guaranteed. On operation exit, disable
steady ticking and attempt to clear the animated line before returning control
to existing result handling; clearing a failed output device is not guaranteed.
See the [drawing implementation](https://github.com/console-rs/indicatif/blob/0.18.6/src/state.rs).

## Implementation outline

1. Introduce a small private progress reporter with terminal/plain modes,
   stage changes, byte updates and explicit cleanup. Keep formatting, animation
   and throttling out of the download integrity loop. Avoid a general event bus
   or extensible reporting framework.
2. Add an internal progress-aware download path. Preserve the existing public
   `download_verified_archive` signature and silent behavior for library callers
   with a no-output wrapper. The CLI installation path opts into reporting.
3. Notify before checksum and archive requests. Pass the usable response total
   to the reporter, then report actual cumulative bytes after successful writes.
   Preserve the existing read, write, flush, deadline and digest error paths.
   Do not wrap the reader with an animation helper that changes byte accounting.
4. Emit verification immediately before comparing digests and extraction before
   calling `extract_verified_archive`. Keep the installation stage active through
   staging validation, rename and cleanup. Stop reporting before returning to
   existing CLI result handling. Construct active reporting only after the
   duplicate-installation branch.
5. Extend the existing offline fixtures and CLI assertions. Add user-facing
   behavior to README and CHANGELOG only when implemented, not in this design.

## Feedback loop and acceptance

No reproduction or test command was executed during the initial documentation task.
The missing feedback is the Issue's reported symptom, not a newly verified
runtime diagnosis. The diagnosing-bugs reproduction, minimization and fix phases
are deferred until implementation; no root-cause claim is made here.

### Stage-feedback implementation (#15)

The CLI now opts into a private reporter, initialized on its first stage after
the duplicate-installation branch. The public download API remains silent.
Download-stage messages are flushed before requests, verification is announced
before digest comparison, and extraction feedback persists through commit and
cleanup. Reporting is stopped before the existing result handling resumes.

The lockfile resolves indicatif 0.18.6 with default features disabled. Newly
locked packages are indicatif, console 0.16.6, encode_unicode 1.0.0,
portable-atomic 1.15.0 and unit-prefix 0.5.2; existing dependencies were not
upgraded. The dependency rationale above applies to this implementation.

Offline tests compose the real downloader and extractor through the private
installation seam. A local server checks that each request's stage has already
been written before it sends the response. Additional coverage exercises invalid
checksums, failed downloads, digest mismatches, invalid archives, duplicates,
cleanup failure and stopped reporting after sink errors or installation return.

Windows terminal smoke uses the ignored `stage_output_smoke` test with
`--ignored --nocapture`, invoked directly from the compiled library test binary
under a terminal with `TERM=xterm-256color`. Observed ASCII frames advance during
deliberate pauses, stage changes render, and the line clears before the final
smoke message. This is a reporter smoke, not a fresh live Node.js installation.
Running through Cargo in this environment selected the plain-output fallback.
Full three-platform terminal/redirected acceptance remains part of #17.

Validation on Windows (2026-10-13): `cargo fmt --check`,
`cargo check --all-targets`, `cargo clippy --all-targets --all-features -- -D warnings`
and `cargo test --all` passed. The suite reported 140 library tests, 19 CLI tests
and 8 selection CLI tests passed; one manual smoke test is ignored by default
and was run separately. Clippy initially caught test-module placement; moving
the module after implementation resolved it. Redirected smoke output contained
the four stage lines without ESC or cursor-control carriage returns.

### Live byte implementation (#16)

The archive copy reports cumulative bytes after successful chunk writes. The
checksum-list copy and public download wrapper remain silent. The CLI displays
raw bytes with the existing spinner; this slice ignores response totals entirely.
Plain output emits an initial zero snapshot, an immediate first data update,
subsequent changed values at most once per second, and a final snapshot only
after the copy and flush succeed. The initial snapshot does not delay the first
data update. The final byte snapshot is not a checksum or installation success
message; digest comparison and the existing installation lifecycle still follow.

`download_progress_is_visible_before_eof` uses a local chunked HTTP fixture. The
server sends two bytes, waits for a bounded signal triggered by flushing the
rendered `Downloaded 2 bytes` line, then sends the remainder and EOF. The fixture
also terminates on a failed assertion without hanging the client. Its bounds
limit failure time; the success path uses synchronization rather than sleeps.
The test failed against the #15 implementation with "server never observed
rendered byte progress before EOF" and passed after the streaming change.
Focused command:

```text
cargo test download_progress_is_visible_before_eof -- --nocapture
```

Reporter throttling tests use explicit monotonic timestamps to exercise the
one-second boundary, unchanged counts and unthrottled stage/final messages.
Offline coverage also checks write/flush/read errors, timeouts, byte limits,
truncation, digest mismatch, duplicate installation and real extraction with a
failed byte-output sink. The existing installation seam still composes the real
local HTTP downloader, extraction, commit and cleanup.

Windows reporter smoke on 2026-10-15 used the library test executable's ignored
`stage_output_smoke` with `--ignored --nocapture` and `TERM=xterm-256color` for the
terminal case. ASCII frames continued while the displayed count stayed at 1024
or 2048 during deliberate pauses; stage changes and clearing were observed.
Redirected output included 0, 1024 and 2048 byte snapshots without ESC, carriage
returns or percentages. This reporter smoke is not a fresh live Node.js install;
full Windows/macOS/Linux acceptance remains in #17. No dependencies were added
or upgraded for #16.

Validation on Windows (2026-10-15): `cargo fmt --check`,
`cargo check --all-targets`, `cargo clippy --all-targets --all-features -- -D warnings`
and `cargo test --all` passed. The suite reported 147 library tests, 19 CLI tests
and 8 selection CLI tests passed; the ignored reporter smoke was run separately
in terminal and redirected modes. Loopback tests ran outside the sandbox because
the sandbox blocked local HTTP connections. The initial check emitted an
incremental-cache hard-link warning and used file copies; it did not affect the
check outcome.

| Case | Required assertion |
| --- | --- |
| Fixed-size response | Bytes update before EOF; known total and percentage are correct |
| Chunked/unknown total | Bytes advance; no fabricated total or percentage |
| Invalid/inconsistent total | Display falls back without weakening transport validation |
| Slow or stalled read | Terminal animation remains active; byte count does not advance artificially |
| Read/write/flush failure, truncation, limit or timeout | Original error is retained; no completion or later stage |
| Digest mismatch | No extraction or success; original diagnostic preserved |
| Extraction, commit or cleanup failure | Animation stops; existing lifecycle semantics preserved |
| Duplicate installation | No download/preparation; original stdout and empty stderr |
| Redirected stderr | Plain lines only, bounded update rate; no ESC or cursor-control carriage returns |
| Progress sink failure | Installation result unchanged; plain sink disables further reporting; terminal drawing errors follow the library's best-effort behavior |
| Success | Original single stdout result only after full installation and cleanup |

Use injected clocks and failing sinks for reporter tests. Reuse the local HTTP
fixture for response semantics and the existing installation closures for
failure paths. The current offline CLI tests do not exercise a fresh network
installation; compose the real installation preparation and reporter through a
private test seam without exposing production URL overrides.

Before implementation is considered complete, run the four required checks from
[CONTRIBUTING](../../CONTRIBUTING.md), then verify real terminal animation and
redirected output on Windows, macOS and Linux. Non-terminal CI alone cannot
validate terminal behavior. Record exact commands, platform, commit and actual
results; label unexecuted cases explicitly.

### Total-aware implementation (#17)

Only one positive digit-only Content-Length within the archive byte limit is
eligible for display. Transfer-Encoding, Content-Range, duplicate lengths,
duplicate content encodings and any non-identity Content-Encoding disable the
total. HeaderMap tests cover malformed/ambiguous metadata that the HTTP parser
can reject before the normal reporting boundary. Real local HTTP tests cover
fixed-size, close-delimited, chunked and encoded response presentation. Existing
transport/framing failures remain errors rather than successful fallback.

Plain output includes bytes/total and integer percentages rounded down, using
wide arithmetic to avoid overflow. The terminal adds a bar and the requested
leading dot spinner. If observed bytes exceed the display total, subsequent
output uses bytes only and the terminal removes the bar. Progress reaching 100%
does not bypass EOF, digest comparison, extraction, commit or cleanup. Stage
changes remove total/percentage formatting. No dependency changes were needed.

The new known-length HTTP regression failed against #16's byte-only output and
passed after this change. The existing synchronized before-EOF regression now
covers both chunked and fixed-length transfers (two of three bytes shows 66%).
Controlled-clock tests cover throttling and inconsistent totals; installation
tests verify that even a 100% transfer can still fail extraction or cleanup.

Two default-ignored native acceptance entry points are available in the compiled
library test executable after `cargo test --lib --no-run`:

```text
<library-test-executable> stage_output_smoke --ignored --nocapture
<library-test-executable> installation_progress_smoke --ignored --nocapture
```

Run each directly in a native terminal and then with stderr redirected to a
file. On Windows use PowerShell's `& <path>`; on macOS/Linux execute the path
directly. Supported terminals must render the dot glyphs. The reporter smoke
pauses with unchanged bytes, shows 50%/100%, then demonstrates byte-only fallback.
The installation smoke uses isolated temporary storage and real local HTTP,
hashing, extraction, commit and cleanup for known/unknown totals, successful
installation, duplicate installation and invalid-archive failure. It does not
change the user's installation root or expose production URL overrides.

Windows interactive and redirected runs on 2026-10-15 passed. Dot frames, the
bar, percentages and fallback were observed; redirected runs contained no ESC
or cursor-control carriage returns. The offline lifecycle run passed for both
response modes. These are native offline checks, not a fresh download from the
Node.js server, nor evidence of macOS/Linux execution.

On the same Windows checkout, `cargo fmt --check`, `cargo check --all-targets`,
`cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all`
passed: 152 library tests, 19 CLI tests and 8 selection CLI tests (179 passed).
The two default-ignored acceptance tests above were executed separately.
Standards review found no issues; specification review found no implementation
defects and identified the pending native macOS/Linux acceptance below.

| Native acceptance | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Reporter: stalled activity, known/unknown totals, fallback, stage changes | Passed | Not run | Not run |
| Reporter redirected: plain output, totals, fallback, no terminal controls | Passed | Not run | Not run |
| Offline install lifecycle: success, failure, duplicate, known/unknown totals | Passed | Not run | Not run |
| Offline install lifecycle redirected | Passed | Not run | Not run |

This leaves the complete cross-platform acceptance criterion in #17 open. The
existing CI matrix runs code checks and automated tests on Windows/macOS/Linux
when a PR targets main, but it does not establish interactive rendering. Run the
same native acceptance entry points on macOS/Linux and record commit, commands
and observations before closing #17 or declaring all of #13/#14 accepted.

## Confirmed decisions

- `indicatif`, one animated line, 100 ms steady ticking, plain redirected output
  with at most one byte update per second, and no speed/ETA/new flags.
- Accept indicatif's ignored terminal drawing errors without a custom terminal
  adapter; plain output stops after a write/flush error and animation always
  stops when installation exits.

No new domain terms or hard-to-reverse architectural decisions have been
established, so this design does not create a glossary or ADR.
