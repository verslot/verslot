use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "verslot-selection-cli-{}-{nonce}",
            std::process::id()
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
            .stdout(format!("using node@{version}\n"))
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
fn missing_selection_is_read_only_and_use_does_not_install() {
    let fixture = Fixture::new();
    fixture.assert_current("");
    fixture
        .command()
        .args(["use", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("use node@22.0.0: not installed: node@22.0.0\n");
    assert!(!fixture.root.exists());
    fixture.seed("22.0.0");
    fixture.assert_current("");
    assert!(!fixture.root.join(".mutation.lock").exists());
    assert!(!fixture.root.join("current").exists());
    fixture.finish();
}

#[test]
fn selection_cli_switches_and_preserves_m3_behavior() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.seed("24.0.0");
    fixture.select("22.0.0");
    fixture.assert_current("node@22.0.0\n");
    fixture
        .command()
        .args(["install", "node@24.0.0"])
        .assert()
        .success()
        .stdout("already installed node@24.0.0\n")
        .stderr("");
    fixture.assert_current("node@22.0.0\n");
    fixture.select("24.0.0");
    fixture.assert_current("node@24.0.0\n");
    fixture
        .command()
        .arg("list")
        .assert()
        .success()
        .stdout("node@22.0.0\nnode@24.0.0\n")
        .stderr("");
    fixture
        .command()
        .args(["uninstall", "node@24.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "cannot uninstall current version: node@24.0.0",
        ));
    fixture
        .command()
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .success()
        .stdout("uninstalled node@22.0.0\n")
        .stderr("");
    fixture.assert_current("node@24.0.0\n");
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
    fixture
        .command()
        .args(["use", "node@22.0.0"])
        .assert()
        .success()
        .stdout("already using node@22.0.0\n")
        .stderr("");
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
    fixture.assert_current("node@22.0.0\n");
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
        .stderr("use node@24.0.0: not installed: node@24.0.0\n");
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
    fixture.assert_current("node@22.0.0\n");
    assert_eq!(fs::read(&receipt).unwrap(), b"invalid receipt");
    fs::write(
        fixture.root.join("installs/node/22.0.0/.verslot-install"),
        b"invalid current receipt",
    )
    .unwrap();
    for arguments in [vec!["current"], vec!["use", "node@26.0.0"]] {
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
fn current_and_use_fail_while_writer_holds_lock_then_succeed_after_release() {
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
    for arguments in [vec!["current"], vec!["use", "node@24.0.0"]] {
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
    drop(lock);
    fixture.assert_current("node@22.0.0\n");
    fixture.select("24.0.0");
    fixture.assert_current("node@24.0.0\n");
    fixture.finish();
}

#[test]
fn unfinished_switch_blocks_queries_and_writers_but_keeps_list_available() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    for name in [".node-next", ".node-previous"] {
        let residue = fixture.root.join("current").join(name);
        fs::write(&residue, b"preserved residue").unwrap();
        let diagnostic_path = fs::canonicalize(&residue).unwrap();
        for arguments in [
            vec!["current"],
            vec!["use", "node@22.0.0"],
            vec!["install", "node@22.0.0"],
            vec!["uninstall", "node@22.0.0"],
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
            .stdout("node@22.0.0\n")
            .stderr("");
        fs::remove_file(residue).unwrap();
    }
    fixture.assert_current("node@22.0.0\n");
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
        fixture.assert_current(&format!("node@{version}\n"));
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
        .stdout("node@24.0.0\n")
        .stderr("");
    fixture.finish();
}
