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
        .args(["install", "node@22"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented: install node@22"));
}

#[test]
fn uninstall_is_explicitly_not_implemented_yet() {
    verslot_command()
        .args(["uninstall", "node@22"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not implemented: uninstall node@22",
        ));
}

#[test]
fn use_is_explicitly_not_implemented_yet() {
    verslot_command()
        .args(["use", "node@22"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented: use node@22"));
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
            .args([command, "node@22", "node@24"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

#[test]
fn target_parsing_is_deferred() {
    for command in ["install", "uninstall", "use"] {
        for target in ["node@22", "node", "unknown@version", "node@", "@22"] {
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
fn query_commands_reject_targets() {
    for command in ["list", "current"] {
        verslot_command()
            .args([command, "node@22"])
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
