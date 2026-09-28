use super::*;
use crate::distribution::NodeDistribution;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct Fixture {
    base: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot Unix 切换 {}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let storage = Storage::from_source(Some(base.as_os_str())).unwrap();
        Self { base, storage }
    }

    fn target(version: &str) -> Target {
        format!("node@{version}").parse().unwrap()
    }

    fn install(&self, version: &str) -> PathBuf {
        let target = Self::target(version);
        let directory = self.storage.root().join("installs/node").join(version);
        fs::create_dir_all(directory.join("bin")).unwrap();
        fs::write(directory.join("bin/node"), version).unwrap();
        fs::set_permissions(
            directory.join("bin/node"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
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

    fn current(&self) -> PathBuf {
        self.storage.root().join("current/node")
    }

    fn sibling(&self, name: &str) -> PathBuf {
        self.storage.root().join("current").join(name)
    }

    fn selected(&self, expected: Option<&str>) {
        assert_eq!(
            self.storage.read_selected().unwrap(),
            expected.map(|version| Self::target(version).version)
        );
    }

    fn no_residue(&self) {
        for name in [".node-next", ".node-previous"] {
            assert_eq!(
                fs::symlink_metadata(self.sibling(name)).unwrap_err().kind(),
                io::ErrorKind::NotFound
            );
        }
    }

    fn unchanged_installations(&self) {
        for version in ["22.0.0", "24.0.0"] {
            let target = Self::target(version);
            validate_installation(self.storage.root(), &target).unwrap();
            assert_eq!(
                fs::read(
                    self.storage
                        .root()
                        .join("installs/node")
                        .join(version)
                        .join("bin/node")
                )
                .unwrap(),
                version.as_bytes()
            );
        }
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
fn first_selection_switch_and_same_version_preserve_installations() {
    let fixture = Fixture::new();
    let first = fixture.install("22.0.0");
    let second = fixture.install("24.0.0");
    assert!(fixture.storage.select(&Fixture::target("22.0.0")).unwrap());
    assert_eq!(fs::read_link(fixture.current()).unwrap(), first);
    fixture.selected(Some("22.0.0"));
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    assert_eq!(fs::read_link(fixture.current()).unwrap(), second);
    let inode = fs::symlink_metadata(fixture.current()).unwrap().ino();
    assert!(!fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    assert_eq!(
        fs::symlink_metadata(fixture.current()).unwrap().ino(),
        inode
    );
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn relative_selection_is_untouched_on_noop_and_supported_on_switch() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    fixture.install("24.0.0");
    fs::create_dir(fixture.storage.root().join("current")).unwrap();
    std::os::unix::fs::symlink("../installs/node/22.0.0", fixture.current()).unwrap();
    let inode = fs::symlink_metadata(fixture.current()).unwrap().ino();
    assert!(!fixture.storage.select(&Fixture::target("22.0.0")).unwrap());
    assert_eq!(
        fs::symlink_metadata(fixture.current()).unwrap().ino(),
        inode
    );
    assert_eq!(
        fs::read_link(fixture.current()).unwrap(),
        Path::new("../installs/node/22.0.0")
    );
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
}

#[test]
fn missing_or_incomplete_targets_and_invalid_old_state_fail_before_preparation() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture
            .storage
            .select(&Fixture::target("22.0.0"))
            .unwrap_err()
            .to_string(),
        "not installed: node@22.0.0"
    );
    assert!(!fixture.storage.root().exists());
    let first = fixture.install("22.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    assert_eq!(
        fixture
            .storage
            .select(&Fixture::target("24.0.0"))
            .unwrap_err()
            .to_string(),
        "not installed: node@24.0.0"
    );
    fixture.selected(Some("22.0.0"));
    let second = fixture.install("24.0.0");
    fs::remove_file(second.join(".verslot-install")).unwrap();
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).is_err());
    fixture.selected(Some("22.0.0"));
    fs::remove_file(first.join(".verslot-install")).unwrap();
    assert!(
        fixture
            .storage
            .select(&Fixture::target("26.0.0"))
            .unwrap_err()
            .to_string()
            .contains("current installation is not complete")
    );
    assert_eq!(fs::read_link(fixture.current()).unwrap(), first);
    fixture.no_residue();
}

#[test]
fn preparation_and_publish_failures_keep_the_old_link_and_clean_owned_links() {
    for failed_step in [
        SwitchStep::Candidate,
        SwitchStep::Backup,
        SwitchStep::Publish,
    ] {
        let fixture = Fixture::new();
        fixture.install("22.0.0");
        fixture.install("24.0.0");
        fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
        let inode = fs::symlink_metadata(fixture.current()).unwrap().ino();
        let error = fixture
            .storage
            .select_unix(&Fixture::target("24.0.0"), |step| {
                assert!(acquire_lock(fixture.storage.root()).is_err());
                if step == SwitchStep::Publish && failed_step == SwitchStep::Publish {
                    crate::storage::selection_tests::assert_other_processes_are_busy(&fixture.base);
                }
                if step == failed_step {
                    Err(io::Error::other(format!("injected {step:?}")))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("injected {failed_step:?}"))
        );
        assert_eq!(
            fs::symlink_metadata(fixture.current()).unwrap().ino(),
            inode
        );
        fixture.selected(Some("22.0.0"));
        fixture.no_residue();
        fixture.unchanged_installations();
    }
}

#[test]
fn native_exclusive_link_creation_preserves_conflicting_entries() {
    for (step, name) in [
        (SwitchStep::Candidate, ".node-next"),
        (SwitchStep::Backup, ".node-previous"),
    ] {
        let fixture = Fixture::new();
        fixture.install("22.0.0");
        fixture.install("24.0.0");
        fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
        let error = fixture
            .storage
            .select_unix(&Fixture::target("24.0.0"), |observed| {
                if observed == step {
                    fs::write(fixture.sibling(name), b"unexpected").unwrap();
                }
                Ok(())
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("create symbolic link"));
        assert_eq!(fs::read(fixture.sibling(name)).unwrap(), b"unexpected");
        assert_eq!(
            fixture.storage.read_current().unwrap(),
            Some(Fixture::target("22.0.0").version)
        );
        fixture.unchanged_installations();
    }
}

#[test]
fn readback_failure_restores_previous_selection_or_absence() {
    for old in [None, Some("22.0.0")] {
        let fixture = Fixture::new();
        fixture.install("22.0.0");
        fixture.install("24.0.0");
        if let Some(version) = old {
            fixture.storage.select(&Fixture::target(version)).unwrap();
        }
        let error = fixture
            .storage
            .select_unix(&Fixture::target("24.0.0"), |step| {
                if step == SwitchStep::ReadBack {
                    Err(io::Error::other("injected read-back failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("injected read-back failure"));
        assert!(error.contains("previous selection restored"));
        fixture.selected(old);
        fixture.no_residue();
        fixture.unchanged_installations();
    }
}

#[test]
fn actual_readback_validation_failure_rolls_back_without_repairing_installation() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    let second = fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::ReadBack {
                fs::remove_file(second.join(".verslot-install")).unwrap();
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("previous selection restored"));
    fixture.selected(Some("22.0.0"));
    fixture.no_residue();
    assert!(!second.join(".verslot-install").exists());
    assert_eq!(fs::read(second.join("bin/node")).unwrap(), b"24.0.0");
}

#[test]
fn rollback_failure_reports_observed_state_and_keeps_backup() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    let second = fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| match step {
            SwitchStep::ReadBack => Err(io::Error::other("injected read-back failure")),
            SwitchStep::Rollback => Err(io::Error::other("injected rollback failure")),
            _ => Ok(()),
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected read-back failure"));
    assert!(error.contains("injected rollback failure"));
    assert!(error.contains("current state: node@24.0.0"));
    assert!(error.contains(&fixture.sibling(".node-previous").display().to_string()));
    assert_eq!(fs::read_link(fixture.current()).unwrap(), second);
    assert!(
        fs::symlink_metadata(fixture.sibling(".node-previous"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(fixture.storage.read_selected().is_err());
    fixture.unchanged_installations();
}

#[test]
fn postcommit_cleanup_failure_keeps_new_selection_and_reports_residue() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::CleanupBackup {
                Err(io::Error::other("injected cleanup failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("switch completed but cleanup failed"));
    assert!(error.contains(&fixture.sibling(".node-previous").display().to_string()));
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some(Fixture::target("24.0.0").version)
    );
    assert!(fs::symlink_metadata(fixture.sibling(".node-previous")).is_ok());
    fixture.unchanged_installations();
}

#[test]
fn first_selection_rollback_failure_reports_retained_current_without_a_backup() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    fixture.install("24.0.0");
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| match step {
            SwitchStep::ReadBack => Err(io::Error::other("injected read-back failure")),
            SwitchStep::Rollback => Err(io::Error::other("injected rollback failure")),
            _ => Ok(()),
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected read-back failure"));
    assert!(error.contains("injected rollback failure"));
    assert!(error.contains("current state: node@24.0.0"));
    assert!(error.contains(&fixture.current().display().to_string()));
    assert!(!error.contains(".node-previous"));
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn preparation_cleanup_failure_reports_both_errors_and_retains_owned_candidate() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| match step {
            SwitchStep::Backup => Err(io::Error::other("injected backup failure")),
            SwitchStep::CleanupCandidate => Err(io::Error::other("injected cleanup failure")),
            _ => Ok(()),
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected backup failure"));
    assert!(error.contains("injected cleanup failure"));
    assert!(error.contains(&fixture.sibling(".node-next").display().to_string()));
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some(Fixture::target("22.0.0").version)
    );
    assert!(fs::symlink_metadata(fixture.sibling(".node-next")).is_ok());
    fixture.unchanged_installations();
}

#[test]
fn unexpected_current_replacement_is_preserved_during_publish_and_rollback() {
    for attack_step in [SwitchStep::Publish, SwitchStep::ReadBack] {
        let fixture = Fixture::new();
        fixture.install("22.0.0");
        fixture.install("24.0.0");
        fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
        let outside = fixture.base.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"preserved").unwrap();
        let error = fixture
            .storage
            .select_unix(&Fixture::target("24.0.0"), |step| {
                if step == attack_step {
                    let replacement = fixture.sibling("replacement");
                    std::os::unix::fs::symlink(&outside, &replacement).unwrap();
                    fs::rename(replacement, fixture.current()).unwrap();
                }
                Ok(())
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("link changed"));
        assert_eq!(fs::read_link(fixture.current()).unwrap(), outside);
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
        if attack_step == SwitchStep::ReadBack {
            assert!(error.contains("rollback failed"));
            assert!(error.contains("current state: unavailable"));
            assert!(fs::symlink_metadata(fixture.sibling(".node-previous")).is_ok());
        } else {
            fixture.no_residue();
        }
        fixture.unchanged_installations();
    }
}

#[test]
fn cleanup_rejects_replacement_even_when_it_has_the_same_destination() {
    let fixture = Fixture::new();
    let first = fixture.install("22.0.0");
    fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let error = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::CleanupBackup {
                let replacement = fixture.sibling("replacement");
                std::os::unix::fs::symlink(&first, &replacement).unwrap();
                fs::rename(replacement, fixture.sibling(".node-previous")).unwrap();
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("switch completed but cleanup failed"));
    assert!(error.contains("link changed"));
    assert_eq!(
        fs::read_link(fixture.sibling(".node-previous")).unwrap(),
        first
    );
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some(Fixture::target("24.0.0").version)
    );
    fixture.unchanged_installations();
}

#[test]
fn native_atomic_replacement_exposes_only_old_or_new_link() {
    let fixture = Fixture::new();
    let first = fixture.install("22.0.0");
    let second = fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let finished = AtomicBool::new(false);
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let observer = scope.spawn(|| {
            barrier.wait();
            let mut reads = 0;
            while !finished.load(Ordering::Acquire) || reads == 0 {
                let destination = fs::read_link(fixture.current()).unwrap();
                assert!(destination == first || destination == second);
                reads += 1;
            }
            reads
        });
        barrier.wait();
        let result = (0..40).try_for_each(|index| {
            fixture
                .storage
                .select(&Fixture::target(if index % 2 == 0 {
                    "24.0.0"
                } else {
                    "22.0.0"
                }))
                .map(|_| ())
        });
        finished.store(true, Ordering::Release);
        assert!(observer.join().unwrap() > 0);
        result.unwrap();
    });
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn native_permission_failure_preserves_old_selection_when_enforced() {
    let fixture = Fixture::new();
    let first = fixture.install("22.0.0");
    fixture.install("24.0.0");
    fixture.storage.select(&Fixture::target("22.0.0")).unwrap();
    let directory = fixture.storage.root().join("current");
    let result = fixture
        .storage
        .select_unix(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::Publish {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
            }
            Ok(())
        });
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    // Privileged runners may rename despite directory permission bits.
    if let Err(error) = result {
        assert!(error.to_string().contains("publish selection"));
        assert_eq!(fs::read_link(fixture.current()).unwrap(), first);
    }
    fixture.unchanged_installations();
}
