use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "verslot-setup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let source = root.join("source");
        fs::write(&source, b"old executable").unwrap();
        Self { root, source }
    }

    fn setup(&self) -> io::Result<PathBuf> {
        setup_at(&self.root, &self.source, |source, shim| {
            fs::hard_link(source, shim)
        })
    }

    fn finish(self) {
        assert_eq!(
            self.root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(self.root).unwrap();
    }
}

#[cfg(unix)]
fn file_identity(path: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).unwrap();
    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn file_identity(path: &Path) -> (u64, u64) {
    let file = File::open(path).unwrap();
    let information = winapi_util::file::information(&file).unwrap();
    (information.volume_serial_number(), information.file_index())
}

#[test]
fn setup_repeats_and_refreshes_the_complete_owned_generation() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    let executable = directory.join(executable_name("verslot"));
    let identity = file_identity(&executable);
    assert_ne!(identity, file_identity(&fixture.source));
    let modified = fs::metadata(directory.join(RECEIPT))
        .unwrap()
        .modified()
        .unwrap();
    fixture.setup().unwrap();
    assert_eq!(file_identity(&executable), identity);
    assert_eq!(
        fs::metadata(directory.join(RECEIPT))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    fs::write(&fixture.source, b"new executable").unwrap();
    fixture.setup().unwrap();
    let identity = file_identity(&executable);
    for name in COMMANDS {
        let shim = directory.join(executable_name(name));
        assert_eq!(fs::read(&shim).unwrap(), b"new executable");
        assert_eq!(file_identity(&shim), identity);
    }
    assert!(!fixture.root.join(".shims-previous").exists());
    fixture.finish();
}

#[test]
fn setup_migrates_legacy_copies_with_the_same_digest_and_commands() {
    let fixture = Fixture::new();
    let directory = fixture.root.join("shims");
    fs::create_dir(&directory).unwrap();
    for name in COMMANDS {
        fs::copy(&fixture.source, directory.join(executable_name(name))).unwrap();
    }
    fs::write(
        directory.join(RECEIPT),
        format!(
            "verslot-shims-v1\n{}\n{}\n",
            file_digest(&fixture.source).unwrap(),
            COMMANDS.map(executable_name).join("\n")
        ),
    )
    .unwrap();

    fixture.setup().unwrap();

    let identity = file_identity(&directory.join(executable_name("verslot")));
    for name in COMMANDS {
        let shim = directory.join(executable_name(name));
        assert_eq!(file_identity(&shim), identity);
    }
    fixture.finish();
}

#[test]
fn setup_copies_entries_when_hard_links_are_unavailable() {
    let fixture = Fixture::new();
    let directory = setup_at(&fixture.root, &fixture.source, |_, _| {
        #[cfg(windows)]
        let error = io::Error::from_raw_os_error(50);
        #[cfg(target_os = "linux")]
        let error = io::Error::from_raw_os_error(1);
        #[cfg(not(any(windows, target_os = "linux")))]
        let error = io::Error::from(io::ErrorKind::Unsupported);
        Err(error)
    })
    .unwrap();

    let identity = file_identity(&directory.join(executable_name("verslot")));
    for name in &COMMANDS[1..] {
        let shim = directory.join(executable_name(name));
        assert_ne!(file_identity(&shim), identity);
        assert_eq!(fs::read(shim).unwrap(), b"old executable");
    }
    fixture.finish();
}

#[test]
fn setup_preserves_the_previous_generation_after_an_unexpected_link_error() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    fs::write(&fixture.source, b"new executable").unwrap();

    let error = setup_at(&fixture.root, &fixture.source, |_, _| {
        Err(io::Error::from(io::ErrorKind::AlreadyExists))
    })
    .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(directory.join(executable_name("verslot"))).unwrap(),
        b"old executable"
    );
    assert!(!fixture.root.join(".shims-previous").exists());
    fixture.finish();
}

#[test]
fn setup_upgrades_the_previous_four_command_receipt_with_yarn_entries() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    for name in ["yarn", "yarnpkg"] {
        fs::remove_file(directory.join(executable_name(name))).unwrap();
    }
    let old_commands = ["verslot", "node", "npm", "npx"]
        .map(executable_name)
        .join("\n");
    fs::write(
        directory.join(RECEIPT),
        format!(
            "verslot-shims-v1\n{}\n{old_commands}\n",
            file_digest(&fixture.source).unwrap()
        ),
    )
    .unwrap();
    fixture.setup().unwrap();
    for name in COMMANDS {
        assert_eq!(
            fs::read(directory.join(executable_name(name))).unwrap(),
            b"old executable"
        );
    }
    fixture.finish();
}

#[test]
fn setup_preserves_unowned_and_modified_entries() {
    let fixture = Fixture::new();
    let directory = fixture.root.join("shims");
    fs::create_dir(&directory).unwrap();
    let node = directory.join(executable_name("node"));
    fs::write(&node, b"user file").unwrap();
    assert!(fixture.setup().is_err());
    assert_eq!(fs::read(&node).unwrap(), b"user file");
    fs::remove_file(&node).unwrap();
    fs::remove_dir(&directory).unwrap();
    fixture.setup().unwrap();
    fs::write(&node, b"modified user file").unwrap();
    fs::write(&fixture.source, b"new executable").unwrap();
    assert!(
        fixture
            .setup()
            .unwrap_err()
            .to_string()
            .contains("modified shim")
    );
    assert_eq!(fs::read(&node).unwrap(), b"modified user file");
    fixture.finish();
}

#[test]
fn setup_preserves_extra_files_and_rejects_invalid_receipts() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    fs::write(directory.join("notes"), b"user notes").unwrap();
    assert!(
        fixture
            .setup()
            .unwrap_err()
            .to_string()
            .contains("unknown entry")
    );
    fs::remove_file(directory.join("notes")).unwrap();
    fs::write(directory.join(RECEIPT), "invalid").unwrap();
    assert!(fixture.setup().is_err());
    assert_eq!(
        fs::read(directory.join(executable_name("node"))).unwrap(),
        b"old executable"
    );
    fixture.finish();
}

#[test]
fn setup_retries_cleanup_but_preserves_the_only_previous_generation() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    let backup = fixture.root.join(".shims-previous");
    fs::rename(&directory, &backup).unwrap();
    assert!(
        fixture
            .setup()
            .unwrap_err()
            .to_string()
            .contains("previous shims preserved")
    );
    assert!(backup.join(executable_name("verslot")).exists());
    fs::rename(&backup, &directory).unwrap();
    fs::create_dir(&backup).unwrap();
    for entry in fs::read_dir(&directory).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), backup.join(entry.file_name())).unwrap();
    }
    fs::remove_file(backup.join(executable_name("node"))).unwrap();
    fixture.setup().unwrap();
    assert!(!backup.exists());
    fixture.finish();
}

#[test]
fn setup_obeys_the_global_lock_without_reading_node_selection() {
    let fixture = Fixture::new();
    let lock = acquire_exclusive_lock(&fixture.root).unwrap();
    assert!(fixture.setup().unwrap_err().to_string().contains("busy"));
    drop(lock);
    fs::write(fixture.root.join("current"), b"broken selection state").unwrap();
    fixture.setup().unwrap();
    fixture.finish();
}

#[test]
fn failed_publication_restores_the_previous_generation() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    let backup = fixture.root.join(".shims-previous");
    let missing_candidate = fixture.root.join("missing-candidate");
    assert!(publish_shims(&fixture.root, &missing_candidate, &directory, &backup, true).is_err());
    assert_eq!(
        fs::read(directory.join(executable_name("node"))).unwrap(),
        b"old executable"
    );
    assert!(!backup.exists());
    fixture.finish();
}

#[test]
fn publication_preserves_an_unexpected_backup_directory() {
    let fixture = Fixture::new();
    let directory = fixture.setup().unwrap();
    let backup = fixture.root.join(".shims-previous");
    fs::create_dir(&backup).unwrap();
    assert!(
        publish_shims(
            &fixture.root,
            &fixture.root.join("candidate"),
            &directory,
            &backup,
            true
        )
        .is_err()
    );
    assert!(directory.join(RECEIPT).exists());
    assert!(backup.exists());
    fixture.finish();
}

#[cfg(unix)]
#[test]
fn setup_rejects_linked_directories_and_executables() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let external = fixture.root.join("external");
    fs::create_dir(&external).unwrap();
    symlink(&external, fixture.root.join("shims")).unwrap();
    assert!(fixture.setup().is_err());
    assert_eq!(fs::read_dir(&external).unwrap().count(), 0);
    fs::remove_file(fixture.root.join("shims")).unwrap();
    let directory = fixture.setup().unwrap();
    let node = directory.join(executable_name("node"));
    fs::remove_file(&node).unwrap();
    symlink(&fixture.source, &node).unwrap();
    assert!(fixture.setup().is_err());
    assert_eq!(fs::read(&fixture.source).unwrap(), b"old executable");
    fs::remove_file(node).unwrap();
    fixture.finish();
}

#[cfg(windows)]
#[test]
fn setup_rejects_junction_directories_without_following_them() {
    let fixture = Fixture::new();
    let external = fixture.root.join("external");
    fs::create_dir(&external).unwrap();
    junction::create(&external, fixture.root.join("shims")).unwrap();
    assert!(fixture.setup().is_err());
    assert_eq!(fs::read_dir(&external).unwrap().count(), 0);
    junction::delete(fixture.root.join("shims")).unwrap();
    fixture.finish();
}
