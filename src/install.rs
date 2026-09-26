use std::fs;
use std::io;
use std::path::Path;

use crate::distribution::NodeDistribution;
use crate::download::download_verified_archive;
use crate::installation::{
    extract_verified_archive, real_directory, validate_directory, validate_installation,
};
use crate::mutation::{acquire_lock, allocate_operation, create_directory, remove_operation};
use crate::storage::Storage;
use crate::target::Target;

pub(crate) fn install(target: &Target) -> Result<String, String> {
    NodeDistribution::for_current_build(target.version)
        .map_err(|error| format!("install {target}: {error}"))?;
    let storage = Storage::from_env().map_err(|error| format!("install {target}: {error}"))?;
    install_at(
        storage.root(),
        target,
        |root, operation, staging| {
            real_directory(root, operation).map_err(|error| error.to_string())?;
            let archive = operation.join("archive");
            let digest = download_verified_archive(target.version, &archive)?;
            extract_verified_archive(root, &archive, staging, target.version, &digest)
                .map_err(|error| error.to_string())
        },
        remove_operation,
    )
    .map_err(|error| format!("install {target}: {error}"))
}

// Private closures provide offline failure fixtures without exposing production URL overrides.
fn install_at(
    root: &Path,
    target: &Target,
    prepare: impl FnOnce(&Path, &Path, &Path) -> Result<(), String>,
    cleanup: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<String, String> {
    let distribution = NodeDistribution::for_current_build(target.version)?;
    fs::create_dir_all(root).map_err(|error| format!("create storage: {error}"))?;
    let root = fs::canonicalize(root).map_err(|error| format!("resolve storage: {error}"))?;
    let _lock = acquire_lock(&root).map_err(|error| error.to_string())?;
    let destination = root
        .join("installs")
        .join(target.tool.to_string())
        .join(target.version.to_string());
    // Create and inspect each mutation ancestor separately; never follow internal links.
    create_directory(&root, &root.join("installs")).map_err(|error| error.to_string())?;
    create_directory(&root, destination.parent().unwrap()).map_err(|error| error.to_string())?;
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            validate_installation(&root, target).map_err(|error| {
                format!("existing destination is not a complete installation: {error}")
            })?;
            return Ok(format!("already installed {target}"));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect destination: {error}")),
    }
    let operation =
        allocate_operation(&root).map_err(|error| format!("allocate operation: {error}"))?;
    let staging = operation.join("staging");
    let result = (|| {
        prepare(&root, &operation, &staging)?;
        validate_directory(&root, &staging, target, &distribution)
            .map_err(|error| format!("validate staging: {error}"))?;
        real_directory(&root, &staging).map_err(|error| error.to_string())?;
        real_directory(&root, destination.parent().unwrap()).map_err(|error| error.to_string())?;
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(
                    "destination appeared before commit; preserved existing entry".to_owned(),
                );
            }
            Err(error) => return Err(format!("inspect destination before commit: {error}")),
        }
        fs::rename(&staging, &destination).map_err(|error| format!("commit installation: {error}"))
    })();
    let cleanup_result = cleanup(&root, &operation);
    match (result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(format!("installed {target}")),
        (Ok(()), Err(error)) => Err(format!(
            "installation succeeded but cleanup failed at {}: {error}",
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
