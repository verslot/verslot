use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::assert::OutputAssertExt;
use predicates::prelude::*;

use super::{Fixture, executable_name, write_node_fixture};

fn doctor(fixture: &Fixture, directories: &[&Path]) -> Command {
    let mut command = fixture.manager();
    command
        .arg("doctor")
        .env("PATH", std::env::join_paths(directories).unwrap())
        .env("PATHEXT", ".exe;.cmd");
    command
}

fn select_node_with_bundles(fixture: &Fixture) -> PathBuf {
    let installation = fixture.seed("22.0.0");
    let scripts = installation.join(if cfg!(windows) {
        "node_modules/npm/bin"
    } else {
        "lib/node_modules/npm/bin"
    });
    fs::create_dir_all(&scripts).unwrap();
    for name in ["npm", "npx"] {
        fs::write(scripts.join(format!("{name}-cli.js")), name).unwrap();
    }
    fixture.select("22.0.0");
    scripts
}

fn select_yarn(fixture: &Fixture) {
    fixture.seed_yarn("4.9.2");
    fixture
        .manager()
        .args(["use", "yarn@4.9.2"])
        .assert()
        .success();
}

fn external_node(fixture: &Fixture) -> PathBuf {
    let directory = fixture.directory.join("external bin");
    fs::create_dir(&directory).unwrap();
    write_node_fixture(&directory.join(executable_name("node")));
    directory
}

fn display_path(path: &Path) -> &str {
    let path = path.to_str().unwrap();
    if cfg!(windows) {
        path.strip_prefix(r"\\?\").unwrap_or(path)
    } else {
        path
    }
}

#[test]
fn unconfigured_and_installed_but_unselected_environments_are_read_only() {
    let fixture = Fixture::new();
    for installed in [false, true] {
        if installed {
            fixture.seed("22.0.0");
        }
        doctor(&fixture, &[&fixture.directory])
            .assert()
            .success()
            .stderr("")
            .stdout(concat!(
                "   Verslot is not configured yet.\n",
                "   Run `verslot setup` and follow its PATH instructions.\n"
            ));
        assert_eq!(fixture.root.exists(), installed);
        for entry in ["shims", "current", ".mutation.lock"] {
            assert!(!fixture.root.join(entry).exists());
        }
    }
    fixture.finish();
}

#[test]
fn setup_without_a_selection_suggests_use_and_install() {
    let fixture = Fixture::new();
    fixture.setup();

    doctor(&fixture, &[&fixture.root.join("shims")])
        .assert()
        .success()
        .stderr("")
        .stdout(concat!(
            "   No tools selected.\n",
            "   Run `verslot use <tool>@<version>` to select an installed version.\n",
            "   Run `verslot install <tool>@<version>` first if needed.\n"
        ));
    fixture.finish();
}

#[test]
fn healthy_shims_accept_a_cargo_cli_and_are_inspected_without_launching_tools() {
    let fixture = Fixture::new();
    fixture.setup();
    select_node_with_bundles(&fixture);
    select_yarn(&fixture);
    let shims = fixture.root.join("shims");
    let receipt = shims.join(".verslot-shims");
    fs::remove_file(&receipt).unwrap();
    let lock = fixture.root.join(".mutation.lock");
    fs::write(&lock, b"original lock contents").unwrap();
    let cargo = fixture.directory.join("cargo bin");
    fs::create_dir(&cargo).unwrap();
    let cargo_cli = cargo.join(executable_name("verslot"));
    fs::copy(fixture.shim_path("verslot"), &cargo_cli).unwrap();
    let marker = fixture.directory.join("launched");

    doctor(&fixture, &[&cargo, &shims])
        .env_remove("PATHEXT")
        .env("VERSLOT_FIXTURE_LAUNCH_MARKER", &marker)
        .assert()
        .success()
        .stderr("")
        .stdout("   No issues found.\n");
    let details = doctor(&fixture, &[&cargo, &shims])
        .arg("--verbose")
        .env_remove("PATHEXT")
        .env("VERSLOT_FIXTURE_LAUNCH_MARKER", &marker)
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::str::contains(format!(
            "Storage: {}",
            display_path(&fixture.root)
        )))
        .stdout(predicate::str::contains(format!(
            "Shims: {}",
            display_path(&shims)
        )))
        .stdout(predicate::str::contains("node: selected node@22.0.0"))
        .stdout(predicate::str::contains("yarn: selected yarn@4.9.2"))
        .stdout(predicate::str::contains(format!(
            "verslot: {}",
            display_path(&cargo_cli)
        )))
        .stdout(predicate::str::contains(format!(
            "Yarn Node.js: {} (Verslot shim)",
            display_path(&fixture.shim_path("node"))
        )))
        .stdout(predicate::str::contains('\x1b').not());
    let stdout = std::str::from_utf8(&details.get_output().stdout).unwrap();
    assert!(stdout.lines().all(|line| {
        line.strip_prefix("   ")
            .is_some_and(|message| !message.starts_with(' '))
    }));

    assert!(!marker.exists());
    assert!(!receipt.exists());
    assert_eq!(fs::read(&lock).unwrap(), b"original lock contents");
    fixture
        .manager()
        .arg("current")
        .assert()
        .success()
        .stdout("   node@22.0.0\n   yarn@4.9.2\n");
    fixture.finish();
}

#[test]
fn missing_required_shims_and_off_path_shims_report_recovery_on_stdout() {
    let fixture = Fixture::new();
    fixture.setup();
    select_node_with_bundles(&fixture);
    doctor(&fixture, &[&fixture.directory])
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains(
            "Shims not found on PATH: node, npm, npx.",
        ))
        .stdout(predicate::str::contains("first on PATH").count(1))
        .stdout(predicate::str::contains("PATH matches").not());

    fs::remove_file(fixture.shim_path("npm")).unwrap();
    fs::remove_file(fixture.shim_path("npx")).unwrap();
    fs::create_dir(fixture.shim_path("npx")).unwrap();
    let shims = fixture.root.join("shims");

    doctor(&fixture, &[&shims])
        .assert()
        .code(1)
        .stderr("")
        .stdout(concat!(
            "   Error: Missing or unusable shims: npm, npx.\n",
            "   Run `verslot setup` from the installed Verslot executable.\n",
            "   If setup rejects the existing shim directory, move it aside and retry.\n"
        ));
    fixture.finish();
}

#[test]
fn missing_bundled_scripts_are_informational() {
    let fixture = Fixture::new();
    fixture.setup();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");

    doctor(&fixture, &[&fixture.root.join("shims")])
        .assert()
        .success()
        .stderr("")
        .stdout(concat!(
            "   Info: bundled npm unavailable for node@22.0.0.\n",
            "   Info: bundled npx unavailable for node@22.0.0.\n"
        ));
    fixture.finish();
}

#[test]
fn selected_yarn_can_use_external_node_with_unselected_shims_missing() {
    let fixture = Fixture::new();
    fixture.setup();
    select_yarn(&fixture);
    for name in ["node", "npm", "npx"] {
        fs::remove_file(fixture.shim_path(name)).unwrap();
    }
    let shims = fixture.root.join("shims");
    let external = external_node(&fixture);

    doctor(&fixture, &[&shims, &external])
        .arg("--verbose")
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::str::contains("node: unselected"))
        .stdout(predicate::str::contains("npm: not found"))
        .stdout(predicate::str::contains(format!(
            "Yarn Node.js: {} (external Node.js)",
            display_path(&external.join(executable_name("node")))
        )));
    doctor(&fixture, &[&shims])
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains(
            "Yarn Node.js: no Node.js found on PATH",
        ));
    #[cfg(unix)]
    doctor(&fixture, &[&shims])
        .env_remove("PATH")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "Yarn Node.js: cannot check PATH (PATH is unset)",
        ));
    fixture.finish();
}

#[test]
fn node_shadowing_does_not_hide_a_usable_yarn_dependency() {
    let fixture = Fixture::new();
    fixture.setup();
    select_node_with_bundles(&fixture);
    select_yarn(&fixture);
    let external = external_node(&fixture);

    doctor(&fixture, &[&external, &fixture.root.join("shims")])
        .arg("--verbose")
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains("node: shadowed by"))
        .stdout(predicate::str::contains(format!(
            "Yarn Node.js: {} (external Node.js)",
            display_path(&external.join(executable_name("node")))
        )));
    fixture.finish();
}

#[test]
fn yarn_does_not_skip_a_node_shim_with_no_valid_selection() {
    let fixture = Fixture::new();
    fixture.setup();
    select_yarn(&fixture);
    let external = external_node(&fixture);
    let shims = fixture.root.join("shims");
    doctor(&fixture, &[&shims, &external])
        .assert()
        .code(1)
        .stderr("")
        .stdout(concat!(
            "   Error: Yarn found a Node.js shim without a valid selection.\n",
            "   Run `verslot use node@<version>` or put a system Node.js directory first on PATH.\n"
        ));

    let installation = fixture.seed("22.0.0");
    fixture.select("22.0.0");
    let receipt = installation.join(".verslot-install");
    fs::write(&receipt, b"broken receipt").unwrap();
    doctor(&fixture, &[&shims, &external])
        .arg("--verbose")
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains("node: invalid selection"))
        .stdout(predicate::str::contains(
            "current installation is not complete",
        ))
        .stdout(predicate::str::contains("yarn: selected yarn@4.9.2"))
        .stdout(predicate::str::contains(
            "Yarn found a Node.js shim without a valid selection",
        ));
    assert_eq!(fs::read(receipt).unwrap(), b"broken receipt");
    fixture.finish();
}

#[test]
fn invalid_selection_entry_leaves_other_findings_available() {
    let fixture = Fixture::new();
    fixture.setup();
    select_yarn(&fixture);
    let current = fixture.root.join("current/node");
    fs::write(&current, b"invalid selection entry").unwrap();
    let external = external_node(&fixture);

    doctor(&fixture, &[&external, &fixture.root.join("shims")])
        .arg("--verbose")
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains("node: invalid selection"))
        .stdout(predicate::str::contains("node: unselected").not())
        .stdout(predicate::str::contains("yarn: selected yarn@4.9.2"))
        .stdout(predicate::str::contains("(external Node.js)"));
    assert_eq!(fs::read(&current).unwrap(), b"invalid selection entry");
    fs::remove_file(current).unwrap();
    fixture.finish();
}

#[test]
fn a_busy_storage_still_reports_path_matches() {
    let fixture = Fixture::new();
    fixture.setup();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.root.join(".mutation.lock"))
        .unwrap();
    lock.try_lock().unwrap();

    doctor(&fixture, &[&fixture.root.join("shims")])
        .arg("--verbose")
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains("storage is busy"))
        .stdout(predicate::str::contains("node: cannot check selection"))
        .stdout(predicate::str::contains("yarn: cannot check selection"))
        .stdout(predicate::str::contains(format!(
            "yarnpkg: {}",
            display_path(&fixture.shim_path("yarnpkg"))
        )));
    drop(lock);
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn pathext_applies_in_each_directory_but_yarn_searches_for_node_exe() {
    let fixture = Fixture::new();
    fixture.setup();
    select_node_with_bundles(&fixture);
    select_yarn(&fixture);
    let earlier = fixture.directory.join("earlier");
    fs::create_dir(&earlier).unwrap();
    for name in ["node.cmd", "npm.cmd", "npm.exe", "npx.ps1"] {
        fs::write(earlier.join(name), b"fixture").unwrap();
    }
    let shims = fixture.root.join("shims");

    doctor(&fixture, &[&earlier, &shims])
        .arg("--verbose")
        .env("PATHEXT", ".cmd;.exe")
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains(format!(
            "node: {}",
            display_path(&earlier.join("node.cmd"))
        )))
        .stdout(predicate::str::contains(format!(
            "npm: {}",
            display_path(&earlier.join("npm.cmd"))
        )))
        .stdout(predicate::str::contains(format!(
            "npx: {}",
            display_path(&fixture.shim_path("npx"))
        )))
        .stdout(predicate::str::contains(format!(
            "Yarn Node.js: {} (Verslot shim)",
            display_path(&fixture.shim_path("node"))
        )));
    doctor(&fixture, &[&earlier, &shims])
        .arg("--verbose")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(format!(
            "node: {}",
            display_path(&earlier.join("node.cmd"))
        )))
        .stdout(predicate::str::contains(format!(
            "npm: {}",
            display_path(&earlier.join("npm.exe"))
        )));
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn unix_path_resolves_empty_and_relative_entries_and_checks_execute_bits() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let local_node = fixture.directory.join("node");
    fs::write(&local_node, b"fixture").unwrap();
    let external = external_node(&fixture);
    let relative = Path::new(external.file_name().unwrap());

    doctor(&fixture, &[Path::new(""), relative])
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "node: {}",
            display_path(&external.join("node"))
        )));
    fs::set_permissions(&local_node, fs::Permissions::from_mode(0o755)).unwrap();
    doctor(&fixture, &[Path::new(""), relative])
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "node: {}",
            display_path(&local_node)
        )));
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn a_path_symlink_to_the_node_shim_still_requires_a_selection() {
    let fixture = Fixture::new();
    fixture.setup();
    select_yarn(&fixture);
    let linked = fixture.directory.join("linked bin");
    fs::create_dir(&linked).unwrap();
    std::os::unix::fs::symlink(fixture.shim_path("node"), linked.join("node")).unwrap();
    let shims = fixture.root.join("shims");
    let path = [linked.as_path(), shims.as_path()];

    doctor(&fixture, &path)
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "Yarn found a Node.js shim without a valid selection",
        ));
    select_node_with_bundles(&fixture);
    doctor(&fixture, &path).assert().success().stderr("");
    fixture.finish();
}
