use std::fs;
use std::io;
use std::path::Path;

use crate::installation::{real_directory, validate_installation};
use crate::storage::Storage;
use crate::target::Target;

pub(crate) fn list_installed() -> Result<Vec<Target>, String> {
    let storage = Storage::from_env().map_err(|error| format!("list: {error}"))?;
    read_installations(storage.root()).map_err(|error| format!("list: {error}"))
}

fn read_installations(root: &Path) -> io::Result<Vec<Target>> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    // Resolve a linked storage root, but reject links beneath that boundary.
    let root = fs::canonicalize(root)?;
    real_directory(&root, &root)?;
    let mut directory = root.clone();
    for component in ["installs", "node"] {
        directory.push(component);
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
            Ok(_) => real_directory(&root, &directory)?,
        }
    }

    let mut installations = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(target) = format!("node@{name}").parse::<Target>() else {
            continue;
        };
        validate_installation(&root, &target)
            .map_err(|error| io::Error::new(error.kind(), format!("inspect {target}: {error}")))?;
        installations.push(target);
    }
    installations.sort_by_key(|target| {
        let version = target.version;
        (version.major, version.minor, version.patch)
    });
    Ok(installations)
}

#[cfg(test)]
mod tests;
