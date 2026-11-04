use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::installation::{real_directory, regular_file};
use crate::mutation::{
    acquire_exclusive_lock, allocate_operation, create_directory, remove_operation,
};
use crate::shim::{COMMANDS, executable_name, is_command_file};
use crate::storage::Storage;

const RECEIPT: &str = ".verslot-shims";
const LEGACY_RECEIPT_VERSION: &str = "verslot-shims-v1";
const RECEIPT_VERSION: &str = "verslot-shims-v2";

pub(crate) fn setup() -> io::Result<PathBuf> {
    let storage = Storage::from_env()?;
    let source = fs::canonicalize(std::env::current_exe()?)?;
    setup_at(storage.root(), &source, |source, shim| {
        fs::hard_link(source, shim)
    })
    .map_err(|error| {
        if cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33)) {
            io::Error::new(
                error.kind(),
                format!("{error}; check permissions, close related processes and retry"),
            )
        } else {
            error
        }
    })
}

fn setup_at(
    root: &Path,
    source: &Path,
    create_hard_link: impl Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let root = fs::canonicalize(root)?;
    let _lock = acquire_exclusive_lock(&root)?;
    let directory = root.join("shims");
    let backup = root.join(".shims-previous");
    // A committed update may retain a Windows executable until its process exits.
    if fs::symlink_metadata(&backup).is_ok() {
        validate_owned_directory(&root, &directory, false).map_err(|error| {
            io::Error::other(format!(
                "unfinished setup; previous shims preserved at {}: {error}",
                backup.display()
            ))
        })?;
        remove_owned_directory(&root, &backup).map_err(|error| {
            io::Error::other(format!(
                "cannot clean previous shims at {}; close related processes and retry: {error}",
                backup.display()
            ))
        })?;
    }
    let digest = file_digest(source)?;
    let previous = match fs::symlink_metadata(&directory) {
        Ok(_) => Some(validate_owned_directory(&root, &directory, false)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let receipt = format!(
        "{RECEIPT_VERSION}\n{digest}\n{}\n",
        COMMANDS.map(executable_name).join("\n")
    );
    if previous.as_deref() == Some(receipt.as_str()) {
        return Ok(directory);
    }
    let operation = allocate_operation(&root)?;
    let candidate = operation.join("shims");
    let result = (|| {
        create_directory(&root, &candidate)?;
        let executable = candidate.join(executable_name("verslot"));
        copy_shim(source, &executable)?;
        for name in &COMMANDS[1..] {
            let shim = candidate.join(executable_name(name));
            match create_hard_link(&executable, &shim) {
                Ok(()) => {}
                Err(error) if hard_link_unavailable(&error) => copy_shim(&executable, &shim)?,
                Err(error) => return Err(error),
            }
        }
        let mut marker = File::create_new(candidate.join(RECEIPT))?;
        marker.write_all(receipt.as_bytes())?;
        marker.sync_all()?;
        drop(marker);
        validate_owned_directory(&root, &candidate, false)?;
        publish_shims(&root, &candidate, &directory, &backup, previous.is_some())?;
        Ok(directory)
    })();
    let cleanup = remove_operation(&root, &operation);
    match (result, cleanup) {
        (Ok(directory), Ok(())) => Ok(directory),
        (Err(error), Ok(())) => Err(error),
        (result, Err(error)) => Err(io::Error::other(format!(
            "{}; setup cleanup failed at {}: {error}",
            result
                .err()
                .map_or_else(|| "shims updated".to_owned(), |error| error.to_string()),
            operation.display()
        ))),
    }
}

fn copy_shim(source: &Path, destination: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let mut output = File::create_new(destination)?;
    io::copy(&mut input, &mut output)?;
    output.set_permissions(input.metadata()?.permissions())?;
    output.sync_all()
}

fn hard_link_unavailable(error: &io::Error) -> bool {
    // Windows can report ERROR_INVALID_FUNCTION or ERROR_NOT_SUPPORTED.
    // Linux uses EPERM for both unsupported filesystems and link policy restrictions.
    error.kind() == io::ErrorKind::Unsupported
        || (cfg!(windows) && matches!(error.raw_os_error(), Some(1 | 50)))
        || (cfg!(target_os = "linux") && error.raw_os_error() == Some(1))
}

fn publish_shims(
    root: &Path,
    candidate: &Path,
    directory: &Path,
    backup: &Path,
    previous: bool,
) -> io::Result<()> {
    real_directory(root, root)?;
    if previous {
        validate_owned_directory(root, directory, false)?;
        require_missing(backup)?;
        fs::rename(directory, backup).map_err(|error| {
            io::Error::other(format!(
                "cannot replace {}; close related processes and retry: {error}",
                directory.display()
            ))
        })?;
    }
    let publication = require_missing(directory).and_then(|()| fs::rename(candidate, directory));
    if let Err(error) = publication {
        if previous
            && let Err(rollback) =
                require_missing(directory).and_then(|()| fs::rename(backup, directory))
        {
            return Err(io::Error::other(format!(
                "publish shims failed: {error}; restore failed: {rollback}; previous shims retained at {}",
                backup.display()
            )));
        }
        return Err(error);
    }
    if previous {
        remove_owned_directory(root, backup).map_err(|error| {
            io::Error::other(format!(
                "shims updated but cleanup failed at {}; close related processes and retry: {error}", backup.display()
            ))
        })?;
    }
    Ok(())
}

fn require_missing(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::other(format!(
            "unexpected entry at {}; preserved entry",
            path.display()
        ))),
    }
}

// Receipts prove which flat files setup may replace or remove. Unknown entries,
// modified executables, symlinks and reparse points are preserved.
fn validate_owned_directory(
    root: &Path,
    directory: &Path,
    allow_missing: bool,
) -> io::Result<String> {
    real_directory(root, directory)?;
    let marker = directory.join(RECEIPT);
    regular_file(root, &marker)?;
    let mut receipt = String::new();
    File::open(marker)?
        .take(4097)
        .read_to_string(&mut receipt)?;
    let lines: Vec<_> = receipt.lines().collect();
    if receipt.len() > 4096
        || lines.len() < 3
        || !matches!(lines[0], LEGACY_RECEIPT_VERSION | RECEIPT_VERSION)
        || lines[1].len() != 64
        || !lines[1].bytes().all(|byte| byte.is_ascii_hexdigit())
        || !lines[2..].contains(&executable_name("verslot").as_str())
        || lines[2..].iter().enumerate().any(|(index, name)| {
            !is_command_file(std::ffi::OsStr::new(name)) || lines[2..2 + index].contains(name)
        })
    {
        return Err(io::Error::other(format!(
            "invalid shim ownership receipt at {}; preserved directory",
            directory.display()
        )));
    }
    for entry in fs::read_dir(directory)? {
        let name = entry?.file_name();
        if name != RECEIPT && !lines[2..].iter().any(|expected| name == *expected) {
            return Err(io::Error::other(format!(
                "unknown entry in {}; preserved directory",
                directory.display()
            )));
        }
    }
    for name in &lines[2..] {
        let path = directory.join(name);
        match regular_file(root, &path) {
            Err(error) if allow_missing && error.kind() == io::ErrorKind::NotFound => continue,
            result => result?,
        };
        if file_digest(&path)? != lines[1] {
            return Err(io::Error::other(format!(
                "modified shim at {}; preserved file",
                path.display()
            )));
        }
    }
    Ok(receipt)
}

fn remove_owned_directory(root: &Path, directory: &Path) -> io::Result<()> {
    let receipt = validate_owned_directory(root, directory, true)?;
    for name in receipt.lines().skip(2) {
        let path = directory.join(name);
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::remove_file(directory.join(RECEIPT))?;
    fs::remove_dir(directory)
}

fn file_digest(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect());
        }
        digest.update(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests;
