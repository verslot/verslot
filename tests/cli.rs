use assert_cmd::Command;
use predicates::prelude::*;

fn verslot_command() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("verslot"))
}

#[test]
fn help_describes_the_cli() {
    verslot_command()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A minimal, extensible tool version manager written in Rust.",
        ))
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("uninstall"))
        .stdout(predicate::str::contains("use"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("not implemented yet").not())
        .stdout(predicate::str::contains("current"));
}

#[test]
fn version_reports_the_package_version() {
    verslot_command()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("verslot {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn invalid_command_fails() {
    verslot_command()
        .arg("invalid-command")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn install_reports_storage_errors_without_network_access() {
    verslot_command()
        .args(["install", "node@22.0.0"])
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("install node@22.0.0:"))
        .stderr(predicate::str::contains("is missing"));
}

#[test]
fn uninstall_reports_storage_errors() {
    verslot_command()
        .args(["uninstall", "node@22.0.0"])
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("uninstall node@22.0.0:"))
        .stderr(predicate::str::contains("is missing"));
}

#[test]
fn use_reports_storage_errors() {
    verslot_command()
        .args(["use", "node@22.0.0"])
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("use node@22.0.0:"))
        .stderr(predicate::str::contains("is missing"));
}

#[test]
fn list_reports_storage_errors() {
    verslot_command()
        .arg("list")
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("list:"))
        .stderr(predicate::str::contains("is missing"));
}

#[test]
fn current_reports_storage_errors() {
    verslot_command()
        .arg("current")
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("current:"))
        .stderr(predicate::str::contains("is missing"));
}

#[test]
fn missing_command_reports_usage() {
    verslot_command()
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn target_commands_require_a_target() {
    for command in ["install", "uninstall", "use"] {
        verslot_command()
            .arg(command)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("<TOOL@VERSION>"));
    }
}

#[test]
fn target_commands_reject_extra_targets() {
    for command in ["install", "uninstall", "use"] {
        verslot_command()
            .args([command, "node@22.0.0", "node@24.0.0"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

#[test]
fn valid_use_targets_reach_storage_resolution() {
    for target in [
        "node@22.0.0",
        "node@0.0.0",
        "node@4294967295.4294967295.4294967295",
    ] {
        verslot_command()
            .args(["use", target])
            .env_remove("HOME")
            .env_remove("LOCALAPPDATA")
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains(format!("use {target}:")))
            .stderr(predicate::str::contains("is missing"));
    }
}

#[test]
fn invalid_targets_report_shared_diagnostics_before_execution() {
    use verslot::target::ParseTargetError::*;

    let cases = [
        ("", InvalidStructure),
        ("node", InvalidStructure),
        ("node@", InvalidStructure),
        ("@22.0.0", InvalidStructure),
        ("node@@22.0.0", InvalidStructure),
        ("Node@", InvalidStructure),
        ("unknown@22.0.0", UnsupportedTool),
        ("Node@22", UnsupportedTool),
        ("nodejs@22.0.0", UnsupportedTool),
        (" node@22.0.0", UnsupportedTool),
        ("../node@22.0.0", UnsupportedTool),
        ("node@22", ComponentCount),
        ("node@22.1", ComponentCount),
        ("node@1.2.3.4", ComponentCount),
        ("node@latest", ComponentCount),
        ("node@lts", ComponentCount),
        ("node@../22.0.0", ComponentCount),
        ("node@1..0", NonAsciiDigits),
        ("node@v22.0.0", NonAsciiDigits),
        ("node@22.0.0-beta", NonAsciiDigits),
        ("node@22.0.0+build", NonAsciiDigits),
        ("node@22.0.*", NonAsciiDigits),
        ("node@^22.0.0", NonAsciiDigits),
        ("node@１.0.0", NonAsciiDigits),
        ("node@22.0.0 ", NonAsciiDigits),
        ("node@22.\t0.0", NonAsciiDigits),
        ("node@1/2.0.0", NonAsciiDigits),
        ("node@1\\2.0.0", NonAsciiDigits),
        ("node@01.x.0", NonAsciiDigits),
        ("node@01.0.0", LeadingZeros),
        ("node@4294967296.01.0", LeadingZeros),
        ("node@4294967296.0.0", OutOfRange),
    ];

    for command in ["install", "uninstall", "use"] {
        for (target, error) in cases {
            verslot_command()
                .args([command, target])
                .assert()
                .code(2)
                .stdout("")
                .stderr(predicate::str::contains(error.to_string()))
                .stderr(predicate::str::contains("not implemented").not());
        }
    }
}

#[test]
fn failed_commands_preserve_existing_contents_and_list_does_not_create_storage() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("verslot-cli-{}-{nonce}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let sentinel = directory.join("sentinel");
    fs::write(&sentinel, b"unchanged").unwrap();

    let cases = [
        vec!["uninstall", "node@22.0.0"],
        vec!["use", "node@22.0.0"],
        vec!["list"],
        vec!["current"],
        vec!["install", "node@22"],
        vec!["uninstall", "node@22"],
        vec!["use", "node@22"],
    ];
    for arguments in &cases {
        let invalid = arguments.last() == Some(&"node@22");
        verslot_command()
            .current_dir(&directory)
            .env("HOME", &directory)
            .env("LOCALAPPDATA", &directory)
            .args(arguments)
            .assert()
            .code(if invalid {
                2
            } else if matches!(arguments[0], "list" | "current") {
                0
            } else {
                1
            })
            .stdout("");
        assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    }

    let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
    let installation = root.join("installs/node/22.0.0");
    let current = root.join("current/node");
    fs::create_dir_all(&installation).unwrap();
    fs::create_dir_all(current.parent().unwrap()).unwrap();
    let installed_file = installation.join("existing-content");
    fs::write(&installed_file, b"installed").unwrap();
    // List ignores current state; selection commands must preserve invalid entries.
    fs::write(&current, b"not a link").unwrap();
    for arguments in &cases {
        let invalid = arguments.last() == Some(&"node@22");
        verslot_command()
            .current_dir(&directory)
            .env("HOME", &directory)
            .env("LOCALAPPDATA", &directory)
            .args(arguments)
            .assert()
            .code(if invalid { 2 } else { 1 })
            .stdout("");
        assert_eq!(fs::read(&installed_file).unwrap(), b"installed");
        assert_eq!(fs::read(&current).unwrap(), b"not a link");
        assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        // Uninstall keeps its persistent mutation lock after rejecting invalid state.
        assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
        assert_eq!(fs::read_dir(&installation).unwrap().count(), 1);
    }

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn query_commands_reject_targets() {
    for command in ["list", "current"] {
        verslot_command()
            .args([command, "node@22.0.0"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

#[test]
fn uninstall_cli_protects_current_state_and_removes_only_complete_inactive_versions() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "verslot-uninstall-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
    let mut command = verslot_command();
    command
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory);
    command
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("not installed: node@22.0.0"));
    assert!(!root.exists());

    for version in ["22.0.0", "24.0.0"] {
        let target = format!("node@{version}")
            .parse::<verslot::target::Target>()
            .unwrap();
        let installation = root.join("installs/node").join(version);
        let executable = installation.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"offline fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let distribution =
            verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
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
    let receipt = root.join("installs/node/22.0.0/.verslot-install");
    let original_receipt = fs::read(&receipt).unwrap();
    fs::write(&receipt, b"incomplete installation").unwrap();
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("installation is not complete"));
    assert_eq!(fs::read(&receipt).unwrap(), b"incomplete installation");
    fs::write(&receipt, original_receipt).unwrap();
    let current = root.join("current/node");
    fs::create_dir_all(current.parent().unwrap()).unwrap();
    fs::write(&current, b"invalid current").unwrap();
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("read current state"));
    assert!(root.join("installs/node/22.0.0/.verslot-install").exists());
    assert_eq!(fs::read(&current).unwrap(), b"invalid current");
    fs::remove_file(&current).unwrap();
    let selected = root.join("installs/node/24.0.0");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&selected, &current).unwrap();
    #[cfg(windows)]
    junction::create(&selected, &current).unwrap();
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .args(["uninstall", "node@24.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "cannot uninstall current version: node@24.0.0",
        ));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".mutation.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("mutation lock"));
    drop(lock);
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .success()
        .stdout("uninstalled node@22.0.0\n")
        .stderr("");
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .arg("list")
        .assert()
        .success()
        .stdout("node@24.0.0\n")
        .stderr("");
    assert!(fs::symlink_metadata(&current).is_ok());
    assert!(selected.join(".verslot-install").exists());
    #[cfg(unix)]
    fs::remove_file(&current).unwrap();
    #[cfg(windows)]
    junction::delete(&current).unwrap();
    let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
    let resolved = fs::canonicalize(&directory).unwrap();
    assert_eq!(resolved.parent(), Some(temporary.as_path()));
    fs::remove_dir_all(resolved).unwrap();
}

#[test]
fn each_command_provides_help() {
    let binary_name = format!("verslot{}", std::env::consts::EXE_SUFFIX);
    for command in ["install", "uninstall", "use", "list", "current"] {
        let expected_usage = if matches!(command, "install" | "uninstall" | "use") {
            format!("Usage: {binary_name} {command} <TOOL@VERSION>")
        } else {
            format!("Usage: {binary_name} {command}")
        };

        verslot_command()
            .args([command, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains(expected_usage))
            .stderr("")
            .stdout(predicate::str::contains("not implemented yet").not());
    }
}

#[test]
fn install_and_list_preserve_invalid_entries_and_current_state() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "verslot-install-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
    let installation = root.join("installs/node/22.0.0");
    fs::create_dir_all(&installation).unwrap();
    fs::write(installation.join("sentinel"), b"preserved").unwrap();
    let current = root.join("current/node");
    fs::create_dir_all(current.parent().unwrap()).unwrap();
    fs::write(&current, b"invalid current state").unwrap();
    verslot_command()
        .args(["install", "node@22.0.0"])
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "existing destination is not a complete installation",
        ));
    assert_eq!(
        fs::read(installation.join("sentinel")).unwrap(),
        b"preserved"
    );
    assert!(!root.join("tmp").exists());
    verslot_command()
        .arg("list")
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("inspect node@22.0.0"));
    let executable = installation.join(if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    });
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, b"offline fixture").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let target = "node@22.0.0".parse::<verslot::target::Target>().unwrap();
    let distribution =
        verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
    fs::write(
        installation.join(".verslot-install"),
        format!(
            "verslot-install-v1\n{target}\n{}\n{}\n",
            distribution.archive_filename,
            "a".repeat(64)
        ),
    )
    .unwrap();
    verslot_command()
        .args(["install", "node@22.0.0"])
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .success()
        .stdout("already installed node@22.0.0\n")
        .stderr("");
    assert_eq!(fs::read(&current).unwrap(), b"invalid current state");
    assert!(!root.join("tmp").exists());
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".mutation.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    for version in ["10.0.0", "2.10.0", "2.2.10", "2.2.2"] {
        let target = format!("node@{version}")
            .parse::<verslot::target::Target>()
            .unwrap();
        let distribution =
            verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
        let destination = root.join("installs/node").join(version);
        let executable = destination.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"offline fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(
            destination.join(".verslot-install"),
            format!(
                "verslot-install-v1\n{target}\n{}\n{}\n",
                distribution.archive_filename,
                "a".repeat(64)
            ),
        )
        .unwrap();
    }
    fs::write(root.join("installs/node/02.0.0"), b"ignored").unwrap();
    verslot_command()
        .arg("list")
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .success()
        .stdout("node@2.2.2\nnode@2.2.10\nnode@2.10.0\nnode@10.0.0\nnode@22.0.0\n")
        .stderr("");
    assert_eq!(fs::read(&current).unwrap(), b"invalid current state");
    fs::create_dir(root.join("installs/node/24.0.0")).unwrap();
    verslot_command()
        .arg("list")
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("inspect node@24.0.0"));
    verslot_command()
        .args(["install", "node@22.0.0"])
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("mutation lock"));
    drop(lock);
    let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
    let resolved = fs::canonicalize(&directory).unwrap();
    assert_eq!(resolved.parent(), Some(temporary.as_path()));
    fs::remove_dir_all(resolved).unwrap();
}

#[test]
fn offline_cli_duplicate_list_uninstall_workflow_returns_empty_inventory() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let directory = std::env::temp_dir().join(format!(
        "verslot-workflow-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
    verslot_command()
        .env("HOME", &directory)
        .env("LOCALAPPDATA", &directory)
        .arg("list")
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert!(!root.exists());
    // Seed a complete offline fixture; a fresh production CLI install requires HTTPS.
    let installation = root.join("installs/node/22.0.0");
    let executable = installation.join(if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    });
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, b"offline fixture").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let target = "node@22.0.0".parse::<verslot::target::Target>().unwrap();
    let distribution =
        verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
    let receipt = format!(
        "verslot-install-v1\n{target}\n{}\n{}\n",
        distribution.archive_filename,
        "a".repeat(64)
    );
    fs::write(installation.join(".verslot-install"), &receipt).unwrap();
    for (arguments, stdout) in [
        (vec!["list"], "node@22.0.0\n"),
        (
            vec!["install", "node@22.0.0"],
            "already installed node@22.0.0\n",
        ),
        (vec!["list"], "node@22.0.0\n"),
        (
            vec!["uninstall", "node@22.0.0"],
            "uninstalled node@22.0.0\n",
        ),
        (vec!["list"], ""),
    ] {
        verslot_command()
            .env("HOME", &directory)
            .env("LOCALAPPDATA", &directory)
            .args(&arguments)
            .assert()
            .success()
            .stdout(stdout)
            .stderr("");
        if arguments[0] == "install" {
            assert_eq!(
                fs::read_to_string(installation.join(".verslot-install")).unwrap(),
                receipt
            );
            assert_eq!(fs::read(&executable).unwrap(), b"offline fixture");
        }
    }
    assert!(!installation.exists());
    assert!(!root.join("current").exists());
    assert_eq!(fs::read_dir(root.join("tmp")).unwrap().count(), 0);
    let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
    let resolved = fs::canonicalize(&directory).unwrap();
    assert_eq!(resolved.parent(), Some(temporary.as_path()));
    fs::remove_dir_all(resolved).unwrap();
}
