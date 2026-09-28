use super::*;
use crate::distribution::NodeDistribution;
use std::fs::File;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    base: PathBuf,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot Windows 切换 {}-{}",
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
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("node.exe"), version).unwrap();
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

    fn select_first(&self) {
        self.install("22.0.0");
        self.install("24.0.0");
        self.storage.select(&Self::target("22.0.0")).unwrap();
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
            validate_installation(self.storage.root(), &Self::target(version)).unwrap();
            assert_eq!(
                fs::read(
                    self.storage
                        .root()
                        .join("installs/node")
                        .join(version)
                        .join("node.exe")
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

// A real read handle enforces sharing restrictions on native rename and junction operations.
fn hold_junction(path: &Path, sharing: u32) -> File {
    fs::OpenOptions::new()
        .read(true)
        .share_mode(sharing)
        .custom_flags(0x0020_0000 | 0x0200_0000)
        .open(path)
        .unwrap()
}

#[test]
fn native_junction_rename_and_removal_preserve_target_and_free_entry_name() {
    let fixture = Fixture::new();
    let directory = fixture.install("22.0.0");
    fs::create_dir(fixture.storage.root().join("current")).unwrap();
    let next = fixture.sibling(".node-next");
    junction::create(&directory, &next).unwrap();
    let identity = WindowsJunction::inspect(&next).unwrap();
    fs::rename(&next, fixture.current()).unwrap();
    identity
        .verify_identity(fixture.storage.root(), &fixture.current())
        .unwrap();
    assert_eq!(fs::canonicalize(fixture.current()).unwrap(), directory);
    identity
        .remove(fixture.storage.root(), &fixture.current())
        .unwrap();
    assert_eq!(
        fs::symlink_metadata(fixture.current()).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    // The old name must be reusable immediately, after all identity handles close.
    junction::create(&directory, fixture.current()).unwrap();
    assert_eq!(fs::read(directory.join("node.exe")).unwrap(), b"22.0.0");
}

#[test]
fn first_selection_switch_and_noop_preserve_junction_identity_and_installations() {
    let fixture = Fixture::new();
    fixture.select_first();
    fixture.selected(Some("22.0.0"));
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    let identity = WindowsJunction::inspect(&fixture.current()).unwrap();
    assert!(!fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    identity
        .verify_identity(fixture.storage.root(), &fixture.current())
        .unwrap();
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn native_removal_of_known_dangling_junction_leaves_no_directory_residue() {
    let fixture = Fixture::new();
    let target = fixture.install("22.0.0");
    fs::create_dir(fixture.storage.root().join("current")).unwrap();
    junction::create(&target, fixture.current()).unwrap();
    let identity = WindowsJunction::inspect(&fixture.current()).unwrap();
    fs::remove_file(target.join("node.exe")).unwrap();
    fs::remove_file(target.join(".verslot-install")).unwrap();
    fs::remove_dir(&target).unwrap();
    identity
        .remove(fixture.storage.root(), &fixture.current())
        .unwrap();
    assert_eq!(
        fs::symlink_metadata(fixture.current()).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn native_candidate_and_backup_collisions_preserve_unexpected_directories() {
    for (collision, name) in [
        (SwitchStep::Candidate, ".node-next"),
        (SwitchStep::Backup, ".node-previous"),
    ] {
        let fixture = Fixture::new();
        fixture.select_first();
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
                if step == collision {
                    fs::create_dir(fixture.sibling(name)).unwrap();
                    fs::write(fixture.sibling(name).join("sentinel"), b"preserved").unwrap();
                }
                Ok(())
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains(&fixture.sibling(name).display().to_string()));
        assert_eq!(
            fs::read(fixture.sibling(name).join("sentinel")).unwrap(),
            b"preserved"
        );
        assert_eq!(
            fixture.storage.read_current().unwrap(),
            Some(Fixture::target("22.0.0").version)
        );
        fixture.unchanged_installations();
    }
}

#[test]
fn missing_incomplete_and_invalid_old_selections_fail_before_candidate_creation() {
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
    fixture.select_first();
    assert_eq!(
        fixture
            .storage
            .select(&Fixture::target("26.0.0"))
            .unwrap_err()
            .to_string(),
        "not installed: node@26.0.0"
    );
    let second = fixture.storage.root().join("installs/node/24.0.0");
    fs::remove_file(second.join(".verslot-install")).unwrap();
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).is_err());
    fixture.selected(Some("22.0.0"));
    fs::remove_file(
        fixture
            .storage
            .root()
            .join("installs/node/22.0.0/.verslot-install"),
    )
    .unwrap();
    assert!(
        fixture
            .storage
            .select(&Fixture::target("26.0.0"))
            .unwrap_err()
            .to_string()
            .contains("current installation is not complete")
    );
    fixture.no_residue();
}

#[test]
fn candidate_backup_and_publish_failures_preserve_or_restore_old_junction() {
    for failure in [
        SwitchStep::Candidate,
        SwitchStep::Backup,
        SwitchStep::Publish,
    ] {
        let fixture = Fixture::new();
        fixture.select_first();
        let old = WindowsJunction::inspect(&fixture.current()).unwrap();
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
                assert!(acquire_lock(fixture.storage.root()).is_err());
                if step == failure {
                    Err(io::Error::other(format!("injected {step:?}")))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("injected {failure:?}")));
        old.verify_identity(fixture.storage.root(), &fixture.current())
            .unwrap();
        fixture.selected(Some("22.0.0"));
        fixture.no_residue();
        fixture.unchanged_installations();
    }
}

#[test]
fn publish_gap_is_locked_and_failed_publish_restores_previous_junction() {
    let fixture = Fixture::new();
    fixture.select_first();
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::Publish {
                crate::storage::selection_tests::assert_other_processes_are_busy(&fixture.base);
                assert_eq!(
                    fs::symlink_metadata(fixture.current()).unwrap_err().kind(),
                    io::ErrorKind::NotFound
                );
                assert!(junction::get_target(fixture.sibling(".node-previous")).is_ok());
                assert!(
                    fixture
                        .storage
                        .read_selected()
                        .unwrap_err()
                        .to_string()
                        .contains("storage is busy")
                );
                Err(io::Error::other("injected publication failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("previous selection restored"));
    fixture.selected(Some("22.0.0"));
    fixture.no_residue();
}

#[test]
fn readback_failure_restores_old_selection_or_absence() {
    for old in [None, Some("22.0.0")] {
        let fixture = Fixture::new();
        fixture.install("22.0.0");
        fixture.install("24.0.0");
        if let Some(version) = old {
            fixture.storage.select(&Fixture::target(version)).unwrap();
        }
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
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
fn actual_incomplete_readback_rolls_back_without_repairing_installation() {
    let fixture = Fixture::new();
    fixture.select_first();
    let receipt = fixture
        .storage
        .root()
        .join("installs/node/24.0.0/.verslot-install");
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::ReadBack {
                fs::remove_file(&receipt).unwrap();
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("previous selection restored"));
    fixture.selected(Some("22.0.0"));
    fixture.no_residue();
    assert!(!receipt.exists());
    assert_eq!(
        fs::read(receipt.parent().unwrap().join("node.exe")).unwrap(),
        b"24.0.0"
    );
}

#[test]
fn rollback_and_restore_failures_keep_backup_and_report_observed_state() {
    for failed_step in [SwitchStep::Rollback, SwitchStep::Restore] {
        let fixture = Fixture::new();
        fixture.select_first();
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
                if step == SwitchStep::ReadBack || step == failed_step {
                    Err(io::Error::other(format!("injected {step:?}")))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("injected ReadBack"));
        assert!(error.contains(&format!("injected {failed_step:?}")));
        let observed = if failed_step == SwitchStep::Rollback {
            "node@24.0.0"
        } else {
            "no selection"
        };
        assert!(error.contains(&format!("current state: {observed}")));
        assert!(error.contains(&fixture.sibling(".node-previous").display().to_string()));
        assert!(junction::get_target(fixture.sibling(".node-previous")).is_ok());
        assert!(
            fixture
                .storage
                .read_selected()
                .unwrap_err()
                .to_string()
                .contains("unfinished switch")
        );
        fixture.unchanged_installations();
    }
}

#[test]
fn first_selection_rollback_failure_reports_the_current_path_without_a_backup() {
    let fixture = Fixture::new();
    fixture.install("22.0.0");
    fixture.install("24.0.0");
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::ReadBack || step == SwitchStep::Rollback {
                Err(io::Error::other(format!("injected {step:?}")))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected ReadBack"));
    assert!(error.contains("injected Rollback"));
    assert!(error.contains("current state: node@24.0.0"));
    assert!(error.contains(&fixture.current().display().to_string()));
    assert!(!error.contains(".node-previous"));
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn cleanup_failures_report_residue_and_preserve_committed_or_old_selection() {
    for committed in [false, true] {
        let fixture = Fixture::new();
        fixture.select_first();
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| match step {
                SwitchStep::Backup if !committed => {
                    Err(io::Error::other("injected backup failure"))
                }
                SwitchStep::CleanupCandidate if !committed => {
                    Err(io::Error::other("injected candidate cleanup failure"))
                }
                SwitchStep::CleanupBackup if committed => {
                    Err(io::Error::other("injected backup cleanup failure"))
                }
                _ => Ok(()),
            })
            .unwrap_err()
            .to_string();
        let (name, version) = if committed {
            assert!(error.contains("switch completed but cleanup failed"));
            (".node-previous", "24.0.0")
        } else {
            assert!(error.contains("injected backup failure"));
            assert!(error.contains("injected candidate cleanup failure"));
            (".node-next", "22.0.0")
        };
        assert!(error.contains(&fixture.sibling(name).display().to_string()));
        assert_eq!(
            fixture.storage.read_current().unwrap(),
            Some(Fixture::target(version).version)
        );
        assert!(junction::get_target(fixture.sibling(name)).is_ok());
        fixture.unchanged_installations();
    }
}

#[test]
fn unexpected_current_entries_are_preserved_during_publish_and_rollback() {
    for attack in [SwitchStep::Publish, SwitchStep::ReadBack] {
        let fixture = Fixture::new();
        fixture.select_first();
        let outside = fixture.base.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"preserved").unwrap();
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
                if step == attack {
                    if attack == SwitchStep::ReadBack {
                        fs::rename(fixture.current(), fixture.sibling("retained-new")).unwrap();
                    }
                    junction::create(&outside, fixture.current()).unwrap();
                }
                Ok(())
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("rollback failed"));
        assert_eq!(fs::canonicalize(fixture.current()).unwrap(), outside);
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserved");
        assert!(junction::get_target(fixture.sibling(".node-previous")).is_ok());
        fixture.unchanged_installations();
    }
}

#[test]
fn same_target_backup_replacement_is_detected_by_file_id() {
    let fixture = Fixture::new();
    fixture.select_first();
    let first = fixture.storage.root().join("installs/node/22.0.0");
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::CleanupBackup {
                fs::rename(
                    fixture.sibling(".node-previous"),
                    fixture.sibling("retained-old"),
                )
                .unwrap();
                junction::create(&first, fixture.sibling(".node-previous")).unwrap();
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("switch completed but cleanup failed"));
    assert!(error.contains("junction changed"));
    assert_eq!(
        fs::canonicalize(fixture.sibling(".node-previous")).unwrap(),
        first
    );
    assert_eq!(
        fixture.storage.read_current().unwrap(),
        Some(Fixture::target("24.0.0").version)
    );
    fixture.unchanged_installations();
}

#[test]
fn native_handle_without_delete_sharing_blocks_old_junction_backup() {
    let fixture = Fixture::new();
    fixture.select_first();
    let mut held = None;
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::BackupRename {
                held = Some(hold_junction(&fixture.current(), 1 | 2));
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("backup current junction"));
    drop(held);
    fixture.selected(Some("22.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn native_candidate_sharing_failure_restores_old_selection_and_reports_cleanup() {
    let fixture = Fixture::new();
    fixture.select_first();
    let mut held = None;
    let error = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::PublishRename {
                held = Some(hold_junction(&fixture.sibling(".node-next"), 1 | 2));
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("publish candidate junction"));
    assert!(error.contains("previous selection restored"));
    assert!(error.contains("cleanup failed"));
    assert!(fs::symlink_metadata(fixture.sibling(".node-next")).is_ok());
    drop(held);
    // Failed removal may leave the identified ordinary directory after reparse-data deletion.
    fs::remove_dir(fixture.sibling(".node-next")).unwrap();
    fixture.selected(Some("22.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn native_sharing_failures_during_readback_rollback_restore_and_cleanup_keep_backup() {
    for locked_step in [
        SwitchStep::ReadBack,
        SwitchStep::RestoreRename,
        SwitchStep::CleanupBackup,
    ] {
        let fixture = Fixture::new();
        fixture.select_first();
        let mut held = None;
        let error = fixture
            .storage
            .select_windows(&Fixture::target("24.0.0"), |step| {
                if step == locked_step {
                    let path = if step == SwitchStep::ReadBack {
                        fixture.current()
                    } else {
                        fixture.sibling(".node-previous")
                    };
                    let sharing = if step == SwitchStep::RestoreRename {
                        1 | 2
                    } else {
                        1
                    };
                    held = Some(hold_junction(&path, sharing));
                }
                if step == SwitchStep::ReadBack && locked_step != SwitchStep::CleanupBackup {
                    Err(io::Error::other("injected read-back failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .to_string();
        if locked_step == SwitchStep::CleanupBackup {
            assert!(error.contains("switch completed but cleanup failed"));
        } else {
            assert!(error.contains("injected read-back failure"));
            assert!(error.contains("rollback failed"));
        }
        drop(held);
        assert!(junction::get_target(fixture.sibling(".node-previous")).is_ok());
        assert!(fixture.storage.read_selected().is_err());
        fixture.unchanged_installations();
    }
}

#[test]
fn an_open_payload_handle_does_not_imply_that_switching_must_fail() {
    let fixture = Fixture::new();
    fixture.select_first();
    // This is an open payload, not a Node.js process or a loaded executable image.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1 | 2 | 4)
        .open(fixture.storage.root().join("installs/node/22.0.0/node.exe"))
        .unwrap();
    assert!(fixture.storage.select(&Fixture::target("24.0.0")).unwrap());
    drop(held);
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}

#[test]
fn externally_held_backup_requires_confirmed_removal_before_success() {
    let fixture = Fixture::new();
    fixture.select_first();
    let mut held = None;
    let result = fixture
        .storage
        .select_windows(&Fixture::target("24.0.0"), |step| {
            if step == SwitchStep::CleanupBackup {
                held = Some(
                    fs::OpenOptions::new()
                        .access_mode(0)
                        .share_mode(1 | 2 | 4)
                        .custom_flags(0x0020_0000 | 0x0200_0000)
                        .open(fixture.sibling(".node-previous"))
                        .unwrap(),
                );
            }
            Ok(())
        });
    let residue = fs::symlink_metadata(fixture.sibling(".node-previous"));
    match result {
        Ok(true) => {
            // Filesystems may hide an entry immediately despite its open handle.
            assert_eq!(residue.unwrap_err().kind(), io::ErrorKind::NotFound);
        }
        Err(error) => {
            assert!(
                error
                    .to_string()
                    .contains("switch completed but cleanup failed")
            );
            assert!(
                error
                    .to_string()
                    .contains(&fixture.sibling(".node-previous").display().to_string())
            );
            assert!(!matches!(residue, Err(ref error) if error.kind() == io::ErrorKind::NotFound));
        }
        Ok(false) => panic!("a different version must not return no-op"),
    }
    drop(held);
    fixture.selected(Some("24.0.0"));
    fixture.no_residue();
    fixture.unchanged_installations();
}
