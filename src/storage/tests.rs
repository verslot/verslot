use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ROOT_DIRECTORY, ROOT_VARIABLE, Storage};
use crate::target::Target;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("verslot-storage-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
    }

    fn storage(&self) -> Storage {
        Storage::from_source(Some(self.0.as_os_str())).unwrap()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Only this test's atomically allocated directory is removed. std does
        // not traverse symbolic links or Windows junctions during removal.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn directory_link(target: &Path, link: &Path) {
    junction::create(target, link).unwrap();
}

fn target() -> Target {
    "node@22.0.0".parse().unwrap()
}

#[test]
fn root_sources_must_be_present_and_absolute() {
    assert!(Storage::from_source(None).is_err());
    for source in ["", "relative", ".", "..", "~/home", "$HOME"] {
        assert!(Storage::from_source(Some(OsStr::new(source))).is_err());
    }
}

#[test]
fn environment_root_is_read_in_an_isolated_process() {
    const CASE_VARIABLE: &str = "VERSLOT_TEST_ROOT_CASE";
    if let Some(case) = std::env::var_os(CASE_VARIABLE) {
        let result = Storage::from_env();
        if case == "valid" {
            let source = std::env::var_os(ROOT_VARIABLE).unwrap();
            assert_eq!(
                result.unwrap().root(),
                Path::new(&source).join(ROOT_DIRECTORY)
            );
        } else {
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
        }
        return;
    }

    let temporary = TestDirectory::new();
    for (case, source) in [
        ("missing", None),
        ("empty", Some(OsStr::new(""))),
        ("relative", Some(OsStr::new("relative"))),
        ("valid", Some(temporary.0.as_os_str())),
    ] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "storage::tests::environment_root_is_read_in_an_isolated_process",
                "--nocapture",
            ])
            .current_dir(&temporary.0)
            .env(CASE_VARIABLE, case)
            .env("HOME", &temporary.0)
            .env("LOCALAPPDATA", &temporary.0)
            .env_remove(ROOT_VARIABLE);
        if let Some(source) = source {
            command.env(ROOT_VARIABLE, source);
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
    }
    assert_eq!(fs::read_dir(&temporary.0).unwrap().count(), 0);
}

#[test]
fn native_root_and_version_paths_are_constructed_without_writes() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    assert_eq!(storage.root(), temporary.0.join(ROOT_DIRECTORY));
    assert_eq!(
        storage.installation_path(&target()).unwrap(),
        storage.root().join("installs/node/22.0.0")
    );
    assert_eq!(
        storage.temporary_path().unwrap(),
        storage.root().join("tmp")
    );
    assert_eq!(
        storage.current_link_path().unwrap(),
        storage.root().join("current/node")
    );
    assert_eq!(fs::read_dir(&temporary.0).unwrap().count(), 0);
    for version in ["0.0.0", "4294967295.4294967295.4294967295"] {
        let target: Target = format!("node@{version}").parse().unwrap();
        assert_eq!(
            storage.installation_path(&target).unwrap(),
            storage.root().join("installs/node").join(version)
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_roots_accept_drive_and_unc_but_reject_drive_relative_paths() {
    for source in [r"C:Users", r"\Users", r"/Users"] {
        assert!(Storage::from_source(Some(OsStr::new(source))).is_err());
    }
    for source in [
        r"C:\Users\Jason",
        r"\\server\share\Jason",
        r"C:\Users\ Jason ",
    ] {
        assert_eq!(
            Storage::from_source(Some(OsStr::new(source)))
                .unwrap()
                .root(),
            Path::new(source).join("verslot")
        );
    }
}

#[cfg(unix)]
#[test]
fn unix_root_preserves_native_bytes_and_whitespace() {
    use std::os::unix::ffi::OsStrExt;

    let source = OsStr::from_bytes(b"/home/ user\xff ");
    assert_eq!(
        Storage::from_source(Some(source)).unwrap().root(),
        Path::new(source).join(".verslot")
    );
}

#[test]
fn linked_storage_root_is_the_boundary() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let real_root = temporary.0.join("real-root");
    fs::create_dir(&real_root).unwrap();
    directory_link(&real_root, storage.root());
    assert!(storage.installation_path(&target()).is_ok());
    assert!(storage.temporary_path().is_ok());
    assert!(storage.current_link_path().is_ok());
}

#[test]
fn descendants_cannot_escape_through_directory_links() {
    for relative in [
        "installs",
        "installs/node",
        "installs/node/22.0.0",
        "tmp",
        "current",
    ] {
        let temporary = TestDirectory::new();
        let storage = temporary.storage();
        // A string-prefix check would incorrectly accept this sibling.
        let outside = temporary.0.join(format!("{ROOT_DIRECTORY}-outside"));
        fs::create_dir(&outside).unwrap();
        let link = storage.root().join(relative);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        directory_link(&outside, &link);
        let result = match relative {
            "tmp" => storage.temporary_path(),
            "current" => storage.current_link_path(),
            _ => storage.installation_path(&target()),
        };
        assert!(result.is_err(), "{relative}");
    }
}

#[test]
fn missing_descendants_do_not_hide_broken_links_or_files() {
    for broken_link in [false, true] {
        let temporary = TestDirectory::new();
        let storage = temporary.storage();
        fs::create_dir(storage.root()).unwrap();
        let ancestor = storage.root().join("installs");
        if broken_link {
            let destination = temporary.0.join("deleted");
            fs::create_dir(&destination).unwrap();
            directory_link(&destination, &ancestor);
            fs::remove_dir(destination).unwrap();
        } else {
            fs::write(&ancestor, b"not a directory").unwrap();
        }
        assert!(storage.installation_path(&target()).is_err());
    }
}

#[test]
fn links_within_the_storage_boundary_are_allowed() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let internal = storage.root().join("internal");
    fs::create_dir_all(&internal).unwrap();
    directory_link(&internal, &storage.root().join("installs"));
    assert!(storage.installation_path(&target()).is_ok());
}

#[test]
fn missing_roots_check_their_nearest_existing_ancestor() {
    let temporary = TestDirectory::new();
    let ancestor = temporary.0.join("ancestor");
    fs::write(&ancestor, b"not a directory").unwrap();
    let source = ancestor.join("missing");
    let storage = Storage::from_source(Some(source.as_os_str())).unwrap();
    assert!(storage.installation_path(&target()).is_err());
    assert!(storage.read_current().is_err());
}

#[test]
fn existing_invalid_intermediate_directories_cannot_be_hidden_by_later_links() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let outside = temporary.0.join("outside");
    let inside = storage.root().join("inside");
    fs::create_dir(&outside).unwrap();
    fs::create_dir_all(&inside).unwrap();
    directory_link(&inside, &outside.join("node"));
    directory_link(&outside, &storage.root().join("installs"));
    assert!(storage.installation_path(&target()).is_err());
}

#[test]
fn absent_current_state_is_read_without_creating_directories() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    assert_eq!(storage.read_current().unwrap(), None);
    assert_eq!(fs::read_dir(&temporary.0).unwrap().count(), 0);
    fs::create_dir(storage.root()).unwrap();
    assert_eq!(storage.read_current().unwrap(), None);
    assert_eq!(fs::read_dir(storage.root()).unwrap().count(), 0);
    fs::create_dir(storage.root().join("current")).unwrap();
    assert_eq!(storage.read_current().unwrap(), None);
    assert_eq!(
        fs::read_dir(storage.root().join("current"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn current_link_returns_the_resolved_version_without_checking_install_contents() {
    for version in ["0.0.0", "22.0.0", "4294967295.4294967295.4294967295"] {
        let temporary = TestDirectory::new();
        let storage = temporary.storage();
        let target: Target = format!("node@{version}").parse().unwrap();
        let installation = storage.installation_path(&target).unwrap();
        let link = storage.current_link_path().unwrap();
        fs::create_dir_all(&installation).unwrap();
        let sentinel = installation.join("existing-content");
        fs::write(&sentinel, b"unchanged").unwrap();
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        directory_link(&installation, &link);
        assert_eq!(storage.read_current().unwrap(), Some(target.version));
        assert_eq!(fs::read(sentinel).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(installation).unwrap().count(), 1);
    }
}

#[test]
fn ordinary_current_entries_are_errors() {
    for directory in [false, true] {
        let temporary = TestDirectory::new();
        let storage = temporary.storage();
        let link = storage.current_link_path().unwrap();
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        if directory {
            fs::create_dir(&link).unwrap();
        } else {
            fs::write(&link, b"22.0.0").unwrap();
        }
        assert!(storage.read_current().is_err());
    }
}

#[test]
fn dangling_current_link_is_an_error_not_no_selection() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let installation = storage.installation_path(&target()).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(&installation).unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&installation, &link);
    fs::remove_dir(installation).unwrap();
    assert!(storage.read_current().is_err());
    assert!(fs::symlink_metadata(link).is_ok());
}

#[test]
fn current_targets_must_be_direct_canonical_version_directories() {
    for relative in [
        "outside/22.0.0",
        "installs/node-other/22.0.0",
        "installs/node/nested/22.0.0",
        "installs/node/22",
        "installs/node/022.0.0",
        "installs/node/22.0.0-beta",
        "installs/node/4294967296.0.0",
    ] {
        let temporary = TestDirectory::new();
        let storage = temporary.storage();
        let destination = storage.root().join(relative);
        fs::create_dir_all(&destination).unwrap();
        fs::create_dir_all(storage.root().join("installs/node")).unwrap();
        let link = storage.current_link_path().unwrap();
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        directory_link(&destination, &link);
        assert!(storage.read_current().is_err(), "{relative}");
    }
}

#[test]
fn current_version_is_derived_from_the_resolved_name() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let installation = storage.installation_path(&target()).unwrap();
    fs::create_dir_all(&installation).unwrap();
    let alias = storage.root().join("installs/node/not-a-version");
    directory_link(&installation, &alias);
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&alias, &link);
    assert_eq!(storage.read_current().unwrap(), Some(target().version));
}

#[test]
fn invalid_current_ancestors_do_not_report_no_selection() {
    for relative in ["", "current"] {
        for broken_link in [false, true] {
            let temporary = TestDirectory::new();
            let storage = temporary.storage();
            let ancestor = if relative.is_empty() {
                storage.root().to_path_buf()
            } else {
                storage.root().join(relative)
            };
            fs::create_dir_all(ancestor.parent().unwrap()).unwrap();
            if broken_link {
                let destination = temporary.0.join("deleted");
                fs::create_dir(&destination).unwrap();
                directory_link(&destination, &ancestor);
                fs::remove_dir(destination).unwrap();
            } else {
                fs::write(ancestor, b"not a directory").unwrap();
            }
            assert!(storage.read_current().is_err());
        }
    }
}

#[test]
fn current_directory_cannot_escape_even_when_the_entry_is_missing() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let outside = temporary.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::create_dir(storage.root()).unwrap();
    directory_link(&outside, &storage.root().join("current"));
    assert!(storage.read_current().is_err());
}

#[test]
fn current_links_to_external_installations_are_rejected() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let outside = temporary.0.join("outside/22.0.0");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(storage.root().join("installs/node")).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&outside, &link);
    assert!(storage.read_current().is_err());
}

#[test]
fn current_reads_use_the_canonical_storage_root() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let real_root = temporary.0.join("real-root");
    fs::create_dir(&real_root).unwrap();
    directory_link(&real_root, storage.root());
    let installation = storage.installation_path(&target()).unwrap();
    fs::create_dir_all(&installation).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&installation, &link);
    assert_eq!(storage.read_current().unwrap(), Some(target().version));
}

#[test]
fn current_reads_reject_an_escaped_installation_ancestor() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let outside = temporary.0.join("outside");
    let installation = outside.join("node/22.0.0");
    fs::create_dir_all(&installation).unwrap();
    fs::create_dir(storage.root()).unwrap();
    directory_link(&outside, &storage.root().join("installs"));
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&installation, &link);
    assert!(storage.read_current().is_err());
}

#[cfg(unix)]
#[test]
fn relative_current_links_are_resolved_against_the_link_parent() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    fs::create_dir_all(storage.installation_path(&target()).unwrap()).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(Path::new("../installs/node/22.0.0"), &link);
    assert_eq!(storage.read_current().unwrap(), Some(target().version));
}

#[test]
fn current_links_to_files_are_rejected() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let installation = storage.installation_path(&target()).unwrap();
    fs::create_dir_all(&installation).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&installation, &link);
    fs::remove_dir(&installation).unwrap();
    fs::write(&installation, b"not a directory").unwrap();
    assert!(storage.read_current().is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_current_version_names_are_rejected() {
    use std::os::unix::ffi::OsStrExt;

    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let installation = storage
        .root()
        .join("installs/node")
        .join(OsStr::from_bytes(b"22.0.\xff"));
    fs::create_dir_all(&installation).unwrap();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&installation, &link);
    assert!(storage.read_current().is_err());
}

#[test]
fn io_errors_are_not_treated_as_missing_state() {
    let temporary = TestDirectory::new();
    let storage = Storage {
        root: temporary.0.join("invalid\0name"),
    };
    assert!(storage.temporary_path().is_err());
    assert!(storage.read_current().is_err());
}

#[cfg(unix)]
#[test]
fn permission_failures_are_not_treated_as_missing_state() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let current = storage.root().join("current");
    fs::create_dir_all(&current).unwrap();
    let original = fs::metadata(&current).unwrap().permissions();
    fs::set_permissions(&current, fs::Permissions::from_mode(0o0)).unwrap();
    let access = fs::read_dir(&current);
    let result = storage.read_current();
    fs::set_permissions(&current, original).unwrap();
    match access {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            assert_eq!(
                result.unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
        Ok(_) => eprintln!("permission-denial case unavailable: process can bypass mode bits"),
        Err(error) => panic!("unexpected access error: {error}"),
    }
}

#[test]
fn link_loops_are_errors() {
    let temporary = TestDirectory::new();
    let storage = temporary.storage();
    let link = storage.current_link_path().unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&link, &link);
    assert!(storage.read_current().is_err());
    directory_link(&storage.root().join("tmp"), &storage.root().join("tmp"));
    assert!(storage.temporary_path().is_err());
}
