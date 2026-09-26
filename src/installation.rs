use std::collections::BTreeMap;
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Component, Path};

use crate::distribution::{ArchiveFormat, NodeDistribution};
use crate::target::{Target, Tool, Version};

#[cfg(unix)]
mod tar;
#[cfg(windows)]
mod zip;

const RECEIPT: &str = ".verslot-install";
const RECEIPT_LIMIT: u64 = 1024;
const BYTE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const ENTRY_LIMIT: u64 = 100_000;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Extracts an already verified archive into a new staging directory and writes its receipt.
/// The caller owns the mutation lock, supplies a canonical storage root, and cleans up on failure.
pub fn extract_verified_archive(
    canonical_root: &Path,
    archive_path: &Path,
    staging: &Path,
    version: Version,
    digest: &str,
) -> io::Result<()> {
    let distribution = NodeDistribution::for_current_build(version).map_err(invalid)?;
    validate_digest(digest)?;
    regular_file(canonical_root, archive_path)?;
    let archive = File::open(archive_path)?;
    let mut destination = Extraction::new(
        canonical_root,
        staging,
        &distribution,
        BYTE_LIMIT,
        ENTRY_LIMIT,
    )?;
    match distribution.archive_format {
        #[cfg(windows)]
        ArchiveFormat::Zip => zip::extract(archive, &mut destination)?,
        #[cfg(unix)]
        ArchiveFormat::TarGz => tar::extract(archive, &mut destination)?,
        _ => return Err(invalid("archive format is unsupported on this platform")),
    }
    #[cfg(unix)]
    destination.create_links()?;
    validate_executable(canonical_root, staging, distribution.archive_format)?;
    write_receipt(canonical_root, staging, version, &distribution, digest)
}

/// Checks the receipt and executable of a real, direct installation directory without executing it.
pub fn validate_installation(canonical_root: &Path, target: &Target) -> io::Result<()> {
    let distribution = NodeDistribution::for_current_build(target.version).map_err(invalid)?;
    let directory = canonical_root
        .join("installs")
        .join(target.tool.to_string())
        .join(target.version.to_string());
    validate_directory(canonical_root, &directory, target, &distribution)
}

pub(crate) fn validate_directory(
    canonical_root: &Path,
    directory: &Path,
    target: &Target,
    distribution: &NodeDistribution,
) -> io::Result<()> {
    real_directory(canonical_root, directory)?;
    let receipt_path = directory.join(RECEIPT);
    regular_file(canonical_root, &receipt_path)?;
    let mut bytes = Vec::new();
    File::open(receipt_path)?
        .take(RECEIPT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > RECEIPT_LIMIT {
        return Err(invalid("installation receipt exceeds 1024-byte limit"));
    }
    parse_receipt(&bytes, target, distribution)?;
    validate_executable(canonical_root, directory, distribution.archive_format)
}

fn validate_digest(digest: &str) -> io::Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "receipt SHA-256 must contain 64 lowercase hexadecimal digits",
        ));
    }
    Ok(())
}

fn parse_receipt(bytes: &[u8], target: &Target, distribution: &NodeDistribution) -> io::Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("receipt is not UTF-8"))?;
    let fields: Vec<_> = text.split('\n').collect();
    if fields.len() != 5
        || fields[0] != "verslot-install-v1"
        || fields[1] != target.to_string()
        || fields[2] != distribution.archive_filename
        || !fields[4].is_empty()
    {
        return Err(invalid(
            "receipt format, target, or platform archive does not match",
        ));
    }
    validate_digest(fields[3])
}

fn write_receipt(
    root: &Path,
    directory: &Path,
    version: Version,
    distribution: &NodeDistribution,
    digest: &str,
) -> io::Result<()> {
    real_directory(root, directory)?;
    let target = Target {
        tool: Tool::Node,
        version,
    };
    let text = format!(
        "verslot-install-v1\n{target}\n{}\n{digest}\n",
        distribution.archive_filename
    );
    let mut file = File::create_new(directory.join(RECEIPT))?;
    file.write_all(text.as_bytes())?;
    file.flush()
}

fn validate_executable(root: &Path, directory: &Path, format: ArchiveFormat) -> io::Result<()> {
    real_directory(root, directory)?;
    let executable = directory.join(match format {
        ArchiveFormat::Zip => "node.exe",
        ArchiveFormat::TarGz => "bin/node",
    });
    let metadata = regular_file(root, &executable)?;
    if metadata.len() == 0 {
        return Err(invalid("Node.js executable is empty"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(invalid("Node.js executable lacks executable permission"));
        }
    }
    Ok(())
}

fn is_link(metadata: &Metadata) -> bool {
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

// Inspect each component before canonicalizing. M3 rejects internal directory links too.
pub(crate) fn real_directory(root: &Path, directory: &Path) -> io::Result<()> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| invalid("path escapes storage root"))?;
    let mut path = root.to_path_buf();
    let metadata = fs::symlink_metadata(&path)?;
    if !root.is_absolute()
        || is_link(&metadata)
        || !metadata.is_dir()
        || fs::canonicalize(root)? != root
    {
        return Err(invalid(
            "storage boundary must be a canonical real directory",
        ));
    }
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(invalid("invalid mutation path component"));
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path)?;
        if is_link(&metadata) || !metadata.is_dir() || !fs::canonicalize(&path)?.starts_with(root) {
            return Err(invalid(
                "mutation ancestor is linked, not a directory, or outside storage",
            ));
        }
    }
    Ok(())
}

fn regular_file(root: &Path, path: &Path) -> io::Result<Metadata> {
    real_directory(
        root,
        path.parent().ok_or_else(|| invalid("file has no parent"))?,
    )?;
    let metadata = fs::symlink_metadata(path)?;
    if is_link(&metadata) || !metadata.is_file() || !fs::canonicalize(path)?.starts_with(root) {
        return Err(invalid("expected a real regular file inside storage"));
    }
    Ok(metadata)
}

fn validate_component(component: &str) -> io::Result<()> {
    let device = component
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered_device = ["COM", "LPT"].iter().any(|prefix| {
        device.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.ends_with(['.', ' '])
        || component
            .chars()
            .any(|character| character.is_control() || "\\:*?\"<>|".contains(character))
        || matches!(
            device.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
        )
        || numbered_device
    {
        return Err(invalid(format!(
            "unsafe archive path component: {component:?}"
        )));
    }
    Ok(())
}

#[derive(Debug)]
enum EntryKind {
    Directory {
        explicit: bool,
    },
    File,
    #[cfg(unix)]
    Link(String),
}

struct Extraction<'a> {
    boundary: &'a Path,
    staging: &'a Path,
    archive_root: String,
    entries: BTreeMap<String, EntryKind>,
    bytes: u64,
    count: u64,
    byte_limit: u64,
    entry_limit: u64,
}

impl<'a> Extraction<'a> {
    fn new(
        boundary: &'a Path,
        staging: &'a Path,
        distribution: &NodeDistribution,
        byte_limit: u64,
        entry_limit: u64,
    ) -> io::Result<Self> {
        let extension = match distribution.archive_format {
            ArchiveFormat::Zip => ".zip",
            ArchiveFormat::TarGz => ".tar.gz",
        };
        let archive_root = distribution
            .archive_filename
            .strip_suffix(extension)
            .ok_or_else(|| invalid("unexpected archive filename"))?
            .to_owned();
        validate_component(&archive_root)?;
        real_directory(
            boundary,
            staging
                .parent()
                .ok_or_else(|| invalid("staging has no parent"))?,
        )?;
        fs::create_dir(staging)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            real_directory(boundary, staging)?;
            fs::set_permissions(staging, fs::Permissions::from_mode(0o700))?;
        }
        let mut entries = BTreeMap::new();
        entries.insert(String::new(), EntryKind::Directory { explicit: false });
        Ok(Self {
            boundary,
            staging,
            archive_root,
            entries,
            bytes: 0,
            count: 0,
            byte_limit,
            entry_limit,
        })
    }

    fn count_entry(&mut self) -> io::Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .filter(|count| *count <= self.entry_limit)
            .ok_or_else(|| invalid(format!("archive exceeds {}-entry limit", self.entry_limit)))?;
        Ok(())
    }

    fn validate_payload_path(&self, name: &[u8], directory: bool) -> io::Result<String> {
        let name = std::str::from_utf8(name).map_err(|_| invalid("archive path is not UTF-8"))?;
        let name = if directory {
            name.strip_suffix('/').unwrap_or(name)
        } else {
            name
        };
        let mut parts = name.split('/');
        if parts.next() != Some(self.archive_root.as_str()) {
            return Err(invalid(
                "archive entry has an unexpected top-level directory",
            ));
        }
        let parts: Vec<_> = parts.collect();
        for part in &parts {
            validate_component(part)?;
        }
        if parts
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case(RECEIPT))
        {
            return Err(invalid(
                "archive contains reserved installation receipt path",
            ));
        }
        if parts.is_empty() && !directory {
            return Err(invalid("archive root must be a directory"));
        }
        Ok(parts.join("/"))
    }

    fn create_parent_directories(&mut self, relative: &str) -> io::Result<()> {
        let Some((parent, _)) = relative.rsplit_once('/') else {
            return Ok(());
        };
        let mut prefix = String::new();
        for component in parent.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            match self.entries.get(&prefix) {
                Some(EntryKind::Directory { .. }) => {}
                Some(_) => return Err(invalid("archive payload ancestor is a file or link")),
                None => {
                    let path = self.staging.join(&prefix);
                    real_directory(self.boundary, path.parent().unwrap())?;
                    fs::create_dir(&path)?;
                    self.entries
                        .insert(prefix.clone(), EntryKind::Directory { explicit: false });
                }
            }
        }
        Ok(())
    }

    fn add_directory_entry(&mut self, relative: &str) -> io::Result<()> {
        self.create_parent_directories(relative)?;
        match self.entries.get_mut(relative) {
            Some(EntryKind::Directory { explicit }) if !*explicit => {
                real_directory(self.boundary, &self.staging.join(relative))?;
                *explicit = true;
            }
            Some(_) => return Err(invalid("duplicate archive path or file/directory conflict")),
            None => {
                let path = self.staging.join(relative);
                real_directory(self.boundary, path.parent().unwrap())?;
                fs::create_dir(&path)?;
                self.entries
                    .insert(relative.to_owned(), EntryKind::Directory { explicit: true });
            }
        }
        Ok(())
    }

    fn write_file_entry(
        &mut self,
        relative: &str,
        size: u64,
        mode: u32,
        reader: &mut impl Read,
    ) -> io::Result<()> {
        if self.entries.contains_key(relative) {
            return Err(invalid("duplicate archive path or file/directory conflict"));
        }
        if size > self.byte_limit - self.bytes {
            return Err(invalid("archive exceeds extracted-byte limit"));
        }
        self.create_parent_directories(relative)?;
        let path = self.staging.join(relative);
        real_directory(self.boundary, path.parent().unwrap())?;
        let mut output = File::create_new(&path)?;
        let mut buffer = [0; 64 * 1024];
        let mut written = 0_u64;
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            written = written
                .checked_add(count as u64)
                .filter(|written| *written <= size)
                .ok_or_else(|| invalid("archive payload exceeds declared size"))?;
            self.bytes = self
                .bytes
                .checked_add(count as u64)
                .filter(|bytes| *bytes <= self.byte_limit)
                .ok_or_else(|| invalid("archive exceeds extracted-byte limit"))?;
            real_directory(self.boundary, path.parent().unwrap())?;
            output.write_all(&buffer[..count])?;
        }
        if written != size {
            return Err(invalid("truncated archive payload"));
        }
        output.flush()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            regular_file(self.boundary, &path)?;
            output.set_permissions(fs::Permissions::from_mode(mode & 0o777))?;
        }
        #[cfg(not(unix))]
        let _ = mode;
        self.entries.insert(relative.to_owned(), EntryKind::File);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
