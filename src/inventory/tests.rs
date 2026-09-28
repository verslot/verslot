use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::distribution::NodeDistribution;

struct Fixture {
    base: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot-inventory-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let root = base.join("storage");
        Self { base, root }
    }

    fn install(&self, version: &str) -> PathBuf {
        let target = format!("node@{version}").parse::<Target>().unwrap();
        let distribution = NodeDistribution::for_current_build(target.version).unwrap();
        let directory = self.root.join("installs/node").join(version);
        let executable = directory.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"offline node fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
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

#[test]
fn missing_and_empty_inventory_paths_do_not_create_storage() {
    let fixture = Fixture::new();
    assert!(read_installations(&fixture.root).unwrap().is_empty());
    assert!(!fixture.root.exists());
    fs::create_dir(&fixture.root).unwrap();
    assert!(read_installations(&fixture.root).unwrap().is_empty());
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    fs::create_dir(fixture.root.join("installs")).unwrap();
    assert!(read_installations(&fixture.root).unwrap().is_empty());
    assert!(!fixture.root.join("installs/node").exists());
    fs::create_dir(fixture.root.join("installs/node")).unwrap();
    assert!(read_installations(&fixture.root).unwrap().is_empty());
    assert!(!fixture.root.join("tmp").exists());
    assert!(!fixture.root.join(".mutation.lock").exists());
}

#[test]
fn versions_sort_numerically_and_ignore_noncanonical_entries_and_other_state() {
    let fixture = Fixture::new();
    for version in ["22.0.0", "2.10.0", "10.0.0", "2.2.10", "2.2.2"] {
        fixture.install(version);
    }
    for name in ["22", "v22.0.0", "02.0.0", "22.0.0-beta", "4294967296.0.0"] {
        fs::write(fixture.root.join("installs/node").join(name), b"ignored").unwrap();
    }
    fs::create_dir_all(fixture.root.join("tmp/orphan/staging")).unwrap();
    fs::create_dir(fixture.root.join("current")).unwrap();
    fs::write(fixture.root.join("current/node"), b"broken selection").unwrap();
    for name in [".node-next", ".node-previous"] {
        fs::write(fixture.root.join("current").join(name), b"switch residue").unwrap();
    }
    let versions: Vec<_> = read_installations(&fixture.root)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        versions,
        [
            "node@2.2.2",
            "node@2.2.10",
            "node@2.10.0",
            "node@10.0.0",
            "node@22.0.0"
        ]
    );
    assert_eq!(
        fs::read(fixture.root.join("current/node")).unwrap(),
        b"broken selection"
    );
}

#[test]
fn canonical_entries_require_complete_installations() {
    let fixture = Fixture::new();
    fixture.install("2.0.0");
    let directory = fixture.install("22.0.0");
    let receipt = directory.join(".verslot-install");
    fs::remove_file(&receipt).unwrap();
    let error = read_installations(&fixture.root).unwrap_err();
    assert!(error.to_string().contains("inspect node@22.0.0"));
    fs::write(&receipt, b"invalid receipt").unwrap();
    assert!(read_installations(&fixture.root).is_err());
    fixture.install("22.0.0");
    let executable = directory.join(if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    });
    fs::write(&executable, b"").unwrap();
    assert!(read_installations(&fixture.root).is_err());
    fixture.install("22.0.0");
    fs::write(
        fixture.root.join("installs/node/10.0.0"),
        b"not a directory",
    )
    .unwrap();
    assert!(read_installations(&fixture.root).is_err());
}

#[test]
fn files_in_inventory_ancestors_are_errors_not_empty_inventory() {
    for component in ["", "installs", "installs/node"] {
        let fixture = Fixture::new();
        let path = if component.is_empty() {
            fixture.root.clone()
        } else {
            fixture.root.join(component)
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"preserved").unwrap();
        assert!(read_installations(&fixture.root).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"preserved");
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

#[test]
fn root_links_are_resolved_and_internal_links_are_rejected() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    let root_link = fixture.base.join("root-link");
    link_directory(&fixture.root, &root_link);
    assert_eq!(read_installations(&root_link).unwrap().len(), 1);
    #[cfg(windows)]
    junction::delete(&root_link).unwrap();
    #[cfg(unix)]
    fs::remove_file(&root_link).unwrap();

    let ignored_link = fixture.root.join("installs/node/not-a-version");
    link_directory(&fixture.base, &ignored_link);
    assert_eq!(read_installations(&fixture.root).unwrap().len(), 1);
    #[cfg(windows)]
    junction::delete(&ignored_link).unwrap();
    #[cfg(unix)]
    fs::remove_file(&ignored_link).unwrap();

    for component in ["installs", "installs/node", "installs/node/22.0.0"] {
        let fixture = Fixture::new();
        fs::create_dir(&fixture.root).unwrap();
        let destination = fixture.base.join("outside");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"preserved").unwrap();
        let link = fixture.root.join(component);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        link_directory(&destination, &link);
        assert!(read_installations(&fixture.root).is_err());
        assert_eq!(
            fs::read(destination.join("sentinel")).unwrap(),
            b"preserved"
        );
        #[cfg(windows)]
        junction::delete(&link).unwrap();
        #[cfg(unix)]
        fs::remove_file(&link).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn dangling_links_and_non_utf8_names_follow_inventory_policy() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    let directory = fixture.root.join("installs/node");
    #[cfg(not(target_os = "macos"))]
    {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let ignored = directory.join(OsString::from_vec(vec![0xff]));
        std::os::unix::fs::symlink("missing", &ignored).unwrap();
    }
    assert_eq!(read_installations(&fixture.root).unwrap().len(), 1);
    std::os::unix::fs::symlink("missing", directory.join("10.0.0")).unwrap();
    assert!(read_installations(&fixture.root).is_err());
    let dangling_root = fixture.base.join("dangling-root");
    std::os::unix::fs::symlink("missing", &dangling_root).unwrap();
    assert!(read_installations(&dangling_root).is_err());
}

#[cfg(windows)]
#[test]
fn receipt_read_sharing_failure_is_reported() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(directory.join(".verslot-install"))
        .unwrap();
    assert!(read_installations(&fixture.root).is_err());
    drop(locked);
    assert_eq!(read_installations(&fixture.root).unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn receipt_permission_failure_is_reported_when_enforced() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    let receipt = directory.join(".verslot-install");
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o000)).unwrap();
    // Privileged runners can bypass permissions and cannot provide this evidence.
    if fs::File::open(&receipt).is_err() {
        assert!(read_installations(&fixture.root).is_err());
    }
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(read_installations(&fixture.root).unwrap().len(), 1);
    let inventory = fixture.root.join("installs/node");
    fs::set_permissions(&inventory, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&inventory).is_err() {
        assert!(read_installations(&fixture.root).is_err());
    }
    fs::set_permissions(&inventory, fs::Permissions::from_mode(0o700)).unwrap();
}
