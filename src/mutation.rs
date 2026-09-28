use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::installation::real_directory;

pub(crate) fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(crate) fn create_directory(root: &Path, path: &Path) -> io::Result<()> {
    real_directory(
        root,
        path.parent()
            .ok_or_else(|| io::Error::other("directory has no parent"))?,
    )?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    real_directory(root, path)
}

pub(crate) fn acquire_lock(root: &Path) -> io::Result<File> {
    real_directory(root, root)?;
    let path = root.join(".mutation.lock");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if is_link(&metadata) || !metadata.is_file() => {
            return Err(io::Error::other("mutation lock must be a regular file"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    match File::create_new(&path) {
        Ok(file) => drop(file),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&path)?;
    if is_link(&metadata) || !metadata.is_file() {
        return Err(io::Error::other("mutation lock must be a regular file"));
    }
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    file.try_lock().map_err(lock_error)?;
    reject_switch_residue(root)?;
    Ok(file)
}

fn lock_error(error: std::fs::TryLockError) -> io::Error {
    match error {
        std::fs::TryLockError::WouldBlock => io::Error::other(
            "storage is busy; retry after the active operation finishes (mutation lock)",
        ),
        std::fs::TryLockError::Error(error) => {
            io::Error::other(format!("cannot acquire mutation lock: {error}"))
        }
    }
}

// Open an existing persistent lock read-only; queries never create it.
pub(crate) fn acquire_read_lock(root: &Path) -> io::Result<Option<File>> {
    real_directory(root, root)?;
    let path = root.join(".mutation.lock");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if is_link(&metadata) || !metadata.is_file() => {
            return Err(io::Error::other("mutation lock must be a regular file"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let file = File::open(&path)?;
    file.try_lock_shared().map_err(lock_error)?;
    Ok(Some(file))
}

// Check ancestors before entries, and never follow or remove reserved links.
pub(crate) fn reject_switch_residue(root: &Path) -> io::Result<()> {
    real_directory(root, root)?;
    let current = root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(_) => real_directory(root, &current)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    for name in [".node-next", ".node-previous"] {
        let path = current.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "unfinished switch at {}; inspect the links and referenced installations before manual recovery",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn allocate_operation(root: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = root.join("tmp");
    create_directory(root, &temporary)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    loop {
        real_directory(root, &temporary)?;
        let path = temporary.join(format!(
            "{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(error) =
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    {
                        let _ = fs::remove_dir(&path);
                        return Err(error);
                    }
                }
                real_directory(root, &path)?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

// Remove only the supplied operation tree; inspect each entry before descending.
pub(crate) fn remove_operation(root: &Path, operation: &Path) -> io::Result<()> {
    let temporary = root.join("tmp");
    if operation.parent() != Some(temporary.as_path()) {
        return Err(io::Error::other(
            "cleanup target is not a direct operation directory",
        ));
    }
    real_directory(root, operation)?;
    remove_tree(root, operation)
}

fn remove_tree(root: &Path, directory: &Path) -> io::Result<()> {
    real_directory(root, directory)?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        real_directory(root, directory)?;
        let metadata = fs::symlink_metadata(&path)?;
        if is_link(&metadata) {
            #[cfg(windows)]
            return Err(io::Error::other(format!(
                "unexpected reparse point during cleanup: {}",
                path.display()
            )));
            #[cfg(not(windows))]
            fs::remove_file(&path)?;
        } else if metadata.is_dir() {
            remove_tree(root, &path)?;
        } else if metadata.is_file() {
            fs::remove_file(&path)?;
        } else {
            return Err(io::Error::other("unexpected file type during cleanup"));
        }
    }
    real_directory(root, directory)?;
    fs::remove_dir(directory)
}
