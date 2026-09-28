use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub(super) fn read_current_link(path: &Path) -> io::Result<PathBuf> {
    std::fs::read_link(path)
}

#[cfg(unix)]
pub(super) struct UnixLink {
    device: u64,
    inode: u64,
    destination: PathBuf,
}

#[cfg(unix)]
impl UnixLink {
    pub(super) fn inspect(path: &Path) -> io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.file_type().is_symlink() {
            return Err(io::Error::other(format!(
                "expected symbolic link at {}",
                path.display()
            )));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            destination: std::fs::read_link(path)?,
        })
    }

    pub(super) fn verify_identity(&self, root: &Path, path: &Path) -> io::Result<()> {
        crate::installation::real_directory(root, path.parent().unwrap())?;
        let observed = Self::inspect(path)?;
        if self.device != observed.device
            || self.inode != observed.inode
            || self.destination != observed.destination
        {
            return Err(io::Error::other(format!(
                "link changed at {}; preserved unexpected entry",
                path.display()
            )));
        }
        Ok(())
    }

    pub(super) fn remove(&self, root: &Path, path: &Path) -> io::Result<()> {
        self.verify_identity(root, path)?;
        std::fs::remove_file(path)
    }
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

#[cfg(windows)]
pub(super) struct WindowsJunction {
    // Keep the original entry alive so its file ID cannot be reused during the operation.
    _file: std::fs::File,
    volume: u64,
    index: u64,
    destination: PathBuf,
}

#[cfg(windows)]
impl WindowsJunction {
    pub(super) fn inspect(path: &Path) -> io::Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !crate::mutation::is_link(&metadata) {
            return Err(io::Error::other(format!(
                "expected junction at {}",
                path.display()
            )));
        }
        // get_target checks the mount-point tag even when the destination is missing.
        let destination = junction::get_target(path)?;
        let file = Self::open_entry(path)?;
        let information = winapi_util::file::information(&file)?;
        Ok(Self {
            _file: file,
            volume: information.volume_serial_number(),
            index: information.file_index(),
            destination,
        })
    }

    fn open_entry(path: &Path) -> io::Result<std::fs::File> {
        use std::os::windows::fs::OpenOptionsExt;
        // OPEN_REPARSE_POINT inspects the junction itself; BACKUP_SEMANTICS opens directories.
        // Zero desired access is compatible with junction's exclusive read/write handles.
        std::fs::OpenOptions::new()
            .access_mode(0)
            .custom_flags(0x0020_0000 | 0x0200_0000)
            .share_mode(1 | 2 | 4)
            .open(path)
    }

    pub(super) fn verify_identity(&self, root: &Path, path: &Path) -> io::Result<()> {
        crate::installation::real_directory(root, path.parent().unwrap())?;
        let observed = Self::inspect(path)?;
        if self.volume != observed.volume
            || self.index != observed.index
            || self.destination != observed.destination
        {
            return Err(io::Error::other(format!(
                "junction changed at {}; preserved unexpected entry",
                path.display()
            )));
        }
        Ok(())
    }

    pub(super) fn remove(self, root: &Path, path: &Path) -> io::Result<()> {
        self.verify_identity(root, path)?;
        // junction 2.0 removes the reparse data but leaves its ordinary directory.
        junction::delete(path)?;
        crate::installation::real_directory(root, path.parent().unwrap())?;
        let file = Self::open_entry(path)?;
        let information = winapi_util::file::information(&file)?;
        let metadata = std::fs::symlink_metadata(path)?;
        if self.volume != information.volume_serial_number()
            || self.index != information.file_index()
            || crate::mutation::is_link(&metadata)
            || !metadata.is_dir()
        {
            return Err(io::Error::other(format!(
                "junction directory changed at {}; preserved unexpected entry",
                path.display()
            )));
        }
        // Close identity handles before removing the directory so Windows can free its name.
        drop(file);
        drop(self);
        std::fs::remove_dir(path)?;
        crate::installation::real_directory(root, path.parent().unwrap())?;
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot confirm junction removal at {}: {error}",
                    path.display()
                ),
            )),
            Ok(_) => Err(io::Error::other(format!(
                "junction removal incomplete at {}; entry is still present",
                path.display()
            ))),
        }
    }
}
