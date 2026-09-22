use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub(super) fn read_current_link(path: &Path) -> io::Result<PathBuf> {
    std::fs::read_link(path)
}

#[cfg(windows)]
pub(super) fn read_current_link(path: &Path) -> io::Result<PathBuf> {
    if !junction::exists(path)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "current node must be a directory junction",
        ));
    }
    junction::get_target(path)
}
