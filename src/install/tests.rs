use super::*;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "verslot-install-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn target(&self) -> Target {
        "node@22.0.0".parse().unwrap()
    }

    fn destination(&self) -> PathBuf {
        self.0.join("installs/node/22.0.0")
    }

    fn install(&self) -> Result<String, String> {
        install_at(
            &self.0,
            &self.target(),
            |_, operation, staging| {
                fs::write(operation.join("archive"), b"offline fixture").unwrap();
                complete_staging(staging, &self.target());
                Ok(())
            },
            remove_operation,
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        if self.0.parent() == Some(temporary.as_path())
            && fs::canonicalize(&self.0).ok().as_ref() == Some(&self.0)
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn complete_staging(directory: &Path, target: &Target) {
    fs::create_dir(directory).unwrap();
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
}

#[test]
fn commits_complete_installation_and_duplicate_skips_preparation() {
    let fixture = Fixture::new();
    let orphan = fixture.0.join("tmp/orphan");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("sentinel"), b"preserved").unwrap();
    assert_eq!(fixture.install().unwrap(), "installed node@22.0.0");
    validate_installation(&fixture.0, &fixture.target()).unwrap();
    assert_eq!(
        install_at(
            &fixture.0,
            &fixture.target(),
            |_, _, _| panic!("duplicate must not download"),
            |_, _| panic!("duplicate has no operation")
        )
        .unwrap(),
        "already installed node@22.0.0"
    );
    assert_eq!(fs::read(orphan.join("sentinel")).unwrap(), b"preserved");
    assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 1);
    assert!(!fixture.0.join("current").exists());
}

#[test]
fn existing_incomplete_destination_is_preserved_without_download() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.destination()).unwrap();
    fs::write(fixture.destination().join("sentinel"), b"preserved").unwrap();
    assert!(
        install_at(
            &fixture.0,
            &fixture.target(),
            |_, _, _| panic!("must not download"),
            remove_operation
        )
        .unwrap_err()
        .contains("existing destination")
    );
    assert_eq!(
        fs::read(fixture.destination().join("sentinel")).unwrap(),
        b"preserved"
    );
    assert!(!fixture.0.join("tmp").exists());
}

#[test]
fn preparation_and_validation_failures_clean_only_the_operation() {
    for failure in [
        "download failed",
        "checksum mismatch",
        "extraction failed",
        "invalid staging",
    ] {
        let fixture = Fixture::new();
        let error = install_at(
            &fixture.0,
            &fixture.target(),
            |_, operation, staging| {
                fs::write(operation.join("archive"), b"partial").unwrap();
                fs::create_dir(staging).unwrap();
                fs::write(staging.join("partial"), b"partial").unwrap();
                if failure == "invalid staging" {
                    Ok(())
                } else {
                    Err(failure.to_owned())
                }
            },
            remove_operation,
        )
        .unwrap_err();
        assert!(error.contains(if failure == "invalid staging" {
            "validate staging"
        } else {
            failure
        }));
        assert!(!fixture.destination().exists());
        assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 0);
    }
}

#[test]
fn destination_appearing_before_commit_is_preserved() {
    let fixture = Fixture::new();
    let error = install_at(
        &fixture.0,
        &fixture.target(),
        |_, _, staging| {
            complete_staging(staging, &fixture.target());
            fs::create_dir(fixture.destination()).unwrap();
            fs::write(fixture.destination().join("sentinel"), b"preserved").unwrap();
            Ok(())
        },
        remove_operation,
    )
    .unwrap_err();
    assert!(error.contains("destination appeared"));
    assert_eq!(
        fs::read(fixture.destination().join("sentinel")).unwrap(),
        b"preserved"
    );
    assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 0);
}

#[test]
fn cleanup_failure_reports_leftover_and_preserves_committed_installation() {
    let fixture = Fixture::new();
    let error = install_at(
        &fixture.0,
        &fixture.target(),
        |_, _, staging| {
            complete_staging(staging, &fixture.target());
            Ok(())
        },
        |_, _| Err(io::Error::other("injected cleanup failure")),
    )
    .unwrap_err();
    assert!(error.contains("installation succeeded but cleanup failed at"));
    validate_installation(&fixture.0, &fixture.target()).unwrap();
    assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 1);
    assert_eq!(fixture.install().unwrap(), "already installed node@22.0.0");
}

#[test]
fn precommit_cleanup_failure_retains_primary_error_and_leftover_path() {
    let fixture = Fixture::new();
    let error = install_at(
        &fixture.0,
        &fixture.target(),
        |_, _, _| Err("download failed".to_owned()),
        |_, _| Err(io::Error::other("cleanup failed")),
    )
    .unwrap_err();
    assert!(error.contains("download failed; cleanup failed at"));
    assert!(error.contains(&fixture.0.join("tmp").display().to_string()));
    assert!(!fixture.destination().exists());
}

#[test]
fn lock_contents_are_preserved_and_contention_is_nonblocking() {
    let fixture = Fixture::new();
    let path = fixture.0.join(".mutation.lock");
    fs::write(&path, b"lock sentinel").unwrap();
    let lock = acquire_lock(&fixture.0).unwrap();
    assert!(fixture.install().unwrap_err().contains("mutation lock"));
    drop(lock);
    assert_eq!(fs::read(&path).unwrap(), b"lock sentinel");
    fixture.install().unwrap();
}

#[test]
fn lock_directory_and_linked_mutation_paths_are_rejected() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join(".mutation.lock")).unwrap();
    assert!(fixture.install().unwrap_err().contains("regular file"));
    fs::remove_dir(fixture.0.join(".mutation.lock")).unwrap();
    let outside = Fixture::new();
    fs::write(outside.0.join("sentinel"), b"preserved").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside.0, fixture.0.join("installs")).unwrap();
    #[cfg(windows)]
    junction::create(&outside.0, fixture.0.join("installs")).unwrap();
    assert!(fixture.install().is_err());
    assert_eq!(fs::read(outside.0.join("sentinel")).unwrap(), b"preserved");
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 1);
    #[cfg(windows)]
    junction::delete(fixture.0.join("installs")).unwrap();
}

#[test]
fn cleanup_refuses_shared_tmp_and_does_not_follow_links() {
    let fixture = Fixture::new();
    let operation = allocate_operation(&fixture.0).unwrap();
    assert!(remove_operation(&fixture.0, &fixture.0.join("tmp")).is_err());
    let outside = Fixture::new();
    fs::write(outside.0.join("sentinel"), b"preserved").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside.0, operation.join("link")).unwrap();
        remove_operation(&fixture.0, &operation).unwrap();
    }
    #[cfg(windows)]
    {
        junction::create(&outside.0, operation.join("link")).unwrap();
        assert!(remove_operation(&fixture.0, &operation).is_err());
        junction::delete(operation.join("link")).unwrap();
        remove_operation(&fixture.0, &operation).unwrap();
    }
    assert_eq!(fs::read(outside.0.join("sentinel")).unwrap(), b"preserved");
}

// Invoked in a child test process; normal test runs have no helper environment.
#[test]
fn process_lock_holder() {
    let Some(root) = std::env::var_os("VERSLOT_TEST_LOCK_ROOT") else {
        return;
    };
    let _lock = acquire_lock(Path::new(&root)).unwrap();
    println!("LOCK_READY");
    std::io::stdout().flush().unwrap();
    let mut byte = [0];
    let _ = std::io::stdin().read(&mut byte);
}

#[test]
fn process_lock_is_released_after_exit_or_termination() {
    use std::io::{BufRead, BufReader};
    for terminate in [false, true] {
        let fixture = Fixture::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "install::tests::process_lock_holder",
                "--nocapture",
            ])
            .env("VERSLOT_TEST_LOCK_ROOT", &fixture.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "helper exited before locking"
            );
            if line.contains("LOCK_READY") {
                break;
            }
            line.clear();
        }
        assert!(acquire_lock(&fixture.0).is_err());
        if terminate {
            child.kill().unwrap();
        } else {
            drop(child.stdin.take());
        }
        child.wait().unwrap();
        let _lock = acquire_lock(&fixture.0).unwrap();
        assert!(fixture.0.join(".mutation.lock").is_file());
    }
}

#[test]
fn lock_file_link_is_rejected() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let destination = outside.0.join("lock");
    fs::write(&destination, b"preserved").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&destination, fixture.0.join(".mutation.lock")).unwrap();
    #[cfg(windows)]
    {
        // A junction needs a directory and requires no symlink privilege.
        junction::create(&outside.0, fixture.0.join(".mutation.lock")).unwrap();
    }
    assert!(acquire_lock(&fixture.0).is_err());
    assert_eq!(fs::read(destination).unwrap(), b"preserved");
    #[cfg(windows)]
    junction::delete(fixture.0.join(".mutation.lock")).unwrap();
}

#[test]
fn linked_storage_root_is_supported_and_linked_destination_is_rejected() {
    let fixture = Fixture::new();
    let alias = fixture.0.join("alias");
    let storage = fixture.0.join("storage");
    fs::create_dir(&storage).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&storage, &alias).unwrap();
    #[cfg(windows)]
    junction::create(&storage, &alias).unwrap();
    install_at(
        &alias,
        &fixture.target(),
        |_, _, staging| {
            complete_staging(staging, &fixture.target());
            Ok(())
        },
        remove_operation,
    )
    .unwrap();
    validate_installation(&storage, &fixture.target()).unwrap();
    let other = storage.join("installs/node/24.0.0");
    #[cfg(unix)]
    std::os::unix::fs::symlink(storage.join("installs/node/22.0.0"), &other).unwrap();
    #[cfg(windows)]
    junction::create(storage.join("installs/node/22.0.0"), &other).unwrap();
    let target = "node@24.0.0".parse().unwrap();
    assert!(
        install_at(
            &alias,
            &target,
            |_, _, _| panic!("must preserve linked destination"),
            remove_operation
        )
        .is_err()
    );
    #[cfg(windows)]
    {
        junction::delete(&other).unwrap();
        junction::delete(&alias).unwrap();
    }
}

#[cfg(windows)]
#[test]
fn windows_locked_payload_causes_commit_failure_without_final_installation() {
    use std::cell::RefCell;
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let held = RefCell::new(None);
    let error = install_at(
        &fixture.0,
        &fixture.target(),
        |_, _, staging| {
            complete_staging(staging, &fixture.target());
            fs::write(staging.join("locked-payload"), b"fixture").unwrap();
            *held.borrow_mut() = Some(
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(staging.join("locked-payload"))
                    .unwrap(),
            );
            Ok(())
        },
        |root, operation| {
            drop(held.borrow_mut().take());
            remove_operation(root, operation)
        },
    )
    .unwrap_err();
    assert!(error.contains("commit installation"));
    assert!(!fixture.destination().exists());
    assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn rename_permission_failure_leaves_no_final_installation() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let parent = fixture.0.join("installs/node");
    let result = install_at(
        &fixture.0,
        &fixture.target(),
        |_, _, staging| {
            complete_staging(staging, &fixture.target());
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
            Ok(())
        },
        |root, operation| {
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
            remove_operation(root, operation)
        },
    );
    // Privileged runners can rename despite mode bits; do not claim failure coverage there.
    if let Err(error) = result {
        assert!(error.contains("commit installation"));
        assert!(!fixture.destination().exists());
    }
    assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 0);
}

fn archive_fixture(target: &Target) -> Vec<u8> {
    use std::io::Cursor;
    let distribution = NodeDistribution::for_current_build(target.version).unwrap();
    let archive_root = distribution
        .archive_filename
        .trim_end_matches(".zip")
        .trim_end_matches(".tar.gz");
    #[cfg(windows)]
    {
        use zip::write::SimpleFileOptions;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in ["node.exe", "node_modules/npm/bin/npm-cli.js"] {
            writer
                .start_file(
                    format!("{archive_root}/{name}"),
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(b"offline fixture").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[cfg(unix)]
    {
        let mut writer = tar::Builder::new(Vec::new());
        for name in ["bin/node", "lib/node_modules/npm/bin/npm-cli.js"] {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o755);
            header.set_size(15);
            header.set_cksum();
            writer
                .append_data(
                    &mut header,
                    format!("{archive_root}/{name}"),
                    Cursor::new(b"offline fixture"),
                )
                .unwrap();
        }
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&writer.into_inner().unwrap()).unwrap();
        gzip.finish().unwrap()
    }
}

#[test]
fn real_extraction_commits_bundled_payload_and_corruption_cleans_operation() {
    use sha2::{Digest, Sha256};
    for corrupt in [false, true] {
        let fixture = Fixture::new();
        let bytes = if corrupt {
            b"corrupt archive".to_vec()
        } else {
            archive_fixture(&fixture.target())
        };
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let result = install_at(
            &fixture.0,
            &fixture.target(),
            |root, operation, staging| {
                let archive = operation.join("archive");
                fs::write(&archive, &bytes).unwrap();
                extract_verified_archive(root, &archive, staging, fixture.target().version, &digest)
                    .map_err(|error| error.to_string())
            },
            remove_operation,
        );
        if corrupt {
            assert!(result.is_err());
            assert!(!fixture.destination().exists());
        } else {
            assert_eq!(result.unwrap(), "installed node@22.0.0");
            validate_installation(&fixture.0, &fixture.target()).unwrap();
            let npm = fixture.destination().join(if cfg!(windows) {
                "node_modules/npm/bin/npm-cli.js"
            } else {
                "lib/node_modules/npm/bin/npm-cli.js"
            });
            assert_eq!(fs::read(npm).unwrap(), b"offline fixture");
        }
        assert_eq!(fs::read_dir(fixture.0.join("tmp")).unwrap().count(), 0);
    }
}

#[test]
fn operation_allocation_is_exclusive_and_existing_operations_are_preserved() {
    let fixture = Fixture::new();
    let first = allocate_operation(&fixture.0).unwrap();
    fs::write(first.join("sentinel"), b"preserved").unwrap();
    let second = allocate_operation(&fixture.0).unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(first.join("sentinel")).unwrap(), b"preserved");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&second).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    remove_operation(&fixture.0, &second).unwrap();
    assert!(first.exists());
}

#[test]
fn malformed_receipts_and_non_directory_destinations_are_preserved() {
    for directory in [false, true] {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.destination().parent().unwrap()).unwrap();
        let sentinel = if directory {
            complete_staging(&fixture.destination(), &fixture.target());
            fixture.destination().join(".verslot-install")
        } else {
            fixture.destination()
        };
        fs::write(&sentinel, b"malformed existing data").unwrap();
        assert!(
            install_at(
                &fixture.0,
                &fixture.target(),
                |_, _, _| panic!("must not download"),
                remove_operation
            )
            .is_err()
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"malformed existing data");
        assert!(!fixture.0.join("tmp").exists());
    }
}

// Runs only in an isolated child process so environment changes cannot affect other tests.
#[test]
fn offline_workflow_child() {
    let Some(source) = std::env::var_os("VERSLOT_TEST_WORKFLOW_ROOT") else {
        return;
    };
    let storage = Storage::from_source(Some(&source)).unwrap();
    assert_eq!(Storage::from_env().unwrap().root(), storage.root());
    let target: Target = "node@22.0.0".parse().unwrap();
    assert!(crate::inventory::list_installed().unwrap().is_empty());
    assert!(!storage.root().exists());
    fs::create_dir(storage.root()).unwrap();
    let root = fs::canonicalize(storage.root()).unwrap();
    let orphan = root.join("tmp/orphan");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("sentinel"), b"preserved").unwrap();

    for corrupt in [true, false] {
        use sha2::{Digest, Sha256};
        let bytes = if corrupt {
            b"corrupt archive".to_vec()
        } else {
            archive_fixture(&target)
        };
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let result = install_at(
            &root,
            &target,
            |root, operation, staging| {
                let archive = operation.join("archive");
                fs::write(&archive, &bytes).unwrap();
                extract_verified_archive(root, &archive, staging, target.version, &digest)
                    .map_err(|error| error.to_string())
            },
            remove_operation,
        );
        if corrupt {
            assert!(result.is_err());
            assert!(crate::inventory::list_installed().unwrap().is_empty());
        } else {
            assert_eq!(result.unwrap(), "installed node@22.0.0");
            assert_eq!(crate::inventory::list_installed().unwrap(), vec![target]);
        }
        assert_eq!(fs::read_dir(root.join("tmp")).unwrap().count(), 1);
    }
    let installation = root.join("installs/node/22.0.0");
    let npm = installation.join(if cfg!(windows) {
        "node_modules/npm/bin/npm-cli.js"
    } else {
        "lib/node_modules/npm/bin/npm-cli.js"
    });
    assert_eq!(fs::read(&npm).unwrap(), b"offline fixture");
    let receipt = fs::read(installation.join(".verslot-install")).unwrap();
    assert_eq!(install(&target).unwrap(), "already installed node@22.0.0");
    assert_eq!(
        fs::read(installation.join(".verslot-install")).unwrap(),
        receipt
    );
    assert_eq!(fs::read(npm).unwrap(), b"offline fixture");
    assert_eq!(
        crate::uninstall::uninstall(&target).unwrap(),
        "uninstalled node@22.0.0"
    );
    assert!(crate::inventory::list_installed().unwrap().is_empty());
    assert!(!installation.exists());
    assert!(
        crate::uninstall::uninstall(&target)
            .unwrap_err()
            .contains("not installed")
    );
    assert_eq!(fs::read(orphan.join("sentinel")).unwrap(), b"preserved");
    assert_eq!(fs::read_dir(root.join("tmp")).unwrap().count(), 1);
    assert!(!root.join("current").exists());
}

#[test]
fn offline_install_list_duplicate_uninstall_workflow_is_isolated() {
    let fixture = Fixture::new();
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "install::tests::offline_workflow_child",
            "--nocapture",
        ])
        .env("VERSLOT_TEST_WORKFLOW_ROOT", &fixture.0)
        .env("HOME", &fixture.0)
        .env("LOCALAPPDATA", &fixture.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "workflow child failed: stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
