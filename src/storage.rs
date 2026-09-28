use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::installation::validate_installation;
use crate::mutation::{acquire_read_lock, reject_switch_residue};
use crate::target::{Target, Version};

mod links;

#[cfg(unix)]
mod switching;

#[cfg(windows)]
mod switching_windows;

#[cfg(windows)]
const ROOT_VARIABLE: &str = "LOCALAPPDATA";
#[cfg(windows)]
const ROOT_DIRECTORY: &str = "verslot";
#[cfg(unix)]
const ROOT_VARIABLE: &str = "HOME";
#[cfg(unix)]
const ROOT_DIRECTORY: &str = ".verslot";

#[derive(Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn from_env() -> io::Result<Self> {
        Self::from_source(std::env::var_os(ROOT_VARIABLE).as_deref())
    }

    pub(crate) fn from_source(source: Option<&OsStr>) -> io::Result<Self> {
        let source = source.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{ROOT_VARIABLE} is missing"),
            )
        })?;
        if source.is_empty() || !Path::new(source).is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{ROOT_VARIABLE} must be a non-empty absolute path"),
            ));
        }
        Ok(Self {
            root: Path::new(source).join(ROOT_DIRECTORY),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn installation_path(&self, target: &Target) -> io::Result<PathBuf> {
        self.checked_directory(&[
            "installs",
            &target.tool.to_string(),
            &target.version.to_string(),
        ])
    }

    pub fn temporary_path(&self) -> io::Result<PathBuf> {
        self.checked_directory(&["tmp"])
    }

    // Inspect the current entry separately: a dangling link is a state error.
    pub fn current_link_path(&self) -> io::Result<PathBuf> {
        Ok(self.checked_directory(&["current"])?.join("node"))
    }

    /// Reads a complete selection under a shared lock without creating any files.
    pub fn read_selected(&self) -> io::Result<Option<Version>> {
        // Validate even missing ancestors using the existing read-only path rules.
        self.current_link_path()?;
        match fs::symlink_metadata(&self.root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let root = fs::canonicalize(&self.root)?;
        let lock = acquire_read_lock(&root)?;
        reject_switch_residue(&root)?;
        if lock.is_none() {
            match fs::symlink_metadata(root.join("current/node")) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
                Ok(_) => {
                    return Err(io::Error::other(
                        "current selection exists without mutation lock",
                    ));
                }
            }
        }
        self.read_complete_current(&root)
    }

    // The caller holds the mutation lock; never reacquire it during a switch.
    pub(crate) fn read_complete_current(
        &self,
        canonical_root: &Path,
    ) -> io::Result<Option<Version>> {
        let version = self.read_current()?;
        if let Some(version) = version {
            let target = Target {
                tool: crate::target::Tool::Node,
                version,
            };
            validate_installation(canonical_root, &target).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("current installation is not complete: {error}"),
                )
            })?;
        }
        Ok(version)
    }

    // Internal state reads also protect the selected version during uninstall.
    pub(crate) fn read_current(&self) -> io::Result<Option<Version>> {
        let link = self.current_link_path()?;
        match fs::symlink_metadata(&link) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        let destination = links::read_current_link(&link)?;
        let destination = if destination.is_absolute() {
            destination
        } else {
            link.parent().unwrap().join(destination)
        };
        let resolved = fs::canonicalize(destination)?;
        let installs = fs::canonicalize(self.checked_directory(&["installs", "node"])?)?;
        if !fs::metadata(&resolved)?.is_dir() || resolved.parent() != Some(installs.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "current node must resolve to a direct installation directory",
            ));
        }
        let name = resolved
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid current version directory name",
                )
            })?;
        let target = format!("node@{name}")
            .parse::<Target>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if fs::canonicalize(self.installation_path(&target)?)? != resolved {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "current version does not match its installation path",
            ));
        }
        Ok(Some(target.version))
    }

    fn checked_directory(&self, components: &[&str]) -> io::Result<PathBuf> {
        let boundary = resolve_directory(&self.root)?;
        let mut path = self.root.clone();
        for component in components {
            path.push(component);
            let resolved = resolve_directory(&path)?;
            if !resolved.starts_with(&boundary) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage path escapes the storage root",
                ));
            }
        }
        Ok(path)
    }
}

// Resolve the nearest existing directory, then append missing path components.
// symlink_metadata must precede canonicalize so broken links stay errors.
fn resolve_directory(path: &Path) -> io::Result<PathBuf> {
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(ancestor)?;
                if !fs::metadata(&resolved)?.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        "storage path is not a directory",
                    ));
                }
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or(error)?;
                missing.push(name);
                ancestor = ancestor.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "storage path has no parent")
                })?;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;
