use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::installation::validate_installation;
use crate::mutation::{acquire_lock, acquire_read_lock, is_link, reject_switch_residue};
use crate::target::{Target, Tool, Version, VersionRequest};

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
    // Resolve partial requests only while the caller holds the mutation lock.
    fn resolve_selection(root: &Path, request: &VersionRequest) -> io::Result<Target> {
        if let Some(target) = request.exact() {
            return Ok(target);
        }
        crate::inventory::read_matching_installations(root, &[request.tool()], |target| {
            request.matches(target)
        })?
        .pop()
        .ok_or_else(|| selection_not_installed(request))
    }

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

    // Inspect the current entry separately to distinguish dangling links from absence.
    pub fn current_link_path(&self, tool: Tool) -> io::Result<PathBuf> {
        Ok(self.checked_directory(&["current"])?.join(tool.to_string()))
    }

    /// Clears a selection under the mutation lock without inspecting its installation.
    /// Returns false when the selection entry is already absent.
    pub fn clear_selection(&self, tool: Tool) -> io::Result<bool> {
        self.current_link_path(tool)?;
        match fs::symlink_metadata(self.root()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let root = fs::canonicalize(self.root())?;
        let _lock = acquire_lock(&root)?;
        let current = root.join("current").join(tool.to_string());
        match fs::symlink_metadata(&current) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
            Ok(_) => {}
        }

        #[cfg(unix)]
        links::UnixLink::inspect(&current)?.remove(&root, &current)?;
        #[cfg(windows)]
        links::WindowsJunction::inspect(&current)?.remove(&root, &current)?;

        match fs::symlink_metadata(&current) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(error),
            Ok(_) => Err(io::Error::other(format!(
                "selection removal incomplete at {}; entry is still present",
                current.display()
            ))),
        }
    }

    /// Reads a complete selection under a shared lock without creating any files.
    pub fn read_selected(&self, tool: Tool) -> io::Result<Option<Version>> {
        Ok(self
            .read_selected_tools(&[tool])?
            .first()
            .map(|target| target.version))
    }

    pub(crate) fn read_selections(&self) -> io::Result<Vec<Target>> {
        self.read_selected_tools(&Tool::ALL)
    }

    /// Retains each tool's result from the same read lock for diagnostics.
    pub(crate) fn inspect_selections(&self) -> io::Result<[io::Result<Option<Version>>; 2]> {
        let Some((root, lock)) = self.open_selection_read()? else {
            return Ok([Ok(None), Ok(None)]);
        };
        Ok(Tool::ALL.map(|tool| self.read_locked_selection(&root, lock.as_ref(), tool)))
    }

    fn read_selected_tools(&self, tools: &[Tool]) -> io::Result<Vec<Target>> {
        let Some((root, lock)) = self.open_selection_read()? else {
            return Ok(Vec::new());
        };
        let mut selected = Vec::new();
        for &tool in tools {
            if let Some(version) = self.read_locked_selection(&root, lock.as_ref(), tool)? {
                selected.push(Target { tool, version });
            }
        }
        Ok(selected)
    }

    fn open_selection_read(&self) -> io::Result<Option<(PathBuf, Option<File>)>> {
        // Validate even missing ancestors using the existing read-only path rules.
        self.checked_directory(&["current"])?;
        match fs::symlink_metadata(&self.root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let root = fs::canonicalize(&self.root)?;
        let lock = acquire_read_lock(&root)?;
        reject_switch_residue(&root)?;
        Ok(Some((root, lock)))
    }

    fn read_locked_selection(
        &self,
        root: &Path,
        lock: Option<&File>,
        tool: Tool,
    ) -> io::Result<Option<Version>> {
        if lock.is_none() {
            return match fs::symlink_metadata(root.join("current").join(tool.to_string())) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
                Ok(_) => Err(io::Error::other(
                    "current selection exists without mutation lock",
                )),
            };
        }
        self.read_complete_current(root, tool)
    }

    // The caller holds the mutation lock; never reacquire it during a switch.
    pub(crate) fn read_complete_current(
        &self,
        canonical_root: &Path,
        tool: Tool,
    ) -> io::Result<Option<Version>> {
        let version = self.read_current(tool)?;
        if let Some(version) = version {
            let target = Target { tool, version };
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
    pub(crate) fn read_current(&self, tool: Tool) -> io::Result<Option<Version>> {
        let link = self.current_link_path(tool)?;
        match fs::symlink_metadata(&link) {
            Ok(metadata) if !is_link(&metadata) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("expected a selection link at {}", link.display()),
                ));
            }
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
        let installs = fs::canonicalize(self.checked_directory(&["installs", &tool.to_string()])?)?;
        if !fs::metadata(&resolved)?.is_dir() || resolved.parent() != Some(installs.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("current {tool} must resolve to a direct installation directory"),
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
        let target = format!("{tool}@{name}")
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

fn selection_not_installed(request: &VersionRequest) -> io::Error {
    if let Some(target) = request.exact() {
        io::Error::other(format!("not installed: {target}"))
    } else {
        io::Error::other(format!("no installed version matches {request}"))
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
