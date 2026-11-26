use sha2::{Digest, Sha256};

use super::*;

const HANDOFF: &[u8] = b"verslot-self-update-v1\n";

struct Prepared {
    operation: PathBuf,
    executable: PathBuf,
}

impl Prepared {
    fn new(fixture: &Fixture) -> Self {
        let operation = fixture.root.join("tmp/update-fixture");
        fs::create_dir_all(operation.join("candidate")).unwrap();
        let operation = fs::canonicalize(operation).unwrap();
        let executable = operation.join("candidate").join(executable_name("verslot"));
        fs::copy(assert_cmd::cargo::cargo_bin!("verslot"), &executable).unwrap();
        fs::write(operation.join("candidate/LICENSE"), b"license").unwrap();
        fs::write(
            operation.join(".verslot-self-update"),
            b"verslot-self-update-v1\n",
        )
        .unwrap();
        fs::write(operation.join("active.lock"), b"").unwrap();
        let target = if cfg!(windows) {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let state = serde_json::json!({
            "format": 1,
            "current": "0.0.1",
            "target": env!("CARGO_PKG_VERSION"),
            "platform": target,
            "receipt": fs::read_to_string(fixture.root.join("shims/.verslot-shims")).unwrap(),
            "digest": Sha256::digest(fs::read(&executable).unwrap()).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        });
        fs::write(operation.join("state.json"), state.to_string()).unwrap();
        Self {
            operation,
            executable,
        }
    }

    fn start(&self, fixture: &Fixture) -> std::process::Child {
        fixture
            .command(&self.executable)
            .arg("__self-update")
            .arg(&self.operation)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn wait_for_lease(&self, child: &mut std::process::Child) {
        let lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.operation.join("active.lock"))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match lease.try_lock() {
                Err(std::fs::TryLockError::WouldBlock) => return,
                Ok(()) => lease.unlock().unwrap(),
                Err(error) => panic!("lease: {error}"),
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "update exited before handoff"
            );
            assert!(Instant::now() < deadline, "update never acquired its lease");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn release_handoff(child: &mut std::process::Child) {
    child.stdin.take().unwrap().write_all(HANDOFF).unwrap();
}

#[test]
fn external_downloaded_copy_cannot_update_or_create_storage() {
    let fixture = Fixture::new();
    fixture
        .manager()
        .args(["self", "update"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains("manual"));
    assert!(!fixture.root.exists());
    fixture.finish();
}

#[test]
fn prepared_update_refreshes_hard_link_and_copy_layouts_and_preserves_user_state() {
    for copies in [false, true] {
        let fixture = Fixture::new();
        fixture.seed("22.0.0");
        fixture.select("22.0.0");
        fixture.setup();
        if copies {
            for name in ["verslot", "node", "npm", "npx"] {
                fs::remove_file(fixture.shim_path(name)).unwrap();
                fs::copy(
                    assert_cmd::cargo::cargo_bin!("verslot"),
                    fixture.shim_path(name),
                )
                .unwrap();
            }
        }
        let old_receipt = fs::read(fixture.root.join("shims/.verslot-shims")).unwrap();
        fs::write(fixture.root.join("config.json"), b"preserve configuration").unwrap();
        let prepared = Prepared::new(&fixture);
        fixture.seed_yarn("4.9.2");
        let mut child = prepared.start(&fixture);
        prepared.wait_for_lease(&mut child);
        assert_eq!(
            fs::read(fixture.root.join("shims/.verslot-shims")).unwrap(),
            old_receipt
        );
        assert!(child.try_wait().unwrap().is_none());
        // setup can run while the handoff is waiting, without deleting active files.
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout(predicates::str::contains("node@22.0.0"));
        release_handoff(&mut child);
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("Updated Verslot v0.0.1"));
        fixture.assert_shim_commands(&["verslot", "node", "npm", "npx", "yarn", "yarnpkg"]);
        let expected = fs::read(&prepared.executable).unwrap();
        for name in ["verslot", "node", "npm", "npx", "yarn", "yarnpkg"] {
            assert_eq!(fs::read(fixture.shim_path(name)).unwrap(), expected);
        }
        fixture
            .shim("node")
            .arg("--version")
            .assert()
            .success()
            .stdout("v22.0.0\n");
        fixture.shim("verslot").arg("setup").assert().success();
        assert!(!prepared.operation.exists());
        assert_eq!(
            fs::read(fixture.root.join("config.json")).unwrap(),
            b"preserve configuration"
        );
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout(predicates::str::contains("node@22.0.0"));
        fixture.finish();
    }
}

#[test]
fn cancelled_handoff_preserves_the_installed_generation_and_reports_failure() {
    let fixture = Fixture::new();
    fixture.setup();
    let original = fs::read(fixture.shim_path("verslot")).unwrap();
    let prepared = Prepared::new(&fixture);
    let mut child = prepared.start(&fixture);
    prepared.wait_for_lease(&mut child);
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("handoff was cancelled"));
    assert_eq!(fs::read(fixture.shim_path("verslot")).unwrap(), original);
    fixture.setup();
    assert!(!prepared.operation.exists());
    fixture.finish();
}

#[test]
fn mutation_lock_blocks_publication_without_changing_the_installation() {
    let fixture = Fixture::new();
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    let mut child = prepared.start(&fixture);
    prepared.wait_for_lease(&mut child);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.root.join(".mutation.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    release_handoff(&mut child);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("storage is busy"));
    drop(lock);
    fixture.shim("verslot").arg("--version").assert().success();
    fixture.setup();
    fixture.finish();
}

#[test]
fn a_changed_generation_cancels_a_stale_handoff() {
    let fixture = Fixture::new();
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    let mut child = prepared.start(&fixture);
    prepared.wait_for_lease(&mut child);
    fixture.seed("22.0.0");
    fixture.setup();
    let receipt = fs::read(fixture.root.join("shims/.verslot-shims")).unwrap();
    release_handoff(&mut child);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("installation changed during update handoff")
    );
    assert_eq!(
        fs::read(fixture.root.join("shims/.verslot-shims")).unwrap(),
        receipt
    );
    fixture.setup();
    fixture.finish();
}

#[test]
fn prepared_version_and_digest_are_rechecked_before_publication() {
    for field in ["digest", "target", "platform"] {
        let fixture = Fixture::new();
        fixture.setup();
        let prepared = Prepared::new(&fixture);
        let state_path = prepared.operation.join("state.json");
        let mut state: serde_json::Value =
            serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
        state[field] = serde_json::json!("invalid");
        fs::write(state_path, state.to_string()).unwrap();
        let output = prepared.start(&fixture).wait_with_output().unwrap();
        assert!(!output.status.success(), "{field}");
        assert!(output.stdout.is_empty());
        fixture.shim("verslot").arg("--version").assert().success();
        fixture.setup();
        fixture.finish();
    }
}

#[test]
fn internal_update_refuses_an_external_executable_and_outside_operation() {
    let fixture = Fixture::new();
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    fixture
        .manager()
        .arg("__self-update")
        .arg(&prepared.operation)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "must run from its prepared executable",
        ));
    fixture
        .manager()
        .arg("__self-update")
        .arg(&fixture.directory)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "outside its direct temporary directory",
        ));
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_locked_shim_failure_preserves_files_and_recovery_succeeds() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    let original = fs::read(fixture.shim_path("node")).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(fixture.shim_path("node"))
        .unwrap();
    let mut child = prepared.start(&fixture);
    release_handoff(&mut child);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    drop(lock);
    assert_eq!(fs::read(fixture.shim_path("node")).unwrap(), original);
    fixture.setup();
    assert!(!prepared.operation.exists());
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_running_old_generation_allows_update_and_defers_backup_cleanup() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    fixture.seed_yarn("4.9.2");
    let ready = fixture.directory.join("old-generation-ready");
    let mut running = fixture
        .shim("node")
        .arg("block")
        .arg(&ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_file(&ready);
    let mut child = prepared.start(&fixture);
    release_handoff(&mut child);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Updated Verslot"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cleanup is pending"));
    assert!(
        fixture
            .root
            .join(".shims-previous/.verslot-shims")
            .is_file()
    );
    fixture.shim("verslot").arg("--version").assert().success();
    drop(running.stdin.take());
    assert!(running.wait().unwrap().success());
    fixture.shim("verslot").arg("setup").assert().success();
    assert!(!fixture.root.join(".shims-previous").exists());
    assert!(!prepared.operation.exists());
    fixture.finish();
}

#[test]
fn setup_retries_temporary_cleanup_after_a_locked_file_is_released() {
    let fixture = Fixture::new();
    fixture.setup();
    let prepared = Prepared::new(&fixture);
    let lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(prepared.operation.join("active.lock"))
        .unwrap();
    lease.try_lock_shared().unwrap();
    fixture.setup();
    assert!(prepared.operation.exists());
    drop(lease);
    fixture.setup();
    assert!(!prepared.operation.exists());
    fixture.finish();
}
