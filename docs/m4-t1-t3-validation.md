# M4 T1–T3 Windows validation

Date: 2026-09-28. This is partial milestone validation requested before T4/T5, not M4 acceptance.

Historical record: subsequent T1–T5 checks, 160 tests and the official two-version
smoke passed under [full local Windows M4 acceptance](m4-validation.md). The
results and limitations below describe the earlier T1–T3 run.

Environment: Windows, x86_64-pc-windows-msvc, rustc 1.98.1 (48a229cea 2026-09-01), package 0.3.0. Local working-tree implementation; no release or cross-platform acceptance claim.

## Final results

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed, exit 0 |
| `cargo check --all-targets` | Passed, exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed, exit 0 |
| `cargo test --all` | Passed, exit 0; 130 library + 20 CLI tests; 0 failed, 0 ignored; main/doc tests contain 0 tests |

The focused `cargo test --lib storage::switching_windows::tests::` rerun also passed: 19 tests, 0 failed, 111 filtered out. The final full run covers all 19 again.

## Initial failures and repairs

1. Formatting and all-target compilation passed initially. Clippy rejected a nested cleanup `if`; collapsed it with a let chain in the Windows helper and the corresponding Unix helper. No selection behavior changed.
2. The initial sandbox test attempt could not open `target/debug/.cargo-artifact-lock` (access denied), before running tests. Tests were retried outside the sandbox.
3. The first executed library suite had 127 passed and 3 failed. Native sharing tests incorrectly assumed a zero-desired-access junction handle would block mutation. Backup/publication succeeded unexpectedly; the rollback test likewise did not observe its expected sharing failure.
4. Native sharing fixtures now hold actual read handles. Added private `BackupRename`, `PublishRename` and `RestoreRename` checkpoints immediately after identity/boundary rechecks and before native rename, allowing tests to distinguish library inspection restrictions from rename restrictions. Production passes a no-op callback; no environment switches or retries were added. The external deletion-pending case retains its separate zero-access handle.
5. The focused Windows suite and all four final commands passed after those changes.

## Coverage and limits

T1 shared/exclusive lock coordination, complete-selection state, invalid/missing lock entries, residue guards, child-process lock release after exit/termination, and existing install/uninstall/list regressions passed on Windows.

T3 native junction rename/removal, immediate name reuse, dangling cleanup, no-op identity preservation, publication-gap query locking, failure rollback, unexpected replacement preservation, sharing failures and deletion confirmation passed with isolated synthetic installations. Installation payload preservation assertions passed. An open synthetic payload is not a loaded executable image or running Node.js process.

T2's Unix-only module and tests are excluded on this Windows host. Linux/macOS compilation, Clippy, atomic visibility and rollback tests are **Not run**; the analogous Unix Clippy edit is not proof of Unix validation. No cross-compiled check was run.

`use` and `current` CLI integration remains T4. The 20 CLI tests still exercise their placeholder contract. Full command workflows, PATH execution and real-distribution A/B smoke remain T4/T5 and separate M4 acceptance. Antivirus interference, other filesystems/architectures and loaded-image locking remain unverified. M4 remains In progress, 3 / 5 implementation tasks.
