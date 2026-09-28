use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::distribution::NodeDistribution;

struct Fixture {
    base: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot-uninstall-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let storage = Storage::from_source(Some(base.as_os_str())).unwrap();
        Self { base, storage }
    }

    fn root(&self) -> &Path {
        self.storage.root()
    }

    fn target(&self) -> Target {
        "node@22.0.0".parse().unwrap()
    }

    fn install(&self, version: &str) -> PathBuf {
        let target = format!("node@{version}").parse::<Target>().unwrap();
        let directory = self.root().join("installs/node").join(version);
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
        let current = self.root().join("current/node");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        link_directory(directory, &current);
    }

    fn uninstall(&self) -> Result<String, String> {
        uninstall_at(&self.storage, &self.target(), remove_operation)
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

#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) {
    junction::create(target, link).unwrap();
}

fn remove_link(link: &Path) {
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
    #[cfg(windows)]
    junction::delete(link).unwrap();
}

#[test]
fn uninstall_removes_only_the_inactive_installation_and_own_operation() {
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let other = fixture.install("24.0.0");
    fixture.select(&other);
    let orphan = fixture.root().join("tmp/orphan");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("sentinel"), b"preserved").unwrap();
    assert_eq!(fixture.uninstall().unwrap(), "uninstalled node@22.0.0");
    assert!(!directory.exists());
    validate_installation(fixture.root(), &"node@24.0.0".parse().unwrap()).unwrap();
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some("node@24.0.0".parse::<Target>().unwrap().version)
    );
    assert_eq!(fs::read(orphan.join("sentinel")).unwrap(), b"preserved");
    assert_eq!(fs::read_dir(fixture.root().join("tmp")).unwrap().count(), 1);
    remove_link(&fixture.root().join("current/node"));
}

#[test]
fn selected_versions_are_protected_even_without_a_receipt() {
    for complete in [false, true] {
        let fixture = Fixture::new();
        let directory = fixture.install("22.0.0");
        if !complete {
            fs::remove_file(directory.join(".verslot-install")).unwrap();
        }
        fixture.select(&directory);
        assert_eq!(
            fixture.uninstall().unwrap_err(),
            "cannot uninstall current version: node@22.0.0"
        );
        assert!(directory.exists());
        assert!(!fixture.root().join("tmp").exists());
        assert_eq!(
            fixture.storage.read_current().unwrap(),
            Some(fixture.target().version)
        );
        remove_link(&fixture.root().join("current/node"));
    }
}

#[test]
fn invalid_current_state_blocks_unrelated_removal() {
    for state in ["file", "outside", "dangling"] {
        let fixture = Fixture::new();
        let directory = fixture.install("22.0.0");
        let current = fixture.root().join("current/node");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        if state == "file" {
            fs::write(&current, b"invalid state").unwrap();
        } else {
            let outside = fixture.base.join("outside");
            fs::create_dir(&outside).unwrap();
            link_directory(&outside, &current);
            if state == "dangling" {
                fs::remove_dir(&outside).unwrap();
            }
        }
        assert!(
            fixture
                .uninstall()
                .unwrap_err()
                .contains("read current state")
        );
        assert!(
            uninstall_at(
                &fixture.storage,
                &"node@26.0.0".parse().unwrap(),
                remove_operation
            )
            .unwrap_err()
            .contains("read current state")
        );
        validate_installation(fixture.root(), &fixture.target()).unwrap();
        assert!(directory.exists());
        assert!(!fixture.root().join("tmp").exists());
        if state != "file" {
            remove_link(&current);
        }
    }
}

#[test]
fn missing_and_incomplete_installations_are_preserved() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.uninstall().unwrap_err(),
        "not installed: node@22.0.0"
    );
    assert!(!fixture.root().exists());
    fs::create_dir(fixture.root()).unwrap();
    assert_eq!(
        fixture.uninstall().unwrap_err(),
        "not installed: node@22.0.0"
    );
    assert!(!fixture.root().join("tmp").exists());
    let directory = fixture.install("22.0.0");
    fs::write(directory.join(".verslot-install"), b"malformed receipt").unwrap();
    assert!(
        fixture
            .uninstall()
            .unwrap_err()
            .contains("installation is not complete")
    );
    assert_eq!(
        fs::read(directory.join(".verslot-install")).unwrap(),
        b"malformed receipt"
    );
    assert!(!fixture.root().join("tmp").exists());
    let resolved = fs::canonicalize(&directory).unwrap();
    assert!(resolved.starts_with(fixture.root()));
    fs::remove_dir_all(resolved).unwrap();
    fs::write(&directory, b"not an installation directory").unwrap();
    assert!(
        fixture
            .uninstall()
            .unwrap_err()
            .contains("installation is not complete")
    );
    assert_eq!(
        fs::read(&directory).unwrap(),
        b"not an installation directory"
    );
}

#[test]
fn lock_contention_preserves_the_installation() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    let lock = acquire_lock(fixture.root()).unwrap();
    assert!(fixture.uninstall().unwrap_err().contains("mutation lock"));
    validate_installation(fixture.root(), &fixture.target()).unwrap();
    drop(lock);
    fixture.uninstall().unwrap();
}

#[test]
fn partial_cleanup_failure_reports_detachment_and_retains_the_leftover_path() {
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let other = fixture.install("24.0.0");
    fixture.select(&other);
    let error = uninstall_at(&fixture.storage, &fixture.target(), |root, operation| {
        // The lock must remain held throughout cleanup.
        assert!(acquire_lock(root).is_err());
        let removed = operation.join("removed");
        assert!(!directory.exists());
        fs::remove_file(removed.join(".verslot-install"))?;
        Err(io::Error::other("injected partial deletion failure"))
    })
    .unwrap_err();
    assert!(error.contains("installation detached but cleanup incomplete at"));
    let operation = fs::read_dir(fixture.root().join("tmp"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(error.contains(&operation.display().to_string()));
    assert!(operation.join("removed").exists());
    assert!(!directory.exists());
    validate_installation(fixture.root(), &"node@24.0.0".parse().unwrap()).unwrap();
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some("node@24.0.0".parse::<Target>().unwrap().version)
    );
    assert_eq!(
        fixture.uninstall().unwrap_err(),
        "not installed: node@22.0.0"
    );
    remove_link(&fixture.root().join("current/node"));
}

#[test]
fn linked_installation_ancestors_destination_and_tmp_are_rejected() {
    for component in ["installs", "installs/node", "installs/node/22.0.0", "tmp"] {
        let fixture = Fixture::new();
        if component == "tmp" {
            fixture.install("22.0.0");
        } else {
            fs::create_dir_all(fixture.root()).unwrap();
        }
        let outside = fixture.base.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"preserved").unwrap();
        let link = fixture.root().join(component);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        link_directory(&outside, &link);
        assert!(fixture.uninstall().is_err());
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
        if component == "tmp" {
            validate_installation(fixture.root(), &fixture.target()).unwrap();
        }
        remove_link(&link);
    }
}

#[test]
fn linked_storage_root_is_supported() {
    let fixture = Fixture::new();
    let real_root = fixture.base.join("real-storage");
    fs::create_dir(&real_root).unwrap();
    link_directory(&real_root, fixture.root());
    fixture.install("22.0.0");
    fixture.uninstall().unwrap();
    assert!(!real_root.join("installs/node/22.0.0").exists());
    remove_link(fixture.root());
}

#[test]
fn switch_residue_blocks_uninstall_without_removing_installations() {
    for name in [".node-next", ".node-previous"] {
        let fixture = Fixture::new();
        let directory = fixture.install("22.0.0");
        fs::create_dir(fixture.root().join("current")).unwrap();
        let residue = fixture.root().join("current").join(name);
        link_directory(&directory, &residue);
        let error = fixture.uninstall().unwrap_err();
        assert!(error.contains("unfinished switch"));
        assert!(error.contains(&residue.display().to_string()));
        validate_installation(fixture.root(), &fixture.target()).unwrap();
        assert!(!fixture.root().join("tmp").exists());
        assert!(fs::symlink_metadata(&residue).is_ok());
        remove_link(&residue);
    }
}

#[test]
fn cleanup_never_traverses_payload_links() {
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let outside = fixture.base.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserved").unwrap();
    link_directory(&outside, &directory.join("payload-link"));
    let result = fixture.uninstall();
    #[cfg(unix)]
    assert_eq!(result.unwrap(), "uninstalled node@22.0.0");
    #[cfg(windows)]
    {
        assert!(
            result
                .unwrap_err()
                .contains("installation detached but cleanup incomplete")
        );
        let operation = fs::read_dir(fixture.root().join("tmp"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        remove_link(&operation.join("removed/payload-link"));
        remove_operation(fixture.root(), &operation).unwrap();
    }
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
}

#[cfg(windows)]
#[test]
fn windows_locked_payload_preserves_installation_when_detachment_fails() {
    use std::os::windows::fs::OpenOptionsExt;
    for cleanup_failure in [false, true] {
        let fixture = Fixture::new();
        let directory = fixture.install("22.0.0");
        let payload = directory.join("locked-payload");
        fs::write(&payload, b"preserved").unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&payload)
            .unwrap();
        let error = uninstall_at(&fixture.storage, &fixture.target(), |root, operation| {
            if cleanup_failure {
                Err(io::Error::other("injected cleanup failure"))
            } else {
                remove_operation(root, operation)
            }
        })
        .unwrap_err();
        assert!(error.contains("detach installation"));
        if cleanup_failure {
            assert!(error.contains("cleanup failed at"));
        }
        validate_installation(fixture.root(), &fixture.target()).unwrap();
        assert_eq!(
            fs::read_dir(fixture.root().join("tmp")).unwrap().count(),
            usize::from(cleanup_failure)
        );
        drop(held);
        assert_eq!(fs::read(payload).unwrap(), b"preserved");
        fixture.uninstall().unwrap();
    }
}

#[cfg(windows)]
#[test]
fn windows_locked_detached_payload_reports_native_cleanup_failure() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let payload = directory.join("locked-payload");
    fs::write(&payload, b"preserved").unwrap();
    let error = uninstall_at(&fixture.storage, &fixture.target(), |root, operation| {
        let _held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(operation.join("removed/locked-payload"))?;
        remove_operation(root, operation)
    })
    .unwrap_err();
    assert!(error.contains("installation detached but cleanup incomplete at"));
    assert!(!directory.exists());
    let operation = fs::read_dir(fixture.root().join("tmp"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let payload = operation.join("removed/locked-payload");
    assert_eq!(fs::read(&payload).unwrap(), b"preserved");
    remove_operation(fixture.root(), &operation).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_detachment_permission_failure_preserves_installation_when_enforced() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let parent = directory.parent().unwrap();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o555)).unwrap();
    let result = fixture.uninstall();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o755)).unwrap();
    // Privileged runners may rename despite the directory permission bits.
    if let Err(error) = result {
        assert!(error.contains("detach installation"));
        validate_installation(fixture.root(), &fixture.target()).unwrap();
        assert_eq!(fs::read_dir(fixture.root().join("tmp")).unwrap().count(), 0);
    }
}
