use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use assert_cmd::assert::OutputAssertExt;

#[path = "shim_cli/doctor.rs"]
mod doctor;

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn write_node_fixture(executable: &Path) {
    fs::write(executable, tool_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "verslot shim 路径 {}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        let root = directory.join(if cfg!(windows) { "verslot" } else { ".verslot" });
        Self { directory, root }
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(executable);
        command
            .current_dir(&self.directory)
            .env("HOME", &self.directory)
            .env("LOCALAPPDATA", &self.directory)
            .env("NO_COLOR", "1");
        command
    }

    fn manager(&self) -> Command {
        self.command(assert_cmd::cargo::cargo_bin!("verslot"))
    }

    fn shim_path(&self, name: &str) -> PathBuf {
        self.root.join("shims").join(executable_name(name))
    }

    fn shim(&self, name: &str) -> Command {
        self.command(self.shim_path(name))
    }

    fn setup(&self) {
        self.manager().arg("setup").assert().success().stderr("");
    }

    fn assert_shim_commands(&self, commands: &[&str]) {
        let mut actual: Vec<_> = fs::read_dir(self.root.join("shims"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let mut expected: Vec<_> = commands
            .iter()
            .map(|name| OsString::from(executable_name(name)))
            .collect();
        expected.push(".verslot-shims".into());
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }

    fn seed(&self, version: &str) -> PathBuf {
        let target: verslot::target::Target = format!("node@{version}").parse().unwrap();
        let distribution =
            verslot::distribution::NodeDistribution::for_current_build(target.version).unwrap();
        let installation = self.root.join("installs/node").join(version);
        let executable = installation.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        write_node_fixture(&executable);
        fs::write(
            installation.join(".verslot-install"),
            format!(
                "verslot-install-v1\n{target}\n{}\n{}\n",
                distribution.archive_filename,
                "a".repeat(64)
            ),
        )
        .unwrap();
        installation
    }

    fn select(&self, version: &str) {
        self.manager()
            .args(["use", &format!("node@{version}")])
            .assert()
            .success();
    }

    fn seed_yarn(&self, version: &str) -> PathBuf {
        let installation = self.root.join("installs/yarn").join(version);
        fs::create_dir_all(installation.join("bin")).unwrap();
        fs::write(installation.join("bin/yarn.js"), format!("yarn@{version}")).unwrap();
        let classic = version.starts_with("1.");
        let package = if classic { "yarn" } else { "@yarnpkg/cli-dist" };
        let archive = if classic { "yarn" } else { "cli-dist" };
        fs::write(
            installation.join("package.json"),
            format!(r#"{{"name":"{package}","version":"{version}"}}"#),
        )
        .unwrap();
        fs::write(
            installation.join(".verslot-install"),
            format!(
                "verslot-install-v2\nyarn@{version}\n{archive}-{version}.tgz\n{}\n",
                "a".repeat(128)
            ),
        )
        .unwrap();
        installation
    }

    fn finish(self) {
        for tool in ["node", "yarn"] {
            let current = self.root.join("current").join(tool);
            if fs::symlink_metadata(&current).is_ok() {
                #[cfg(unix)]
                fs::remove_file(current).unwrap();
                #[cfg(windows)]
                junction::delete(current).unwrap();
            }
        }
        assert_eq!(
            self.directory.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(self.directory).unwrap();
    }
}

#[test]
fn setup_synchronizes_shims_with_installed_tools() {
    let fixture = Fixture::new();
    fixture.setup();
    fixture.assert_shim_commands(&["verslot"]);

    fixture.seed("22.0.0");
    fixture.setup();
    fixture.assert_shim_commands(&["verslot", "node", "npm", "npx"]);

    fixture.seed_yarn("4.9.2");
    fixture.setup();
    let all_commands = ["verslot", "node", "npm", "npx", "yarn", "yarnpkg"];
    fixture.assert_shim_commands(&all_commands);

    fixture
        .manager()
        .args(["uninstall", "node@22.0.0"])
        .assert()
        .success();
    fixture.assert_shim_commands(&all_commands);
    fixture.setup();
    fixture.assert_shim_commands(&["verslot", "yarn", "yarnpkg"]);

    fixture
        .manager()
        .args(["uninstall", "yarn@4.9.2"])
        .assert()
        .success();
    fixture.setup();
    fixture.assert_shim_commands(&["verslot"]);
    fixture.finish();
}

#[test]
fn setup_preserves_published_shims_when_an_installed_version_is_corrupt() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    let receipt = fixture.root.join("shims/.verslot-shims");
    let published = fs::read(&receipt).unwrap();

    fixture.seed_yarn("4.9.2");
    fs::create_dir(fixture.root.join("installs/node/24.0.0")).unwrap();
    fixture
        .manager()
        .arg("setup")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains("inspect node@24.0.0"));

    fixture.assert_shim_commands(&["verslot", "node", "npm", "npx"]);
    assert_eq!(fs::read(receipt).unwrap(), published);
    fixture.finish();
}

#[test]
fn yarn_and_yarnpkg_use_managed_node_on_path_and_inherit_yarn_settings() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.seed("24.0.0");
    fixture.seed_yarn("1.22.22");
    fixture.seed_yarn("4.9.2");
    fixture.setup();
    fixture.select("22.0.0");
    let path = fixture.root.join("shims");
    for version in ["1.22.22", "4.9.2"] {
        fixture
            .manager()
            .args(["use", &format!("yarn@{version}")])
            .assert()
            .success();
        for name in ["yarn", "yarnpkg"] {
            fixture
                .shim(name)
                .arg("yarn-env")
                .env("PATH", &path)
                .env("YARN_IGNORE_PATH", "0")
                .env("SKIP_YARN_COREPACK_CHECK", "native-value")
                .assert()
                .success()
                .stdout(format!(
                    "bundled=yarn@{version};node=22.0.0\nignore_path=0;corepack_check=native-value\n"
                ));
        }
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout(format!("   node@22.0.0\n   yarn@{version}\n"));
    }
    fixture.select("24.0.0");
    fixture
        .shim("yarn")
        .args(["exit", "37"])
        .env("PATH", &path)
        .assert()
        .code(37)
        .stdout("bundled=yarn@4.9.2;node=24.0.0\n");
    fixture
        .shim("yarn")
        .arg("--version")
        .env("PATH", &fixture.directory)
        .assert()
        .code(1)
        .stdout("");
    fixture.finish();
}

#[test]
fn yarn_uses_system_node_on_path_without_a_managed_node_selection() {
    let fixture = Fixture::new();
    fixture.seed_yarn("4.9.2");
    fixture.setup();
    fixture.manager().args(["use", "yarn@4"]).assert().success();
    fixture
        .manager()
        .arg("current")
        .assert()
        .success()
        .stdout("   yarn@4.9.2\n");
    let system = fixture.directory.join("system");
    fs::create_dir(&system).unwrap();
    write_node_fixture(&system.join(executable_name("node")));
    fixture
        .shim("yarn")
        .arg("yarn-env")
        .env("PATH", system)
        .env_remove("YARN_IGNORE_PATH")
        .env("SKIP_YARN_COREPACK_CHECK", "native-value")
        .assert()
        .success()
        .stdout(
            "bundled=yarn@4.9.2;node=system\nignore_path=<unset>;corepack_check=native-value\n",
        );
    fixture.finish();
}

#[test]
fn yarn_versions_list_select_and_uninstall_independently_of_same_numbered_node() {
    let fixture = Fixture::new();
    fixture.setup();
    fixture.seed("4.9.2");
    fixture.select("4.9.2");
    fixture.seed_yarn("4.9.1");
    fixture.seed_yarn("4.9.2");
    fixture.seed_yarn("4.10.0");
    fixture
        .manager()
        .args(["use", "yarn@4.9"])
        .assert()
        .success();
    fixture
        .manager()
        .arg("list")
        .assert()
        .success()
        .stdout("   node@4.9.2\n   yarn@4.9.1\n   yarn@4.9.2\n   yarn@4.10.0\n");
    fixture
        .manager()
        .args(["uninstall", "yarn@4.9.2"])
        .assert()
        .code(1);
    fixture.manager().args(["use", "yarn@4"]).assert().success();
    fixture
        .manager()
        .args(["uninstall", "yarn@4.9.2"])
        .assert()
        .success();
    fixture
        .manager()
        .arg("current")
        .assert()
        .success()
        .stdout("   node@4.9.2\n   yarn@4.10.0\n");
    assert!(fixture.root.join("installs/node/4.9.2").exists());
    assert!(!fixture.root.join("installs/yarn/4.9.2").exists());
    fixture.finish();
}

#[test]
fn partial_selection_does_not_read_another_tools_damaged_inventory_directory() {
    for (tool, version, other) in [("node", "22.0.0", "yarn"), ("yarn", "4.9.2", "node")] {
        let fixture = Fixture::new();
        if tool == "node" {
            fixture.seed(version);
        } else {
            fixture.seed_yarn(version);
        }
        fs::write(
            fixture.root.join("installs").join(other),
            b"damaged unrelated inventory",
        )
        .unwrap();
        fixture
            .manager()
            .args([
                "use",
                &format!("{tool}@{}", version.split('.').next().unwrap()),
            ])
            .assert()
            .success();
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout(format!("   {tool}@{version}\n"));
        fixture.finish();
    }
}

#[test]
fn damaged_yarn_does_not_disable_node_or_fall_back_to_another_yarn() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    let yarn = fixture.seed_yarn("4.9.2");
    fixture.setup();
    fixture
        .manager()
        .args(["use", "yarn@4.9.2"])
        .assert()
        .success();
    fs::remove_file(yarn.join("bin/yarn.js")).unwrap();
    fixture
        .shim("yarn")
        .arg("--version")
        .assert()
        .code(1)
        .stdout("");
    fixture
        .shim("node")
        .arg("--version")
        .assert()
        .success()
        .stdout("v22.0.0\n");
    fixture.finish();
}

fn tool_bytes() -> &'static [u8] {
    static TOOL: OnceLock<Vec<u8>> = OnceLock::new();
    TOOL.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("verslot-shim-compiler-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let executable = directory.join(executable_name("fixture"));
        let output = Command::new("rustc")
            .args(["--edition=2024", "-C", "debuginfo=0"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shim_tool.rs"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(&executable).unwrap();
        // rustc may leave linker artifacts beside the isolated test executable.
        assert_eq!(directory.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(directory).unwrap();
        bytes
    })
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timeout waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn wait_for_controller(path: &Path, controller: &mut std::process::Child) {
    use std::io::Read;
    let deadline = Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        if let Some(status) = controller.try_wait().unwrap() {
            let mut diagnostic = String::new();
            controller
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut diagnostic)
                .unwrap();
            panic!("controller exited {status}: {diagnostic}");
        }
        assert!(
            Instant::now() < deadline,
            "timeout waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn one_path_exposes_manager_and_selected_node_without_cargo_path() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    // Invoke a system shell by absolute path with only our shim directory on PATH.
    #[cfg(windows)]
    let shell = std::env::var_os("COMSPEC").unwrap();
    #[cfg(unix)]
    let shell = OsString::from("/bin/sh");
    let mut command = fixture.command(shell);
    command.env("PATH", fixture.root.join("shims"));
    #[cfg(windows)]
    command.args(["/d", "/c", "verslot --version && node --version"]);
    #[cfg(unix)]
    command.args(["-c", "verslot --version && node --version"]);
    command
        .assert()
        .success()
        .stdout(format!(
            "   verslot {}\nv22.0.0\n",
            env!("CARGO_PKG_VERSION")
        ))
        .stderr("");
    fixture.manager().arg("setup").assert().success();
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_setup_prints_one_line_with_a_normal_path() {
    let fixture = Fixture::new();
    let directory = fixture.root.join("shims");
    let extended = directory.to_str().unwrap();
    let expected = extended.strip_prefix(r"\\?\").unwrap();
    fixture
        .manager()
        .arg("setup")
        .assert()
        .success()
        .stdout(format!(
            "✔  Configured shims at {expected} (put it first on PATH)\n"
        ))
        .stderr("");
    fixture.finish();
}

#[test]
fn shim_never_falls_back_when_selection_or_bundled_command_is_missing() {
    let fixture = Fixture::new();
    let installation = fixture.seed("22.0.0");
    fixture.setup();
    let outside = fixture.directory.join("system");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join(executable_name("node")), tool_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            outside.join(executable_name("node")),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fixture
        .shim("node")
        .env("PATH", &outside)
        .arg("--version")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains("no Node.js version selected"));
    fixture.select("22.0.0");
    fixture
        .shim("npx")
        .env("PATH", &outside)
        .arg("--version")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains("no usable bundled npx"));
    fixture
        .shim("node")
        .arg("--version")
        .assert()
        .success()
        .stdout("v22.0.0\n");
    fs::write(installation.join(".verslot-install"), b"broken").unwrap();
    fixture
        .shim("node")
        .env("PATH", &outside)
        .assert()
        .code(1)
        .stdout("");
    fixture.shim("verslot").arg("--version").assert().success();
    #[cfg(windows)]
    {
        // Invalid PE startup is distinct from a tool returning a nonzero status.
        let installation = fixture.seed("22.0.0");
        fs::write(installation.join("node.exe"), b"invalid executable").unwrap();
        fixture
            .shim("node")
            .env("PATH", &outside)
            .arg("--version")
            .assert()
            .code(1)
            .stdout("");
    }
    fixture.finish();
}

#[test]
fn bundled_npm_and_npx_use_the_selected_node_and_explicit_script() {
    let fixture = Fixture::new();
    for version in ["22.0.0", "24.0.0"] {
        let installation = fixture.seed(version);
        fixture.setup();
        let npm = installation.join(if cfg!(windows) {
            "node_modules/npm/bin"
        } else {
            "lib/node_modules/npm/bin"
        });
        fs::create_dir_all(&npm).unwrap();
        for name in ["npm", "npx"] {
            fs::write(npm.join(format!("{name}-cli.js")), name).unwrap();
        }
        fixture.select(version);
        for name in ["npm", "npx"] {
            fixture
                .shim(name)
                .arg("--version")
                .assert()
                .success()
                .stdout(format!("bundled={name};node={version}\nv{version}\n"))
                .stderr("");
        }
    }
    fixture.finish();
}

#[test]
fn shim_preserves_arguments_environment_directory_streams_and_nonzero_exit() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    let arguments: Vec<OsString> = [
        "",
        "a b",
        "a\"b",
        "中文",
        "$()&;|<>%!*",
        "--help",
        "--",
        "trailing\\",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let mut child = fixture
        .shim("node")
        .arg("inspect")
        .args(&arguments)
        .env("VERSLOT_FIXTURE_VALUE", "inherited value")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin bytes\0\xff")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(37));
    let mut expected = format!(
        "version=22.0.0\ncwd={:?}\nenv=inherited value\nargs={arguments:?}\n",
        fixture.directory
    )
    .into_bytes();
    expected.extend_from_slice(b"stdin bytes\0\xff");
    // Windows can represent cwd using a normal or extended-length absolute path.
    assert_eq!(output.stdout, expected);
    assert_eq!(output.stderr, b"tool stderr\n");
    fixture.finish();
}

#[test]
fn a_running_tool_does_not_block_switching_or_change_its_own_version() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.seed("24.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    let ready = fixture.directory.join("ready");
    let mut child = fixture
        .shim("node")
        .arg("block")
        .arg(&ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&ready);
    fixture.select("24.0.0");
    fixture
        .shim("node")
        .arg("--version")
        .assert()
        .success()
        .stdout("v24.0.0\n");
    child.stdin.take().unwrap().write_all(b"finished").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"v22.0.0:finished\n");
    fixture.finish();
}

#[test]
fn unuse_allows_uninstalling_the_only_selected_version_and_keeps_shims() {
    for (tool, version, commands, error) in [
        (
            "node",
            "22.0.0",
            &["node", "npm", "npx"][..],
            "no Node.js version selected",
        ),
        (
            "yarn",
            "4.9.2",
            &["yarn", "yarnpkg"][..],
            "no Yarn version selected",
        ),
    ] {
        let fixture = Fixture::new();
        if tool == "node" {
            fixture.seed(version);
        } else {
            fixture.seed_yarn(version);
        }
        fixture.setup();
        let target = format!("{tool}@{version}");
        fixture.manager().args(["use", &target]).assert().success();
        fixture
            .manager()
            .args(["uninstall", &target])
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicates::str::contains(format!("verslot unuse {tool}")));
        fixture
            .manager()
            .args(["unuse", tool])
            .assert()
            .success()
            .stdout(format!("✔  Unselected {tool}\n"))
            .stderr("");
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout("");
        fixture
            .manager()
            .arg("list")
            .assert()
            .success()
            .stdout(format!("   {target}\n"));
        fixture
            .manager()
            .args(["unuse", tool])
            .assert()
            .success()
            .stdout(format!("   Already unselected {tool}\n"))
            .stderr("");

        let system = fixture.directory.join("system");
        fs::create_dir(&system).unwrap();
        for name in commands {
            write_node_fixture(&system.join(executable_name(name)));
            fixture
                .shim(name)
                .arg("--version")
                .env("PATH", &system)
                .assert()
                .code(1)
                .stdout("")
                .stderr(predicates::str::contains(error));
        }
        fixture
            .manager()
            .args(["uninstall", &target])
            .assert()
            .success()
            .stdout(format!("✔  Uninstalled {target}\n"));
        fixture.manager().arg("list").assert().success().stdout("");
        for name in commands {
            assert!(fixture.shim_path(name).is_file());
        }
        fixture.shim("verslot").arg("--help").assert().success();
        fixture.finish();
    }
}

#[test]
fn unuse_keeps_tool_selections_independent_and_allows_reselection() {
    let fixture = Fixture::new();
    let node = fixture.seed("22.0.0");
    let yarn = fixture.seed_yarn("4.9.2");
    fixture.select("22.0.0");
    fixture.manager().args(["use", "yarn@4"]).assert().success();
    for (tool, remaining, target) in [
        ("node", "yarn@4.9.2", "node@22.0.0"),
        ("yarn", "node@22.0.0", "yarn@4.9.2"),
    ] {
        fixture.manager().args(["unuse", tool]).assert().success();
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout(format!("   {remaining}\n"));
        assert!(node.join(".verslot-install").is_file());
        assert!(yarn.join(".verslot-install").is_file());
        fixture.manager().args(["use", target]).assert().success();
        fixture
            .manager()
            .arg("current")
            .assert()
            .success()
            .stdout("   node@22.0.0\n   yarn@4.9.2\n");
    }
    fixture.finish();
}

#[test]
fn unuse_clears_damaged_and_dangling_selections_without_touching_installations() {
    for (tool, version) in [("node", "22.0.0"), ("yarn", "4.9.2")] {
        for damage in ["receipt", "executable", "missing"] {
            let fixture = Fixture::new();
            let installation = if tool == "node" {
                fixture.seed(version)
            } else {
                fixture.seed_yarn(version)
            };
            fs::write(installation.join("sentinel"), b"preserved").unwrap();
            fixture
                .manager()
                .args(["use", &format!("{tool}@{version}")])
                .assert()
                .success();
            let retained = if damage == "missing" {
                let detached = fixture.directory.join("detached-installation");
                fs::rename(&installation, &detached).unwrap();
                detached
            } else {
                let damaged = if damage == "receipt" {
                    installation.join(".verslot-install")
                } else if tool == "yarn" {
                    installation.join("bin/yarn.js")
                } else {
                    installation.join(if cfg!(windows) {
                        "node.exe"
                    } else {
                        "bin/node"
                    })
                };
                fs::remove_file(damaged).unwrap();
                installation
            };

            fixture
                .manager()
                .args(["unuse", tool])
                .assert()
                .success()
                .stdout(format!("✔  Unselected {tool}\n"))
                .stderr("");
            assert!(matches!(
                fs::symlink_metadata(fixture.root.join("current").join(tool)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            ));
            assert_eq!(fs::read(retained.join("sentinel")).unwrap(), b"preserved");
            fixture
                .manager()
                .arg("current")
                .assert()
                .success()
                .stdout("");
            fixture.finish();
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_new_executable_refreshes_the_manager_and_every_tool_entry() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    let mut upgraded = fs::read(assert_cmd::cargo::cargo_bin!("verslot")).unwrap();
    // PE overlays change the generation digest without another executable build.
    upgraded.extend_from_slice(b"shim upgrade fixture");
    let source = fixture.directory.join(executable_name("upgraded-verslot"));
    fs::write(&source, &upgraded).unwrap();
    fixture.command(&source).arg("setup").assert().success();
    for name in ["verslot", "node", "npm", "npx"] {
        assert_eq!(fs::read(fixture.shim_path(name)).unwrap(), upgraded);
    }
    fixture.shim("verslot").arg("--version").assert().success();
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_setup_warns_when_only_previous_shim_cleanup_is_blocked() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.select("22.0.0");
    fixture.setup();
    let ready = fixture.directory.join("ready");
    let mut running = fixture
        .shim("node")
        .arg("block")
        .arg(&ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_file(&ready);

    fixture.seed_yarn("4.9.2");
    let backup = fixture.root.join(".shims-previous");
    for _ in 0..2 {
        fixture
            .manager()
            .arg("setup")
            .assert()
            .success()
            .stdout(predicates::str::contains("Configured shims at"))
            .stderr(predicates::str::contains(
                "warning: previous shim cleanup is pending",
            ))
            .stderr(predicates::str::contains(".shims-previous"));
        assert!(backup.join(".verslot-shims").is_file());
        fixture.assert_shim_commands(&["verslot", "node", "npm", "npx", "yarn", "yarnpkg"]);
    }

    fixture
        .manager()
        .args(["uninstall", "yarn@4.9.2"])
        .assert()
        .success();
    fixture
        .manager()
        .arg("setup")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains(
            "refresh blocked by pending cleanup",
        ))
        .stderr(predicates::str::contains("Current shims remain at"));
    fixture.assert_shim_commands(&["verslot", "node", "npm", "npx", "yarn", "yarnpkg"]);

    drop(running.stdin.take());
    assert!(running.wait().unwrap().success());
    fixture.setup();
    fixture.assert_shim_commands(&["verslot", "node", "npm", "npx"]);
    assert!(!backup.exists());
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_setup_preserves_a_locked_entry_then_succeeds_after_release() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    let path = fixture.shim_path("node");
    let original = fs::read(&path).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    fixture
        .manager()
        .arg("setup")
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicates::str::contains(
            "close related processes and retry",
        ));
    drop(lock);
    assert_eq!(fs::read(&path).unwrap(), original);
    fixture.manager().arg("setup").assert().success();
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_shim_preserves_full_width_exit_codes() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    fixture
        .shim("node")
        .args(["exit", "-1073741510"])
        .assert()
        .code(-1073741510)
        .stdout("")
        .stderr("");
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn unix_exec_preserves_non_utf8_arguments() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    let argument = OsString::from_vec(vec![0xff, b'a']);
    let output = fixture
        .shim("node")
        .arg("inspect")
        .arg(&argument)
        .env("VERSLOT_FIXTURE_VALUE", "test")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(37));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains(&format!("args={:?}", [argument]))
    );
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn unix_ctrl_c_reaches_the_tool_and_waits_for_its_cleanup() {
    let fixture = Fixture::new();
    fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    let ready = fixture.directory.join("ready");
    let interrupted = fixture.directory.join("interrupted");
    let release = fixture.directory.join("release");
    let mut child = fixture
        .shim("node")
        .arg("interrupt")
        .args([&ready, &interrupted, &release])
        .spawn()
        .unwrap();
    wait_for_file(&ready);
    Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    wait_for_file(&interrupted);
    assert!(child.try_wait().unwrap().is_none());
    fs::write(release, b"finish").unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(37));
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn windows_ctrl_c_keeps_the_wrapper_waiting_for_tool_cleanup() {
    let fixture = Fixture::new();
    let installation = fixture.seed("22.0.0");
    fixture.setup();
    fixture.select("22.0.0");
    let node = installation.join("node.exe");
    fs::remove_file(&node).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut controller = fixture
        .command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(fixtures.join("windows_interrupt.ps1"))
        .arg("-Source")
        .arg(fixtures.join("interrupt_tool.cs"))
        .arg("-Executable")
        .arg(&node)
        .arg("-Shim")
        .arg(fixture.shim_path("node"))
        .arg("-Directory")
        .arg(&fixture.directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_controller(&fixture.directory.join("ready"), &mut controller);
    fs::write(fixture.directory.join("send"), b"interrupt").unwrap();
    wait_for_controller(&fixture.directory.join("wrapper-alive"), &mut controller);
    fs::write(fixture.directory.join("release"), b"finish").unwrap();
    let output = controller.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.directory.join("cleaned").exists());
    fixture.finish();
}
