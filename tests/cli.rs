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
        .stdout(predicate::str::contains("not implemented yet"))
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
fn install_is_explicitly_not_implemented_yet() {
    verslot_command()
        .args(["install", "node@22.0.0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not implemented: install node@22.0.0",
        ));
}

#[test]
fn uninstall_is_explicitly_not_implemented_yet() {
    verslot_command()
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not implemented: uninstall node@22.0.0",
        ));
}

#[test]
fn use_is_explicitly_not_implemented_yet() {
    verslot_command()
        .args(["use", "node@22.0.0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented: use node@22.0.0"));
}

#[test]
fn list_is_explicitly_not_implemented_yet() {
    verslot_command()
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented: list"));
}

#[test]
fn current_is_explicitly_not_implemented_yet() {
    verslot_command()
        .arg("current")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented: current"));
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
fn valid_targets_retain_placeholder_behavior() {
    for command in ["install", "uninstall", "use"] {
        for target in [
            "node@22.0.0",
            "node@0.0.0",
            "node@4294967295.4294967295.4294967295",
        ] {
            verslot_command()
                .args([command, target])
                .assert()
                .code(1)
                .stdout("")
                .stderr(format!("not implemented: {command} {target}\n"));
        }
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
fn commands_leave_storage_and_working_directory_unchanged() {
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
        vec!["install", "node@22.0.0"],
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
            .code(if invalid { 2 } else { 1 })
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
    // Even invalid state must not be read by placeholder commands.
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
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
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
fn query_commands_report_unimplemented_status() {
    for command in ["list", "current"] {
        verslot_command()
            .arg(command)
            .assert()
            .code(1)
            .stdout("")
            .stderr(format!("not implemented: {command}\n"));
    }
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
            .stdout(predicate::str::contains("not implemented yet"))
            .stderr("");
    }
}
