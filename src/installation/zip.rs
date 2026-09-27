use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

use super::{Extraction, invalid};

pub(super) fn extract(mut file: File, destination: &mut Extraction<'_>) -> io::Result<()> {
    // ZipArchive indexes by name and can hide duplicate central-directory records.
    // Preflight the bounded directory before its allocation, retaining every raw record.
    let count = inspect_central_directory(&mut file, destination.entry_limit)?;
    file.rewind()?;
    let config = ::zip::read::Config {
        archive_offset: ::zip::read::ArchiveOffset::Known(0),
    };
    let mut archive =
        ::zip::ZipArchive::with_config(config, file).map_err(|error| invalid(error.to_string()))?;
    if archive.len() as u64 != count {
        return Err(invalid("ZIP central-directory entries disagree"));
    }
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| invalid(error.to_string()))?;
        destination.count_entry()?;
        if entry.encrypted()
            || !matches!(
                entry.compression(),
                ::zip::CompressionMethod::Stored | ::zip::CompressionMethod::Deflated
            )
        {
            return Err(invalid("unsupported ZIP encryption or compression"));
        }
        let mode = entry.unix_mode().unwrap_or(0);
        let file_type = mode & 0o170000;
        if entry.is_symlink() || !matches!(file_type, 0 | 0o040000 | 0o100000) {
            return Err(invalid("unsupported ZIP link or special entry"));
        }
        let directory = entry.is_dir();
        if (file_type == 0o040000 && !directory) || (file_type == 0o100000 && directory) {
            return Err(invalid("ZIP file type disagrees with its path"));
        }
        destination.validate_payload_path(entry.name_raw(), directory)?;
        let relative = destination.validate_payload_path(entry.name().as_bytes(), directory)?;
        if directory {
            if entry.size() != 0 {
                return Err(invalid("ZIP directory contains payload data"));
            }
            // Reading EOF checks the directory's CRC too.
            let mut byte = [0];
            if entry.read(&mut byte)? != 0 {
                return Err(invalid("invalid ZIP directory payload"));
            }
            destination.add_directory_entry(&relative)?;
        } else {
            let size = entry.size();
            destination.write_file_entry(&relative, size, mode, &mut entry)?;
        }
    }
    Ok(())
}

fn number16(bytes: &[u8], start: usize) -> u16 {
    u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap())
}

fn number32(bytes: &[u8], start: usize) -> u64 {
    u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as u64
}

fn number64(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap())
}

fn inspect_central_directory(file: &mut File, entry_limit: u64) -> io::Result<u64> {
    let length = file.metadata()?.len();
    let tail_length = length.min(65_557);
    let mut tail = vec![0; tail_length as usize];
    file.seek(SeekFrom::Start(length - tail_length))?;
    file.read_exact(&mut tail)?;
    let end = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|index| {
            tail[*index..*index + 4] == *b"PK\x05\x06"
                && *index + 22 + number16(&tail, *index + 20) as usize == tail.len()
        })
        .ok_or_else(|| invalid("missing or truncated ZIP end record"))?;
    let footer = &tail[end..];
    if number16(footer, 4) != 0
        || number16(footer, 6) != 0
        || number16(footer, 8) != number16(footer, 10)
    {
        return Err(invalid("unsupported multi-disk ZIP"));
    }
    let mut count = number16(footer, 10) as u64;
    let mut size = number32(footer, 12);
    let mut offset = number32(footer, 16);
    let mut end_offset = length - tail_length + end as u64;
    if count == u16::MAX as u64 || size == u32::MAX as u64 || offset == u32::MAX as u64 {
        let locator_offset = end_offset
            .checked_sub(20)
            .ok_or_else(|| invalid("missing ZIP64 locator"))?;
        let mut locator = [0; 20];
        file.seek(SeekFrom::Start(locator_offset))?;
        file.read_exact(&mut locator)?;
        if locator[..4] != *b"PK\x06\x07"
            || number32(&locator, 4) != 0
            || number32(&locator, 16) != 1
        {
            return Err(invalid("invalid or multi-disk ZIP64 locator"));
        }
        end_offset = number64(&locator, 8);
        let mut record = [0; 56];
        file.seek(SeekFrom::Start(end_offset))?;
        file.read_exact(&mut record)?;
        if record[..4] != *b"PK\x06\x06"
            || number64(&record, 4) < 44
            || end_offset
                .checked_add(number64(&record, 4))
                .and_then(|end| end.checked_add(12))
                != Some(locator_offset)
            || number32(&record, 16) != 0
            || number32(&record, 20) != 0
            || number64(&record, 24) != number64(&record, 32)
        {
            return Err(invalid("invalid ZIP64 end record"));
        }
        count = number64(&record, 32);
        size = number64(&record, 40);
        offset = number64(&record, 48);
    }
    if count > entry_limit || offset.checked_add(size) != Some(end_offset) {
        return Err(invalid(
            "ZIP directory exceeds entry limit or has invalid boundaries",
        ));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut names = BTreeSet::new();
    for _ in 0..count {
        let mut header = [0; 46];
        file.read_exact(&mut header)?;
        if header[..4] != *b"PK\x01\x02" {
            return Err(invalid("invalid ZIP central header"));
        }
        let mut name = vec![0; number16(&header, 28) as usize];
        file.read_exact(&mut name)?;
        if !names.insert(name) {
            return Err(invalid("duplicate ZIP central-directory path"));
        }
        file.seek(SeekFrom::Current(
            number16(&header, 30) as i64 + number16(&header, 32) as i64,
        ))?;
        if file.stream_position()? > end_offset {
            return Err(invalid("ZIP central header escapes directory"));
        }
    }
    if file.stream_position()? != end_offset {
        return Err(invalid("ZIP central-directory size mismatch"));
    }
    Ok(count)
}
