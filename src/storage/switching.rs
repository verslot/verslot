use std::fs;
use std::io;
use std::path::Path;

use super::Storage;
use super::links::UnixLink;
use crate::installation::{real_directory, validate_installation};
use crate::mutation::{acquire_lock, create_directory};
use crate::target::{Target, Tool, Version};

// Private checkpoints allow deterministic failures without production environment switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SwitchStep {
    Candidate,
    Backup,
    Publish,
    ReadBack,
    Rollback,
    CleanupCandidate,
    CleanupBackup,
}

impl Storage {
    /// Selects a complete Unix installation; returns false for an unchanged selection.
    /// Holds the exclusive mutation lock through preparation, publication and recovery.
    pub fn select(&self, target: &Target) -> io::Result<bool> {
        self.select_unix(target, |_| Ok(()))
    }

    fn select_unix(
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
        let old_link = previous.map(|_| UnixLink::inspect(&current)).transpose()?;
        let mut next_link = None;
        let mut backup_link = None;

        let preparation = (|| {
            checkpoint(SwitchStep::Candidate)?;
            next_link = Some(create_link(&root, &next, &destination)?);
            verify_selection_link(&root, &next, next_link.as_ref().unwrap(), target)?;
            if let Some(version) = previous {
                checkpoint(SwitchStep::Backup)?;
                let old_target = Target {
                    tool: Tool::Node,
                    version,
                };
                let old_destination = root.join("installs/node").join(version.to_string());
                backup_link = Some(create_link(&root, &backup, &old_destination)?);
                verify_selection_link(&root, &backup, backup_link.as_ref().unwrap(), &old_target)?;
            }
            checkpoint(SwitchStep::Publish)?;
            if let Some(old_link) = &old_link {
                verify_selection_link(
                    &root,
                    &current,
                    old_link,
                    &Target {
                        tool: Tool::Node,
                        version: previous.unwrap(),
                    },
                )?;
            } else {
                verify_absent(&root, &current)?;
            }
            verify_selection_link(&root, &next, next_link.as_ref().unwrap(), target)?;
            if let Some(backup_link) = &backup_link {
                verify_selection_link(
                    &root,
                    &backup,
                    backup_link,
                    &Target {
                        tool: Tool::Node,
                        version: previous.unwrap(),
                    },
                )?;
            }
            fs::rename(&next, &current)
                .map_err(|error| path_error("publish selection", &current, error))
        })();
        if let Err(error) = preparation {
            let mut message = error.to_string();
            cleanup_link(
                &root,
                &next,
                next_link.as_ref(),
                SwitchStep::CleanupCandidate,
                &mut checkpoint,
                &mut message,
            );
            cleanup_link(
                &root,
                &backup,
                backup_link.as_ref(),
                SwitchStep::CleanupBackup,
                &mut checkpoint,
                &mut message,
            );
            return Err(io::Error::other(message));
        }

        let new_link = next_link.as_ref().unwrap();
        let read_back = (|| {
            checkpoint(SwitchStep::ReadBack)?;
            verify_selection_link(&root, &current, new_link, target)?;
            verify_current(self, &root, Some(target.version))
        })();
        if let Err(error) = read_back {
            let rollback = (|| {
                checkpoint(SwitchStep::Rollback)?;
                new_link.verify_identity(&root, &current)?;
                if let Some(backup_link) = &backup_link {
                    let old_target = Target {
                        tool: Tool::Node,
                        version: previous.unwrap(),
                    };
                    verify_selection_link(&root, &backup, backup_link, &old_target)?;
                    fs::rename(&backup, &current)
                        .map_err(|error| path_error("restore selection", &current, error))?;
                    verify_selection_link(&root, &current, backup_link, &old_target)?;
                } else {
                    new_link.remove(&root, &current)?;
                    verify_absent(&root, &current)?;
                }
                verify_current(self, &root, previous)
            })();
            return match rollback {
                Ok(()) => Err(io::Error::other(format!(
                    "{error}; previous selection restored"
                ))),
                Err(rollback_error) => {
                    let observed = match self.read_complete_current(&root) {
                        Ok(Some(version)) => format!("node@{version}"),
                        Ok(None) => "no selection".to_owned(),
                        Err(error) => format!("unavailable: {error}"),
                    };
                    let residue = match real_directory(&root, &current_directory)
                        .and_then(|()| fs::symlink_metadata(&backup))
                    {
                        Ok(_) => format!(
                            "rollback entry retained at {}; inspect it before manual recovery",
                            backup.display()
                        ),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            format!(
                                "inspect current path {} before manual recovery",
                                current.display()
                            )
                        }
                        Err(error) => {
                            format!("cannot inspect rollback path {}: {error}", backup.display())
                        }
                    };
                    Err(io::Error::other(format!(
                        "{error}; rollback failed: {rollback_error}; current state: {observed}; {residue}"
                    )))
                }
            };
        }

        let mut cleanup_error = String::new();
        cleanup_link(
            &root,
            &backup,
            backup_link.as_ref(),
            SwitchStep::CleanupBackup,
            &mut checkpoint,
            &mut cleanup_error,
        );
        if !cleanup_error.is_empty() {
            return Err(io::Error::other(format!(
                "switch completed but cleanup failed{cleanup_error}"
            )));
        }
        Ok(true)
    }
}

fn path_error(operation: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{operation} at {}: {error}", path.display()),
    )
}

fn create_link(root: &Path, path: &Path, destination: &Path) -> io::Result<UnixLink> {
    real_directory(root, path.parent().unwrap())?;
    std::os::unix::fs::symlink(destination, path)
        .map_err(|error| path_error("create symbolic link", path, error))?;
    UnixLink::inspect(path).map_err(|error| {
        path_error(
            "cannot identify prepared link; preserved entry",
            path,
            error,
        )
    })
}

fn verify_selection_link(
    root: &Path,
    path: &Path,
    link: &UnixLink,
    target: &Target,
) -> io::Result<()> {
    link.verify_identity(root, path)?;
    validate_installation(root, target)?;
    let destination = root.join("installs/node").join(target.version.to_string());
    if fs::canonicalize(path)? != destination {
        return Err(io::Error::other(format!(
            "unexpected link destination at {}",
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

fn cleanup_link(
    root: &Path,
    path: &Path,
    link: Option<&UnixLink>,
    step: SwitchStep,
    checkpoint: &mut impl FnMut(SwitchStep) -> io::Result<()>,
    message: &mut String,
) {
    if let Some(link) = link
        && let Err(error) = checkpoint(step).and_then(|()| link.remove(root, path))
    {
        message.push_str(&format!("; cleanup failed at {}: {error}", path.display()));
    }
}

#[cfg(test)]
mod tests;
