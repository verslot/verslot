use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "verslot-selection-cli-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
        Self { directory, root }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("verslot"));
        command
            .current_dir(&self.directory)
            .env("HOME", &self.directory)
            .env("LOCALAPPDATA", &self.directory);
        command
    }

    fn seed(&self, version: &str) {
        let target = format!("node@{version}")
            .parse::<verslot::target::Target>()
            .unwrap();
        let distribution =
            verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
        let installation = self.root.join("installs/node").join(version);
        let executable = installation.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        // Not a runnable program: the selection commands must only inspect it.
        fs::write(&executable, b"offline fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(
            installation.join(".verslot-install"),
            format!(
                "verslot-install-v1\n{target}\n{}\n{}\n",
                distribution.archive_filename,
                "a".repeat(64)
            ),
        )
        .unwrap();
    }

    fn executable_directory(&self, version: &str) -> PathBuf {
        let installation = self.root.join("installs/node").join(version);
        if cfg!(windows) {
            installation
        } else {
            installation.join("bin")
        }
    }

    fn select(&self, version: &str) {
        self.command()
            .args(["use", &format!("node@{version}")])
            .assert()
            .success()
            .stdout(format!("✔  Selected node@{version}\n"))
            .stderr("");
    }

    fn assert_current(&self, stdout: &str) {
        self.command()
            .arg("current")
            .assert()
            .success()
            .stdout(stdout.to_owned())
            .stderr("");
    }

    fn finish(self) {
        let current = self.root.join("current/node");
        if fs::symlink_metadata(&current).is_ok() {
            #[cfg(unix)]
            fs::remove_file(&current).unwrap();
            #[cfg(windows)]
            junction::delete(&current).unwrap();
        }
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        let resolved = fs::canonicalize(&self.directory).unwrap();
        assert_eq!(resolved.parent(), Some(temporary.as_path()));
        fs::remove_dir_all(resolved).unwrap();
    }
}

#[test]
fn unuse_succeeds_without_creating_missing_storage_or_current_directories() {
    let fixture = Fixture::new();
    for tool in ["node", "yarn"] {
        fixture
            .command()
            .args(["unuse", tool])
            .assert()
            .success()
            .stdout(format!("   Already unselected {tool}\n"))
            .stderr("");
        assert!(!fixture.root.exists());
    }
    fixture.seed("22.0.0");
    fixture
        .command()
        .args(["unuse", "node"])
        .assert()
        .success()
        .stdout("   Already unselected node\n")
        .stderr("");
    assert!(!fixture.root.join("current").exists());
    fixture.assert_current("");
    fixture.finish();
}

#[test]
fn missing_selection_is_read_only_and_use_does_not_install() {
    let fixture = Fixture::new();
    fixture.assert_current("");
    for request in ["node@18", "node@18.1"] {
        fixture
            .command()
            .args(["use", request])
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains(format!(
                "no installed version matches {request}"
            )));
        assert!(!fixture.root.exists());
    }
    fixture
        .command()
        .args(["use", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(
            "✘  error: node@22.0.0 is not installed\n   \n   Run `verslot install node@22.0.0` first.\n",
        );
    assert!(!fixture.root.exists());
    fixture.seed("22.0.0");
    fixture.assert_current("");
    assert!(!fixture.root.join(".mutation.lock").exists());
    assert!(!fixture.root.join("current").exists());
    fixture.finish();
}

#[test]
fn partial_use_selects_the_numerically_latest_installed_major_version() {
    let fixture = Fixture::new();
    for version in [
        "18.9.9", "18.10.0", "18.1.10", "19.0.0", "118.0.0", "1.20.0",
    ] {
        fixture.seed(version);
    }
    for name in ["18", "018.99.0", "18.100.0-beta", "18.99"] {
        fs::write(fixture.root.join("installs/node").join(name), b"ignored").unwrap();
    }
    fixture
        .command()
        .args(["use", "node@18"])
        .assert()
        .success()
        .stdout("✔  Selected node@18.10.0\n")
        .stderr("");
    fixture.assert_current("   node@18.10.0\n");
    fixture
        .command()
        .args(["use", "node@1"])
        .assert()
        .success()
        .stdout("✔  Selected node@1.20.0\n")
        .stderr("");
    fixture.assert_current("   node@1.20.0\n");
    fixture.seed("18.11.0");
    fixture
        .command()
        .args(["use", "node@18"])
        .assert()
        .success()
        .stdout("✔  Selected node@18.11.0\n")
        .stderr("");
    fixture.assert_current("   node@18.11.0\n");
    fixture.finish();
}

#[test]
fn partial_use_matches_minor_components_and_exact_versions() {
    let fixture = Fixture::new();
    for version in ["18.1.1", "18.1.9", "18.1.10", "18.10.100", "19.1.100"] {
        fixture.seed(version);
    }
    fixture
        .command()
        .args(["use", "node@18.1"])
        .assert()
        .success()
        .stdout("✔  Selected node@18.1.10\n")
        .stderr("");
    fixture.assert_current("   node@18.1.10\n");
    fixture.select("18.1.1");
    fixture.assert_current("   node@18.1.1\n");
    fixture.finish();
}

#[test]
fn unmatched_partial_use_preserves_selection_without_installing() {
    let fixture = Fixture::new();
    fixture.seed("18.10.0");
    fixture.select("18.10.0");
    for request in ["node@1", "node@18.1", "node@19"] {
        fixture
            .command()
            .args(["use", request])
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains(format!(
                "no installed version matches {request}"
            )));
        fixture.assert_current("   node@18.10.0\n");
    }
    assert_eq!(
        fs::read_dir(fixture.root.join("installs/node"))
            .unwrap()
            .count(),
        1
    );
    assert!(!fixture.root.join("tmp").exists());
    fixture.finish();
}

#[test]
fn partial_use_reports_corrupt_matching_installations_without_falling_back() {
    let fixture = Fixture::new();
    for version in ["18.1.1", "18.1.10", "22.0.0"] {
        fixture.seed(version);
    }
    fixture.select("22.0.0");
    let receipt = fixture.root.join("installs/node/18.1.10/.verslot-install");
    fs::write(&receipt, b"invalid receipt").unwrap();
    for request in ["node@18", "node@18.1"] {
        fixture
            .command()
            .args(["use", request])
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains("inspect node@18.1.10"))
            .stderr(predicate::str::contains(
                "receipt format, target, or platform",
            ));
        fixture.assert_current("   node@22.0.0\n");
    }
    assert_eq!(fs::read(&receipt).unwrap(), b"invalid receipt");
    fixture.finish();
}

#[test]
fn partial_use_ignores_corrupt_installations_outside_the_requested_series() {
    let fixture = Fixture::new();
    fixture.seed("18.1.10");
    fixture.seed("18.10.0");
    fs::write(
        fixture.root.join("installs/node/18.10.0/.verslot-install"),
        b"invalid receipt",
    )
    .unwrap();
    fixture
        .command()
        .args(["use", "node@18.1"])
        .assert()
        .success()
        .stdout("✔  Selected node@18.1.10\n")
        .stderr("");
    fixture.assert_current("   node@18.1.10\n");
    fixture.finish();
}

#[test]
fn partial_use_rejects_linked_installations_and_preserves_their_targets() {
    let fixture = Fixture::new();
    fixture.seed("18.1.0");
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    let outside = fixture.directory.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserved").unwrap();
    let linked = fixture.root.join("installs/node/18.10.0");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &linked).unwrap();
    #[cfg(windows)]
    junction::create(&outside, &linked).unwrap();
    fixture
        .command()
        .args(["use", "node@18"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("inspect node@18.10.0"))
        .stderr(predicate::str::contains("linked"));
    fixture.assert_current("   node@22.0.0\n");
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
    assert!(fs::symlink_metadata(&linked).is_ok());
    #[cfg(unix)]
    fs::remove_file(&linked).unwrap();
    #[cfg(windows)]
    junction::delete(&linked).unwrap();
    fixture.finish();
}

#[test]
fn selection_cli_switches_and_preserves_m3_behavior() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.seed("24.0.0");
    fixture.select("22.0.0");
    fixture.assert_current("   node@22.0.0\n");
    fixture
        .command()
        .args(["install", "node@24.0.0"])
        .assert()
        .success()
        .stdout("   Already installed node@24.0.0\n")
        .stderr("");
    fixture.assert_current("   node@22.0.0\n");
    fixture.select("24.0.0");
    fixture.assert_current("   node@24.0.0\n");
    fixture
        .command()
        .arg("list")
        .assert()
        .success()
        .stdout("   node@22.0.0\n   node@24.0.0\n")
        .stderr("");
    fixture
        .command()
        .args(["uninstall", "node@24.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "node@24.0.0 is currently selected",
        ));
    fixture
        .command()
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .success()
        .stdout("✔  Uninstalled node@22.0.0\n")
        .stderr("   Uninstalling\n");
    fixture.assert_current("   node@24.0.0\n");
    assert!(!fixture.root.join("current/.node-next").exists());
    assert!(!fixture.root.join("current/.node-previous").exists());
    fixture.finish();
}

#[test]
fn selecting_same_version_does_not_replace_current_link() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    let current = fixture.root.join("current/node");
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        fs::symlink_metadata(&current).unwrap().ino()
    };
    #[cfg(windows)]
    let open_junction = || {
        use std::os::windows::fs::OpenOptionsExt;
        // Zero desired access stays compatible with junction's exclusive query handles.
        fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(7)
            .custom_flags(0x0020_0000 | 0x0200_0000)
            .open(&current)
            .unwrap()
    };
    #[cfg(windows)]
    let link_handle = open_junction();
    #[cfg(windows)]
    let identity = winapi_util::file::information(&link_handle).unwrap();
    for request in ["node@22.0.0", "node@22", "node@22.0"] {
        fixture
            .command()
            .args(["use", request])
            .assert()
            .success()
            .stdout("   Already selected node@22.0.0\n")
            .stderr("");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::symlink_metadata(&current).unwrap().ino(), identity);
    }
    #[cfg(windows)]
    {
        let observed = winapi_util::file::information(open_junction()).unwrap();
        assert_eq!(
            observed.volume_serial_number(),
            identity.volume_serial_number()
        );
        assert_eq!(observed.file_index(), identity.file_index());
        drop(link_handle);
    }
    fixture.assert_current("   node@22.0.0\n");
    fixture.finish();
}

#[test]
fn missing_or_incomplete_target_preserves_selection_and_invalid_current_takes_precedence() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    fixture
        .command()
        .args(["use", "node@24.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(
            "✘  error: node@24.0.0 is not installed\n   \n   Run `verslot install node@24.0.0` first.\n",
        );
    fixture.seed("24.0.0");
    let receipt = fixture.root.join("installs/node/24.0.0/.verslot-install");
    fs::write(&receipt, b"invalid receipt").unwrap();
    fixture
        .command()
        .args(["use", "node@24.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "receipt format, target, or platform",
        ));
    fixture.assert_current("   node@22.0.0\n");
    assert_eq!(fs::read(&receipt).unwrap(), b"invalid receipt");
    fs::write(
        fixture.root.join("installs/node/22.0.0/.verslot-install"),
        b"invalid current receipt",
    )
    .unwrap();
    for arguments in [
        vec!["current"],
        vec!["use", "node@26.0.0"],
        vec!["use", "node@26"],
        vec!["use", "node@26.1"],
    ] {
        fixture
            .command()
            .args(arguments)
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains(
                "current installation is not complete",
            ))
            .stderr(predicate::str::contains("not installed").not());
    }
    fixture.finish();
}

#[test]
fn selection_commands_respect_the_writer_lock() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.seed("24.0.0");
    fixture.select("22.0.0");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.root.join(".mutation.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    // Model a concurrent child retaining a duplicate until it reaches exec.
    let inherited_lock = lock.try_clone().unwrap();
    for arguments in [
        vec!["current"],
        vec!["use", "node@24.0.0"],
        vec!["use", "node@24"],
        vec!["use", "node@24.0"],
        vec!["use", "node@18"],
        vec!["unuse", "node"],
        vec!["unuse", "yarn"],
    ] {
        fixture
            .command()
            .args(arguments)
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains(
                "storage is busy; retry after the active operation finishes",
            ));
    }
    lock.unlock().unwrap();
    drop(lock);
    fixture.assert_current("   node@22.0.0\n");
    fixture.select("24.0.0");
    fixture.assert_current("   node@24.0.0\n");
    drop(inherited_lock);
    fixture.finish();
}

#[test]
fn unfinished_switch_blocks_queries_and_writers_but_keeps_list_available() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    for name in [
        ".node-next",
        ".node-previous",
        ".yarn-next",
        ".yarn-previous",
    ] {
        let residue = fixture.root.join("current").join(name);
        fs::write(&residue, b"preserved residue").unwrap();
        let diagnostic_path = fs::canonicalize(&residue).unwrap();
        for arguments in [
            vec!["current"],
            vec!["use", "node@22.0.0"],
            vec!["use", "node@22"],
            vec!["use", "node@22.0"],
            vec!["install", "node@22.0.0"],
            vec!["uninstall", "node@22.0.0"],
            vec!["unuse", "node"],
            vec!["unuse", "yarn"],
        ] {
            fixture
                .command()
                .args(arguments)
                .assert()
                .code(1)
                .stdout("")
                .stderr(predicate::str::contains("unfinished switch"))
                .stderr(predicate::str::contains(
                    diagnostic_path.display().to_string(),
                ));
            assert_eq!(fs::read(&residue).unwrap(), b"preserved residue");
        }
        fixture
            .command()
            .arg("list")
            .assert()
            .success()
            .stdout("   node@22.0.0\n")
            .stderr("");
        fs::remove_file(residue).unwrap();
    }
    fixture.assert_current("   node@22.0.0\n");
    fixture.finish();
}

// A copied native test binary identifies its adjacent installation receipt.
// This fixture exercises executable lookup without Node.js, a shell or a compiler.
#[test]
fn synthetic_node_reports_its_receipt() {
    if std::env::var_os("VERSLOT_TEST_NODE_EXECUTION").is_none() {
        return;
    }
    let executable = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let directory = executable.parent().unwrap();
    let installation = if cfg!(windows) {
        directory
    } else {
        directory.parent().unwrap()
    };
    let receipt = fs::read_to_string(installation.join(".verslot-install")).unwrap();
    println!("synthetic {}", receipt.lines().nth(1).unwrap());
}

#[test]
fn fixed_entry_and_controlled_path_execute_selected_payload_after_switch() {
    let fixture = Fixture::new();
    let executable_name = if cfg!(windows) { "node.exe" } else { "node" };
    for version in ["22.0.0", "24.0.0"] {
        fixture.seed(version);
        fs::copy(
            std::env::current_exe().unwrap(),
            fixture.executable_directory(version).join(executable_name),
        )
        .unwrap();
    }
    let entry = fixture.root.join(if cfg!(windows) {
        "current/node"
    } else {
        "current/node/bin"
    });
    let selected_path = std::env::join_paths([&entry]).unwrap();
    for version in ["22.0.0", "24.0.0"] {
        fixture.select(version);
        for program in [entry.join(executable_name), PathBuf::from(executable_name)] {
            Command::new(program)
                .args([
                    "--exact",
                    "synthetic_node_reports_its_receipt",
                    "--nocapture",
                ])
                .current_dir(&fixture.directory)
                .env("HOME", &fixture.directory)
                .env("LOCALAPPDATA", &fixture.directory)
                .env("PATH", &selected_path)
                .env("VERSLOT_TEST_NODE_EXECUTION", "1")
                .assert()
                .success()
                .stdout(predicate::str::contains(format!(
                    "synthetic node@{version}\n"
                )))
                .stderr("");
        }
        fixture.assert_current(&format!("   node@{version}\n"));
    }
    let competing_directory = fixture.executable_directory("22.0.0");
    let competing_path = std::env::join_paths([&competing_directory, &entry]).unwrap();
    Command::new(executable_name)
        .args([
            "--exact",
            "synthetic_node_reports_its_receipt",
            "--nocapture",
        ])
        .current_dir(&fixture.directory)
        .env("PATH", &competing_path)
        .env("VERSLOT_TEST_NODE_EXECUTION", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("synthetic node@22.0.0\n"))
        .stderr("");
    fixture
        .command()
        .arg("current")
        .env("PATH", &competing_path)
        .assert()
        .success()
        .stdout("   node@24.0.0\n")
        .stderr("");
    fixture.finish();
}
