use std::fs;
use std::io;
use std::path::Path;

use crate::installation::{real_directory, validate_installation};
use crate::mutation::{acquire_lock, allocate_operation, remove_operation};
use crate::storage::Storage;
use crate::target::Target;

pub(crate) fn uninstall(target: &Target) -> Result<String, String> {
    let storage = Storage::from_env().map_err(|error| format!("uninstall {target}: {error}"))?;
    uninstall_at(&storage, target, remove_operation)
        .map_err(|error| format!("uninstall {target}: {error}"))
}

// The private cleanup closure exercises partial deletion failures in offline tests.
fn uninstall_at(
    storage: &Storage,
    target: &Target,
    cleanup: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<String, String> {
    let not_installed = || format!("not installed: {target}");
    match fs::symlink_metadata(storage.root()) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_installed()),
        Err(error) => return Err(format!("inspect storage: {error}")),
        Ok(_) => {}
    }
    let root =
        fs::canonicalize(storage.root()).map_err(|error| format!("resolve storage: {error}"))?;
    let _lock = acquire_lock(&root).map_err(|error| error.to_string())?;
    let current = storage
        .read_current()
        .map_err(|error| format!("read current state: {error}"))?;
    if current == Some(target.version) {
        return Err(format!("cannot uninstall current version: {target}"));
    }

    let mut destination = root.clone();
    for component in ["installs", "node"] {
        destination.push(component);
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_installed()),
            Err(error) => return Err(format!("inspect installation ancestor: {error}")),
            Ok(_) => real_directory(&root, &destination).map_err(|error| error.to_string())?,
        }
    }
    destination.push(target.version.to_string());
    match fs::symlink_metadata(&destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(not_installed()),
        Err(error) => return Err(format!("inspect installation: {error}")),
        Ok(_) => {}
    }
    validate_installation(&root, target)
        .map_err(|error| format!("installation is not complete: {error}"))?;
    let operation =
        allocate_operation(&root).map_err(|error| format!("allocate operation: {error}"))?;
    let removed = operation.join("removed");
    let detached = (|| {
        validate_installation(&root, target)
            .map_err(|error| format!("recheck installation: {error}"))?;
        real_directory(&root, &operation).map_err(|error| error.to_string())?;
        match fs::symlink_metadata(&removed) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect detach destination: {error}")),
            Ok(_) => return Err("detach destination already exists".to_owned()),
        }
        fs::rename(&destination, &removed).map_err(|error| format!("detach installation: {error}"))
    })();
    let cleanup_result = cleanup(&root, &operation);
    match (detached, cleanup_result) {
        (Ok(()), Ok(())) => Ok(format!("uninstalled {target}")),
        (Ok(()), Err(error)) => Err(format!(
            "installation detached but cleanup incomplete at {}: {error}",
            operation.display()
        )),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!(
            "{error}; cleanup failed at {}: {cleanup_error}",
            operation.display()
        )),
    }
}

#[cfg(test)]
mod tests;
