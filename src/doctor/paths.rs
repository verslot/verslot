use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::output::format_path_for_display as display_path;
use crate::shim;

pub(super) fn first_match(path: Option<&OsStr>, command: &str) -> io::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    #[cfg(windows)]
    let names = windows_candidate_names(command);
    #[cfg(unix)]
    let names = [command];
    for directory in std::env::split_paths(path) {
        #[cfg(windows)]
        if directory.as_os_str().is_empty() {
            continue;
        }
        for name in &names {
            let candidate = directory.join(name);
            if is_executable_file(&candidate)? {
                return std::path::absolute(candidate).map(Some);
            }
        }
    }
    Ok(None)
}

#[cfg(windows)]
fn windows_candidate_names(command: &str) -> Vec<std::ffi::OsString> {
    let extensions = std::env::var_os("PATHEXT").unwrap_or_default();
    let mut names = Vec::new();
    for extension in std::env::split_paths(&extensions) {
        let extension = extension.as_os_str();
        let bytes = extension.as_encoded_bytes();
        if bytes.len() < 2
            || bytes[0] != b'.'
            || bytes
                .iter()
                .any(|&byte| matches!(byte, b'/' | b'\\' | b':' | 0))
        {
            continue;
        }
        let mut name = std::ffi::OsString::from(command);
        name.push(extension);
        names.push(name);
    }
    if names.is_empty() {
        names.push(shim::executable_name(command).into());
    }
    names
}

pub(super) fn is_executable_file(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                Ok(metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            }
            #[cfg(windows)]
            Ok(metadata.is_file())
        }
        Err(error) if is_missing(&error) => Ok(false),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("{}: {error}", display_path(path)),
        )),
    }
}

fn is_missing(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

pub(super) fn matches_shim(path: &Path, shims: &Path, command: &str) -> io::Result<bool> {
    let Some(directory) = path.parent() else {
        return Ok(false);
    };
    let expected = shim::executable_name(command);
    let expected = OsStr::new(&expected);
    if !matches_name(path, expected) {
        return Ok(false);
    }
    if directory == shims {
        return Ok(true);
    }
    // Resolve directory aliases while keeping the identity of the command entry itself.
    let shims = match shims.canonicalize() {
        Ok(shims) => shims,
        Err(error) if is_missing(&error) => return Ok(false),
        Err(error) => return Err(error),
    };
    if directory.canonicalize()? == shims {
        return Ok(true);
    }
    let resolved = path.canonicalize()?;
    Ok(resolved.parent() == Some(shims.as_path()) && matches_name(&resolved, expected))
}

fn matches_name(path: &Path, expected: &OsStr) -> bool {
    path.file_name().is_some_and(|name| {
        if cfg!(windows) {
            name.eq_ignore_ascii_case(expected)
        } else {
            name == expected
        }
    })
}

pub(super) fn yarn_node(path: Option<&OsStr>) -> io::Result<Option<PathBuf>> {
    #[cfg(windows)]
    {
        match shim::windows_node_on_path(path.unwrap_or_default()) {
            Ok(node) => Ok(Some(node)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    #[cfg(unix)]
    {
        // With no inherited PATH, exec may search a platform-dependent default.
        if path.is_none() {
            return Err(io::Error::other("PATH is unset"));
        }
        first_match(path, "node")
    }
}
