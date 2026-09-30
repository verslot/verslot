use std::fs;
use std::io;
use std::path::Path;

use super::Storage;
use super::links::WindowsJunction;
use crate::installation::{real_directory, validate_installation};
use crate::mutation::{acquire_lock, create_directory};
use crate::target::{Target, Tool, Version};

// Private checkpoints exercise failures without production environment switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SwitchStep {
    Candidate,
    Backup,
    BackupRename,
    Publish,
    PublishRename,
    ReadBack,
    Rollback,
    Restore,
    RestoreRename,
    CleanupCandidate,
    CleanupBackup,
}

impl Storage {
    /// Selects a complete Windows installation; false means an unchanged selection.
    /// Keeps the exclusive lock through the junction visibility gap and all recovery.
    pub fn select(&self, target: &Target) -> io::Result<bool> {
        self.select_windows(target, |_| Ok(()))
    }

    fn select_windows(
        &self,
        target: &Target,
        mut checkpoint: impl FnMut(SwitchStep) -> io::Result<()>,
    ) -> io::Result<bool> {
        self.current_link_path()?;
        let not_installed = || io::Error::other(format!("not installed: {target}"));
        match fs::symlink_metadata(self.root()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_installed()),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let root = fs::canonicalize(self.root())?;
        let _lock = acquire_lock(&root)?;
        let previous = self.read_complete_current(&root)?;
        let destination = root.join("installs/node").join(target.version.to_string());
        real_directory(&root, destination.parent().unwrap()).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                not_installed()
            } else {
                error
            }
        })?;
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_installed()),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        validate_installation(&root, target)?;
        if previous == Some(target.version) {
            return Ok(false);
        }

        let current_directory = root.join("current");
        create_directory(&root, &current_directory)?;
        let current = current_directory.join("node");
        let next = current_directory.join(".node-next");
        let backup = current_directory.join(".node-previous");
        let mut old_link = previous
            .map(|_| WindowsJunction::inspect(&current))
            .transpose()?;
        let mut candidate = None;
        let mut backed_up = false;
        let mut published = false;

        let update = (|| {
            checkpoint(SwitchStep::Candidate)?;
            real_directory(&root, &current_directory)?;
            junction::create(&destination, &next).map_err(|error| {
                path_error(
                    "create candidate junction; inspect possible residue",
                    &next,
                    error,
                )
            })?;
            candidate = Some(WindowsJunction::inspect(&next).map_err(|error| {
                path_error("identify candidate junction; preserved entry", &next, error)
            })?);
            verify_selection_link(&root, &next, candidate.as_ref().unwrap(), target)?;
            if let Some(old_link) = &old_link {
                checkpoint(SwitchStep::Backup)?;
                verify_selection_link(&root, &current, old_link, &node_target(previous.unwrap()))?;
                verify_absent(&root, &backup)?;
                checkpoint(SwitchStep::BackupRename)?;
                fs::rename(&current, &backup)
                    .map_err(|error| path_error("backup current junction", &backup, error))?;
                backed_up = true;
                verify_selection_link(&root, &backup, old_link, &node_target(previous.unwrap()))?;
            }
            checkpoint(SwitchStep::Publish)?;
            verify_absent(&root, &current)?;
            verify_selection_link(&root, &next, candidate.as_ref().unwrap(), target)?;
            checkpoint(SwitchStep::PublishRename)?;
            fs::rename(&next, &current)
                .map_err(|error| path_error("publish candidate junction", &current, error))?;
            published = true;
            checkpoint(SwitchStep::ReadBack)?;
            verify_selection_link(&root, &current, candidate.as_ref().unwrap(), target)?;
            verify_current(self, &root, Some(target.version))
        })();

        if let Err(error) = update {
            let mut message = error.to_string();
            if backed_up || published {
                let rollback = (|| {
                    checkpoint(SwitchStep::Rollback)?;
                    if published {
                        candidate
                            .take()
                            .unwrap()
                            .remove(&root, &current)
                            .map_err(|error| {
                                path_error(
                                    "remove published junction during rollback",
                                    &current,
                                    error,
                                )
                            })?;
                    }
                    verify_absent(&root, &current)?;
                    if backed_up {
                        let old_link = old_link.as_ref().unwrap();
                        checkpoint(SwitchStep::Restore)?;
                        verify_selection_link(
                            &root,
                            &backup,
                            old_link,
                            &node_target(previous.unwrap()),
                        )?;
                        verify_absent(&root, &current)?;
                        checkpoint(SwitchStep::RestoreRename)?;
                        fs::rename(&backup, &current).map_err(|error| {
                            path_error("restore previous junction", &current, error)
                        })?;
                        verify_selection_link(
                            &root,
                            &current,
                            old_link,
                            &node_target(previous.unwrap()),
                        )?;
                    }
                    verify_current(self, &root, previous)
                })();
                match rollback {
                    Ok(()) => message.push_str("; previous selection restored"),
                    Err(rollback_error) => {
                        message.push_str(&format!("; rollback failed: {rollback_error}"));
                        let observed = match self.read_complete_current(&root) {
                            Ok(Some(version)) => format!("node@{version}"),
                            Ok(None) => "no selection".to_owned(),
                            Err(error) => format!("unavailable: {error}"),
                        };
                        message.push_str(&format!("; current state: {observed}"));
                        let recovery_path = match real_directory(&root, &current_directory)
                            .and_then(|()| fs::symlink_metadata(&backup))
                        {
                            Ok(_) => format!(
                                "rollback entry retained at {}; inspect it before manual recovery",
                                backup.display()
                            ),
                            Err(error) if error.kind() == io::ErrorKind::NotFound => format!(
                                "inspect current path {} before manual recovery",
                                current.display()
                            ),
                            Err(error) => format!(
                                "cannot inspect rollback path {}: {error}",
                                backup.display()
                            ),
                        };
                        message.push_str(&format!("; {recovery_path}"));
                    }
                }
            }
            // The candidate still lives at next only if publication never succeeded.
            // A retained backup is never removed after any failed update or rollback.
            if !published {
                cleanup_candidate(
                    &root,
                    &next,
                    candidate.take(),
                    &mut checkpoint,
                    &mut message,
                );
            }
            return Err(io::Error::other(message));
        }

        if let Some(old_link) = old_link.take() {
            checkpoint(SwitchStep::CleanupBackup)
                .and_then(|()| old_link.remove(&root, &backup))
                .map_err(|error| {
                    io::Error::other(format!(
                        "switch completed but cleanup failed at {}: {error}",
                        backup.display()
                    ))
                })?;
        }
        Ok(true)
    }
}

fn node_target(version: Version) -> Target {
    Target {
        tool: Tool::Node,
        version,
    }
}

fn path_error(operation: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{operation} at {}: {error}", path.display()),
    )
}

fn verify_selection_link(
    root: &Path,
    path: &Path,
    link: &WindowsJunction,
    target: &Target,
) -> io::Result<()> {
    link.verify_identity(root, path)?;
    validate_installation(root, target)?;
    if fs::canonicalize(path)? != root.join("installs/node").join(target.version.to_string()) {
        return Err(io::Error::other(format!(
            "unexpected junction destination at {}",
            path.display()
        )));
    }
    Ok(())
}

fn verify_absent(root: &Path, path: &Path) -> io::Result<()> {
    real_directory(root, path.parent().unwrap())?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::other(format!(
            "unexpected entry at {}; preserved entry",
            path.display()
        ))),
    }
}

fn verify_current(storage: &Storage, root: &Path, expected: Option<Version>) -> io::Result<()> {
    if storage.read_complete_current(root)? != expected {
        return Err(io::Error::other(
            "selection read-back does not match expected version",
        ));
    }
    Ok(())
}

fn cleanup_candidate(
    root: &Path,
    path: &Path,
    candidate: Option<WindowsJunction>,
    checkpoint: &mut impl FnMut(SwitchStep) -> io::Result<()>,
    message: &mut String,
) {
    if let Some(candidate) = candidate
        && let Err(error) =
            checkpoint(SwitchStep::CleanupCandidate).and_then(|()| candidate.remove(root, path))
    {
        message.push_str(&format!("; cleanup failed at {}: {error}", path.display()));
    }
}

#[cfg(test)]
mod tests;
