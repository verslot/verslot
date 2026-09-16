use assert_cmd::Command;
use predicates::prelude::*;

fn verslot_command() -> Command {
    Command::cargo_bin("verslot").expect("verslot binary should be available")
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
        .stdout(predicate::str::contains("current"));
}

#[test]
fn version_reports_the_package_version() {
    verslot_command()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("verslot 0.1.0"));
}

#[test]
fn invalid_command_fails() {
    verslot_command()
        .arg("invalid-command")
        .assert()
        .failure()
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
