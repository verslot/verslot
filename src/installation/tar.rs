use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File};
use std::io::{self, Read};

use flate2::read::MultiGzDecoder;

use super::{EntryKind, Extraction, invalid, real_directory, validate_component};

const METADATA_LIMIT: u64 = 1024 * 1024;

pub(super) fn extract(archive: File, destination: &mut Extraction<'_>) -> io::Result<()> {
    let mut reader = TarReader {
        decoder: MultiGzDecoder::new(archive),
        total: 0,
        last_block: [0; 512],
        limit: destination.byte_limit + destination.entry_limit * 1024 + 1024,
    };
    let mut global = BTreeMap::new();
    let mut local = BTreeMap::new();
    let mut long_name = None;
    let mut long_link = None;
    let mut local_pending = false;
    {
        let mut archive = ::tar::Archive::new(&mut reader);
        for entry in archive.entries()?.raw(true) {
            let mut entry = entry?;
            destination.count_entry()?;
            validate_header_names(entry.header())?;
            let kind = entry.header().entry_type();
            let size = entry.size();
            if kind.is_gnu_longname()
                || kind.is_gnu_longlink()
                || kind.is_pax_local_extensions()
                || kind.is_pax_global_extensions()
            {
                if size > METADATA_LIMIT || size > destination.byte_limit - destination.bytes {
                    return Err(invalid("tar metadata exceeds byte limit"));
                }
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                if bytes.len() as u64 != size {
                    return Err(invalid("truncated tar metadata"));
                }
                destination.bytes += size;
                if kind.is_gnu_longname() || kind.is_gnu_longlink() {
                    if bytes.last() == Some(&0) {
                        bytes.pop();
                    }
                    let field = if kind.is_gnu_longname() {
                        &mut long_name
                    } else {
                        &mut long_link
                    };
                    if field.replace(bytes).is_some() {
                        return Err(invalid("duplicate GNU name metadata"));
                    }
                } else {
                    if kind.is_pax_local_extensions() && local_pending {
                        return Err(invalid("duplicate local PAX metadata"));
                    }
                    let fields = parse_pax(&bytes)?;
                    if kind.is_pax_global_extensions() {
                        global.extend(fields);
                    } else {
                        local = fields;
                        local_pending = true;
                    }
                }
                continue;
            }
            let mut fields = global.clone();
            fields.append(&mut local);
            local_pending = false;
            if let Some(pax_size) = fields.get("size") {
                let pax_size = std::str::from_utf8(pax_size)
                    .map_err(|_| invalid("invalid PAX size"))?
                    .parse::<u64>()
                    .map_err(|_| invalid("invalid PAX size"))?;
                // Raw entry iteration uses header sizes. Reject alternate-size encodings rather
                // than let different parsers disagree about the following header boundary.
                if pax_size != size {
                    return Err(invalid("unsupported PAX size override"));
                }
            }
            let header_name = entry.header().path_bytes().into_owned();
            let name = effective_name(long_name.take(), fields.remove("path"), header_name)?;
            let relative = destination.validate_payload_path(&name, kind.is_dir())?;
            if kind.is_file() {
                if long_link.is_some() || fields.contains_key("linkpath") {
                    return Err(invalid("link metadata on a regular file"));
                }
                let mode = entry.header().mode()?;
                destination.write_file_entry(&relative, size, mode, &mut entry)?;
            } else if kind.is_dir() {
                if size != 0 || long_link.is_some() || fields.contains_key("linkpath") {
                    return Err(invalid("invalid tar directory payload"));
                }
                destination.add_directory_entry(&relative)?;
            } else if kind.is_symlink() {
                if size != 0 {
                    return Err(invalid("tar link contains payload data"));
                }
                let header_link = entry
                    .header()
                    .link_name_bytes()
                    .map(|name| name.into_owned())
                    .unwrap_or_default();
                let target =
                    effective_name(long_link.take(), fields.remove("linkpath"), header_link)?;
                let target = std::str::from_utf8(&target)
                    .map_err(|_| invalid("link target is not UTF-8"))?;
                destination.queue_link(&relative, target)?;
            } else {
                return Err(invalid(
                    "unsupported tar entry type (including hard links and sparse files)",
                ));
            }
        }
    }
    if long_name.is_some() || long_link.is_some() || local_pending {
        return Err(invalid("tar metadata has no following payload entry"));
    }
    if reader.total < 512 || reader.last_block != [0; 512] {
        return Err(invalid("tar archive lacks its end marker"));
    }
    // Read through the gzip trailer to validate its CRC/length, and reject hidden tar payloads.
    let mut padding = 0_u64;
    let mut buffer = [0; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if buffer[..count].iter().any(|byte| *byte != 0) {
            return Err(invalid("nonzero data after tar end marker"));
        }
        padding += count as u64;
    }
    if padding < 512 || !reader.total.is_multiple_of(512) {
        return Err(invalid("truncated tar end marker or padding"));
    }
    Ok(())
}

fn effective_name(
    gnu: Option<Vec<u8>>,
    pax: Option<Vec<u8>>,
    header: Vec<u8>,
) -> io::Result<Vec<u8>> {
    match (gnu, pax) {
        (Some(gnu), Some(pax)) if gnu != pax => {
            Err(invalid("conflicting GNU and PAX name metadata"))
        }
        (Some(name), _) | (_, Some(name)) => Ok(name),
        (None, None) => Ok(header),
    }
}

fn validate_header_names(header: &::tar::Header) -> io::Result<()> {
    let bytes = header.as_bytes();
    let mut fields = vec![&bytes[..100], &bytes[157..257]];
    if header.as_ustar().is_some() {
        fields.push(&bytes[345..500]);
    }
    for field in fields {
        if let Some(terminator) = field.iter().position(|byte| *byte == 0)
            && field[terminator..].iter().any(|byte| *byte != 0)
        {
            return Err(invalid("tar name contains data after a NUL terminator"));
        }
    }
    Ok(())
}

fn parse_pax(bytes: &[u8]) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut fields = BTreeMap::new();
    for field in ::tar::PaxExtensions::new(bytes) {
        let field = field?;
        let key = field.key().map_err(|_| invalid("PAX key is not UTF-8"))?;
        if key.starts_with("GNU.sparse") || matches!(key, "SCHILY.filetype" | "SCHILY.realsize") {
            return Err(invalid("unsupported sparse or special PAX entry"));
        }
        if !matches!(key, "path" | "linkpath" | "size") {
            continue;
        }
        if fields
            .insert(key.to_owned(), field.value_bytes().to_vec())
            .is_some()
        {
            return Err(invalid("duplicate PAX field"));
        }
    }
    Ok(fields)
}

struct TarReader {
    decoder: MultiGzDecoder<File>,
    total: u64,
    last_block: [u8; 512],
    limit: u64,
}

impl Read for TarReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.decoder.read(buffer)?;
        self.total = self
            .total
            .checked_add(count as u64)
            .filter(|total| *total <= self.limit)
            .ok_or_else(|| invalid("tar stream exceeds decompression limit"))?;
        if count >= 512 {
            self.last_block.copy_from_slice(&buffer[count - 512..count]);
        } else if count != 0 {
            self.last_block.rotate_left(count);
            self.last_block[512 - count..].copy_from_slice(&buffer[..count]);
        }
        Ok(count)
    }
}

impl Extraction<'_> {
    fn queue_link(&mut self, relative: &str, target: &str) -> io::Result<()> {
        if self.entries.contains_key(relative) {
            return Err(invalid("duplicate archive link path"));
        }
        normalize_link_target(relative, target)?;
        self.create_parent_directories(relative)?;
        self.entries
            .insert(relative.to_owned(), EntryKind::Link(target.to_owned()));
        Ok(())
    }

    fn resolve_link(&self, relative: &str) -> io::Result<String> {
        let mut path = relative.to_owned();
        let mut visited = BTreeSet::new();
        loop {
            let parts: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
            let mut replaced = false;
            for index in 0..parts.len() {
                let prefix = parts[..=index].join("/");
                if let Some(EntryKind::Link(target)) = self.entries.get(&prefix) {
                    if !visited.insert(prefix.clone()) {
                        return Err(invalid("archive link cycle"));
                    }
                    let mut resolved = normalize_link_target(&prefix, target)?;
                    for part in &parts[index + 1..] {
                        if !resolved.is_empty() {
                            resolved.push('/');
                        }
                        resolved.push_str(part);
                    }
                    path = resolved;
                    replaced = true;
                    break;
                }
                if index + 1 < parts.len()
                    && !matches!(self.entries.get(&prefix), Some(EntryKind::Directory { .. }))
                {
                    return Err(invalid(
                        "archive link target ancestor is missing or not a directory",
                    ));
                }
            }
            if !replaced {
                break;
            }
        }
        match self.entries.get(&path) {
            Some(EntryKind::File) => {}
            Some(EntryKind::Directory { .. }) => {
                if path.is_empty() || relative.starts_with(&format!("{path}/")) {
                    return Err(invalid("archive directory link creates an ancestor cycle"));
                }
            }
            _ => return Err(invalid("dangling archive link target")),
        }
        Ok(path)
    }

    pub(super) fn create_links(&self) -> io::Result<()> {
        let links: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(path, kind)| {
                if let EntryKind::Link(target) = kind {
                    Some((path, target))
                } else {
                    None
                }
            })
            .collect();
        let mut directory_links = Vec::new();
        for (relative, _) in &links {
            let resolved = self.resolve_link(relative)?;
            let path = self.staging.join(&resolved);
            match self.entries.get(&resolved) {
                Some(EntryKind::Directory { .. }) => {
                    real_directory(self.boundary, &path)?;
                    let parent = relative.rsplit_once('/').map_or("", |(parent, _)| parent);
                    directory_links.push((parent.to_owned(), resolved));
                }
                _ => {
                    super::regular_file(self.boundary, &path)?;
                }
            }
        }
        self.reject_directory_cycles(directory_links)?;
        for (relative, target) in links {
            let path = self.staging.join(relative);
            real_directory(self.boundary, path.parent().unwrap())?;
            fs::symlink_metadata(&path).map_or_else(
                |error| {
                    if error.kind() == io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(error)
                    }
                },
                |_| Err(invalid("archive link destination already exists")),
            )?;
            std::os::unix::fs::symlink(target, path)?;
        }
        Ok(())
    }

    // Include physical directory edges so two links cannot create an indirect directory cycle.
    fn reject_directory_cycles(&self, links: Vec<(String, String)>) -> io::Result<()> {
        let mut graph: BTreeMap<String, Vec<String>> = self
            .entries
            .iter()
            .filter(|(_, kind)| matches!(kind, EntryKind::Directory { .. }))
            .map(|(path, _)| (path.clone(), Vec::new()))
            .collect();
        let mut edges = links;
        for directory in graph.keys().filter(|path| !path.is_empty()) {
            let parent = directory.rsplit_once('/').map_or("", |(parent, _)| parent);
            edges.push((parent.to_owned(), directory.clone()));
        }
        let mut incoming: BTreeMap<String, usize> =
            graph.keys().map(|path| (path.clone(), 0)).collect();
        for (source, target) in edges {
            *incoming.get_mut(&target).unwrap() += 1;
            graph.get_mut(&source).unwrap().push(target);
        }
        let mut queue: VecDeque<_> = incoming
            .iter()
            .filter_map(|(path, count)| (*count == 0).then_some(path.clone()))
            .collect();
        let mut visited = 0;
        while let Some(path) = queue.pop_front() {
            visited += 1;
            for target in &graph[&path] {
                let count = incoming.get_mut(target).unwrap();
                *count -= 1;
                if *count == 0 {
                    queue.push_back(target.clone());
                }
            }
        }
        if visited != graph.len() {
            return Err(invalid("archive directory link cycle"));
        }
        Ok(())
    }
}

fn normalize_link_target(relative: &str, target: &str) -> io::Result<String> {
    if target.is_empty() || target.starts_with('/') {
        return Err(invalid("absolute or empty archive link target"));
    }
    let mut parts: Vec<_> = relative
        .rsplit_once('/')
        .map_or(Vec::new(), |(parent, _)| parent.split('/').collect());
    for part in target.split('/') {
        match part {
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(invalid("archive link escapes installation"));
                }
            }
            part => {
                validate_component(part)?;
                parts.push(part);
            }
        }
    }
    Ok(parts.join("/"))
}
