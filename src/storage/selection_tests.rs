use super::*;
use crate::distribution::NodeDistribution;
use crate::mutation::acquire_lock;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn assert_other_processes_are_busy(base: &Path) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::selection_tests::mutations_during_switch_child",
            "--nocapture",
        ])
        .env("VERSLOT_TEST_SWITCH_CONTENTION", "1")
        .env("HOME", base)
        .env("LOCALAPPDATA", base)
        .output()
        .unwrap();
    assert!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).contains("switch contention checked"),
        "contention child failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn mutations_during_switch_child() {
    if std::env::var_os("VERSLOT_TEST_SWITCH_CONTENTION").is_none() {
        return;
    }
    let storage = Storage::from_env().unwrap();
    let first: Target = "node@22.0.0".parse().unwrap();
    let second: Target = "node@24.0.0".parse().unwrap();
    for error in [
        storage.select(&second).unwrap_err().to_string(),
        storage.read_selected().unwrap_err().to_string(),
        crate::install::install(&first).unwrap_err(),
        crate::uninstall::uninstall(&first).unwrap_err(),
    ] {
        assert!(error.contains("storage is busy"), "{error}");
    }
    println!("switch contention checked");
}

struct Fixture {
    base: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot-selection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let storage = Storage::from_source(Some(base.as_os_str())).unwrap();
        Self { base, storage }
    }

    fn installation(&self) -> PathBuf {
        let target = "node@22.0.0".parse::<Target>().unwrap();
        let directory = self.storage.root().join("installs/node/22.0.0");
        let executable = directory.join(if cfg!(windows) {
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
        let distribution = NodeDistribution::for_current_build(target.version).unwrap();
        fs::write(
            directory.join(".verslot-install"),
            format!(
                "verslot-install-v1\n{target}\n{}\n{}\n",
                distribution.archive_filename,
                "a".repeat(64)
            ),
        )
        .unwrap();
        directory
    }

    fn select(&self, directory: &Path) {
        fs::create_dir_all(self.storage.root().join("current")).unwrap();
        create_link(directory, &self.storage.root().join("current/node"));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        if self.base.parent() == Some(temporary.as_path())
            && fs::canonicalize(&self.base).ok().as_ref() == Some(&self.base)
        {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
}

fn create_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    junction::create(target, link).unwrap();
}

#[test]
fn empty_queries_create_nothing_with_or_without_a_lock() {
    let fixture = Fixture::new();
    assert_eq!(fixture.storage.read_selected().unwrap(), None);
    assert!(!fixture.storage.root().exists());
    fs::create_dir(fixture.storage.root()).unwrap();
    assert_eq!(fixture.storage.read_selected().unwrap(), None);
    assert_eq!(fs::read_dir(fixture.storage.root()).unwrap().count(), 0);
    fs::create_dir(fixture.storage.root().join("current")).unwrap();
    assert_eq!(fixture.storage.read_selected().unwrap(), None);
    assert!(!fixture.storage.root().join(".mutation.lock").exists());
    drop(acquire_lock(fixture.storage.root()).unwrap());
    assert_eq!(fixture.storage.read_selected().unwrap(), None);
    assert_eq!(
        fs::read(fixture.storage.root().join(".mutation.lock")).unwrap(),
        b""
    );
}

#[test]
fn complete_queries_require_an_existing_lock_and_preserve_its_contents() {
    let fixture = Fixture::new();
    let directory = fixture.installation();
    fixture.select(&directory);
    assert!(
        fixture
            .storage
            .read_selected()
            .unwrap_err()
            .to_string()
            .contains("without mutation lock")
    );
    let path = fixture.storage.root().join(".mutation.lock");
    assert!(!path.exists());
    fs::write(&path, b"lock sentinel").unwrap();
    let version = "node@22.0.0".parse::<Target>().unwrap().version;
    assert_eq!(fixture.storage.read_selected().unwrap(), Some(version));
    assert_eq!(fs::read(path).unwrap(), b"lock sentinel");
}

#[test]
fn structural_state_remains_distinct_from_complete_selection() {
    for damage in ["receipt", "platform", "executable", "empty"] {
        let fixture = Fixture::new();
        let directory = fixture.installation();
        fixture.select(&directory);
        drop(acquire_lock(fixture.storage.root()).unwrap());
        let executable = directory.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        match damage {
            "receipt" => fs::remove_file(directory.join(".verslot-install")).unwrap(),
            "platform" => fs::write(
                directory.join(".verslot-install"),
                format!(
                    "verslot-install-v1\nnode@22.0.0\n{}\n{}\n",
                    if cfg!(windows) {
                        "node-v22.0.0-linux-x64.tar.gz"
                    } else {
                        "node-v22.0.0-win-x64.zip"
                    },
                    "a".repeat(64)
                ),
            )
            .unwrap(),
            "executable" => fs::remove_file(executable).unwrap(),
            "empty" => fs::write(executable, b"").unwrap(),
            _ => unreachable!(),
        }
        assert!(fixture.storage.read_current().unwrap().is_some());
        assert!(
            fixture
                .storage
                .read_selected()
                .unwrap_err()
                .to_string()
                .contains("current installation is not complete")
        );
    }
}

#[test]
fn invalid_current_entries_never_become_empty_selection() {
    for state in [
        "file",
        "directory",
        "dangling",
        "outside",
        "nested",
        "invalid-version",
        "loop",
    ] {
        let fixture = Fixture::new();
        let directory = fixture.installation();
        drop(acquire_lock(fixture.storage.root()).unwrap());
        let current = fixture.storage.root().join("current/node");
        fs::create_dir(current.parent().unwrap()).unwrap();
        match state {
            "file" => fs::write(&current, b"invalid").unwrap(),
            "directory" => fs::create_dir(&current).unwrap(),
            "dangling" => {
                create_link(&directory, &current);
                fs::remove_dir_all(&directory).unwrap();
            }
            "loop" => {
                #[cfg(unix)]
                create_link(Path::new("node"), &current);
                #[cfg(windows)]
                create_link(&current, &current);
            }
            _ => {
                let destination = match state {
                    "outside" => fixture.base.join("outside"),
                    "nested" => directory.join("nested"),
                    _ => directory.parent().unwrap().join("22"),
                };
                fs::create_dir(&destination).unwrap();
                create_link(&destination, &current);
            }
        }
        assert!(fixture.storage.read_selected().is_err(), "{state}");
    }
}

#[test]
fn residues_are_errors_even_without_a_lock_or_current_selection() {
    for name in [".node-next", ".node-previous"] {
        for kind in ["file", "directory", "link", "dangling"] {
            let fixture = Fixture::new();
            let directory = fixture.installation();
            fs::create_dir(fixture.storage.root().join("current")).unwrap();
            let residue = fixture.storage.root().join("current").join(name);
            match kind {
                "file" => fs::write(&residue, b"preserved").unwrap(),
                "directory" => fs::create_dir(&residue).unwrap(),
                _ => {
                    create_link(&directory, &residue);
                    if kind == "dangling" {
                        fs::remove_dir_all(&directory).unwrap();
                    }
                }
            }
            let error = fixture.storage.read_selected().unwrap_err().to_string();
            assert!(error.contains("unfinished switch"));
            assert!(error.contains(&residue.display().to_string()));
            assert!(fs::symlink_metadata(&residue).is_ok());
            assert!(!fixture.storage.root().join(".mutation.lock").exists());
            assert!(
                acquire_lock(fixture.storage.root())
                    .unwrap_err()
                    .to_string()
                    .contains("unfinished switch")
            );
            assert!(
                fixture
                    .storage
                    .read_selected()
                    .unwrap_err()
                    .to_string()
                    .contains("unfinished switch")
            );
        }
    }
}

#[test]
fn readers_share_locks_and_exclude_writers_without_truncation() {
    let fixture = Fixture::new();
    let directory = fixture.installation();
    fixture.select(&directory);
    drop(acquire_lock(fixture.storage.root()).unwrap());
    let first = acquire_read_lock(fixture.storage.root()).unwrap().unwrap();
    let second = acquire_read_lock(fixture.storage.root()).unwrap().unwrap();
    assert!(fixture.storage.read_selected().unwrap().is_some());
    assert!(
        acquire_lock(fixture.storage.root())
            .unwrap_err()
            .to_string()
            .contains("storage is busy")
    );
    drop(first);
    assert!(acquire_lock(fixture.storage.root()).is_err());
    drop(second);
    let writer = acquire_lock(fixture.storage.root()).unwrap();
    assert!(
        fixture
            .storage
            .read_complete_current(fixture.storage.root())
            .unwrap()
            .is_some()
    );
    assert!(
        fixture
            .storage
            .read_selected()
            .unwrap_err()
            .to_string()
            .contains("storage is busy")
    );
    // Simulate the Windows publish gap while the writer owns the lock.
    fs::rename(
        fixture.storage.root().join("current/node"),
        fixture.storage.root().join("current/.node-previous"),
    )
    .unwrap();
    assert!(
        fixture
            .storage
            .read_selected()
            .unwrap_err()
            .to_string()
            .contains("storage is busy")
    );
    drop(writer);
    assert!(
        fixture
            .storage
            .read_selected()
            .unwrap_err()
            .to_string()
            .contains("unfinished switch")
    );
}

#[test]
fn residue_beside_a_complete_selection_is_not_silently_ignored() {
    let fixture = Fixture::new();
    let directory = fixture.installation();
    fixture.select(&directory);
    drop(acquire_lock(fixture.storage.root()).unwrap());
    fs::write(
        fixture.storage.root().join("current/.node-next"),
        b"preserved",
    )
    .unwrap();
    assert!(fixture.storage.read_current().unwrap().is_some());
    assert!(
        fixture
            .storage
            .read_selected()
            .unwrap_err()
            .to_string()
            .contains("unfinished switch")
    );
}

#[test]
fn complete_queries_support_a_linked_root_but_reject_linked_descendants() {
    let fixture = Fixture::new();
    let real_root = fixture.base.join("real-storage");
    fs::create_dir(&real_root).unwrap();
    create_link(&real_root, fixture.storage.root());
    let directory = fixture.installation();
    fixture.select(&fs::canonicalize(&directory).unwrap());
    drop(acquire_lock(&fs::canonicalize(fixture.storage.root()).unwrap()).unwrap());
    assert!(fixture.storage.read_selected().unwrap().is_some());
    let node = real_root.join("installs/node");
    let retained = real_root.join("installs/retained");
    fs::rename(&node, &retained).unwrap();
    create_link(&retained, &node);
    assert!(fixture.storage.read_current().unwrap().is_some());
    assert!(fixture.storage.read_selected().is_err());
    #[cfg(unix)]
    {
        fs::remove_file(node).unwrap();
        fs::remove_file(fixture.storage.root()).unwrap();
    }
    #[cfg(windows)]
    {
        junction::delete(node).unwrap();
        junction::delete(fixture.storage.root()).unwrap();
    }
}

#[test]
fn invalid_lock_files_and_current_ancestors_are_preserved() {
    for state in [
        "lock-directory",
        "lock-link",
        "current-file",
        "current-link",
    ] {
        let fixture = Fixture::new();
        fs::create_dir(fixture.storage.root()).unwrap();
        let outside = fixture.base.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"preserved").unwrap();
        match state {
            "lock-directory" => {
                fs::create_dir(fixture.storage.root().join(".mutation.lock")).unwrap()
            }
            "lock-link" => create_link(&outside, &fixture.storage.root().join(".mutation.lock")),
            "current-file" => {
                fs::write(fixture.storage.root().join("current"), b"preserved").unwrap()
            }
            _ => create_link(&outside, &fixture.storage.root().join("current")),
        }
        assert!(fixture.storage.read_selected().is_err(), "{state}");
        assert!(acquire_lock(fixture.storage.root()).is_err(), "{state}");
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
    }
}

#[cfg(unix)]
#[test]
fn complete_query_accepts_existing_relative_links() {
    let fixture = Fixture::new();
    fixture.installation();
    drop(acquire_lock(fixture.storage.root()).unwrap());
    fixture.select(Path::new("../installs/node/22.0.0"));
    assert!(fixture.storage.read_selected().unwrap().is_some());
}
