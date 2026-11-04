use std::fs;
use std::io;
use std::path::Path;

use crate::installation::{real_directory, validate_installation};
use crate::storage::Storage;
use crate::target::{Target, Tool};

pub(crate) fn list_installed() -> Result<Vec<Target>, String> {
    let storage = Storage::from_env().map_err(|error| format!("list: {error}"))?;
    read_installations(storage.root()).map_err(|error| format!("list: {error}"))
}

fn read_installations(root: &Path) -> io::Result<Vec<Target>> {
    read_matching_installations(root, &Tool::ALL, |_| true)
}

pub(crate) fn read_matching_installations(
    root: &Path,
    tools: &[Tool],
    matches: impl Fn(&Target) -> bool,
) -> io::Result<Vec<Target>> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    // Resolve a linked storage root, but reject links beneath that boundary.
    let root = fs::canonicalize(root)?;
    real_directory(&root, &root)?;
    let installs = root.join("installs");
    match fs::symlink_metadata(&installs) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
        Ok(_) => real_directory(&root, &installs)?,
    }

    let mut installations = Vec::new();
    for tool in tools {
        let directory = installs.join(tool.to_string());
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
            Ok(_) => real_directory(&root, &directory)?,
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Ok(target) = format!("{tool}@{name}").parse::<Target>() else {
                continue;
            };
            if !matches(&target) {
                continue;
            }
            validate_installation(&root, &target).map_err(|error| {
                io::Error::new(error.kind(), format!("inspect {target}: {error}"))
            })?;
            installations.push(target);
        }
    }
    installations.sort_by_key(|target| {
        let version = target.version;
        (target.tool, version.major, version.minor, version.patch)
    });
    Ok(installations)
}

#[cfg(test)]
mod tests;
