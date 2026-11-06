# Report doctor findings on stdout

`verslot doctor` shows a short status or findings with recovery suggestions.
Use `--verbose` to include storage and shim paths, selections, PATH matches,
and Yarn's Node.js resolution. Both modes write plain text to stdout so
redirection preserves the report.

Diagnosed configuration or state errors and checks that cannot be completed
exit with status `1`. Healthy or unconfigured environments exit with status
`0`, and invalid CLI usage exits with status `2`. Reserve stderr for invalid CLI
usage or failures that prevent report generation.

Continue checks that are independent of a failed check, and distinguish checks
that could not be completed from healthy or unselected state. A fresh environment
with no storage, setup or selected tools is unconfigured rather than erroneous;
invalid selection state must not be treated as unselected.
Installed versions alone do not make absent shims erroneous when no tools are
selected and no shim configuration has been created.

Report the first PATH match for `verslot`, `node`, `npm`, `npx`, `yarn` and
`yarnpkg` using the diagnostic process's inherited PATH. These observations do not
describe shell aliases, functions or command caches.

On Windows, search PATH directories in order and candidate extensions in
`PATHEXT` order, without adding PowerShell-specific `.ps1` handling. On Unix,
resolve empty and relative PATH entries against the current directory and check
file execute bits. These are static observations, not proof that a command can
start successfully.

Shim diagnostics use PATH observations without verifying ownership receipts or
binary digests: the relevant command must exist and its first PATH match must
point to the expected shim. Merely including the shim directory on PATH is
insufficient. Support for a tool does not by itself require that tool's shims to
exist.

Only globally selected tools require their commands to resolve to the expected
shims: `node`, `npm` and `npx` for Node.js, and `yarn` and `yarnpkg` for Yarn.
Missing shims for unselected tools are informational, even when other shims
exist. Report the PATH match for `verslot` without requiring it to be the shim
entry; a Cargo-installed CLI is acceptable.

Retain the existing Node.js installation completeness standard. Missing bundled
npm or npx scripts are reported as unavailable commands without invalidating
Node.js or making that finding alone an error exit.

For selected Yarn, inspect its Node.js dependency using Yarn's existing launch
lookup rules, including the Windows search for `node.exe`. A system Node.js is
acceptable when found first. If the first Node.js is a Verslot shim, it needs a
valid selected Node.js installation; an unselected or damaged Node.js selection
is a Yarn dependency error even if a system Node.js appears later on PATH.
No Node.js process is started and no version compatibility is inferred.

The feature requirements and acceptance criteria are tracked in
[Issue #38](https://github.com/verslot/verslot/issues/38).
Setup's planned shim generation based on installed tools is tracked separately
in [Issue #42](https://github.com/verslot/verslot/issues/42).
