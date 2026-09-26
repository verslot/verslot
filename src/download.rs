use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport,
};
use ureq::{Agent, Body, Error, Timeout};

use crate::distribution::NodeDistribution;
use crate::target::Version;

const CHECKSUM_LIMIT: u64 = 1024 * 1024;
const ARCHIVE_LIMIT: u64 = 512 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Downloads into a new archive file beneath the caller's private operation directory.
/// The caller must validate mutation boundaries and clean up this file on failure (T4).
/// Returns the verified lowercase digest only after the entire response has been read.
pub fn download_verified_archive(version: Version, archive_path: &Path) -> Result<String, String> {
    let distribution = NodeDistribution::for_current_build(version)?;
    let agent = download_agent();
    download_distribution(&agent, &distribution, archive_path)
}

fn download_agent() -> Agent {
    let config = Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_resolve(Some(CONNECT_TIMEOUT))
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_send_request(Some(READ_TIMEOUT))
        .timeout_recv_response(Some(READ_TIMEOUT))
        .build();
    Agent::with_parts(config, ReadTimeoutConnector, DefaultResolver::default())
}

fn download_distribution(
    agent: &Agent,
    distribution: &NodeDistribution,
    archive_path: &Path,
) -> Result<String, String> {
    let deadline = Instant::now() + DOWNLOAD_TIMEOUT;
    let mut checksums = fetch_response(agent, &distribution.checksum_url, deadline)?;
    let mut checksum_bytes = Vec::new();
    copy_bounded(
        &mut checksums.body_mut().as_reader(),
        &mut checksum_bytes,
        CHECKSUM_LIMIT,
        deadline,
    )
    .map_err(|error| format!("download checksums {}: {error}", distribution.checksum_url))?;
    let checksum_text = std::str::from_utf8(&checksum_bytes)
        .map_err(|error| format!("read checksums: invalid UTF-8: {error}"))?;
    let expected = parse_checksum(checksum_text, &distribution.archive_filename)?;

    let mut response = fetch_response(agent, &distribution.archive_url, deadline)?;
    let mut archive = File::create_new(archive_path)
        .map_err(|error| format!("create archive {}: {error}", archive_path.display()))?;
    let digest = copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut archive,
        ARCHIVE_LIMIT,
        deadline,
    )
    .map_err(|error| format!("download archive {}: {error}", distribution.archive_url))?;
    if digest != expected {
        return Err(format!(
            "verify archive {}: SHA-256 mismatch (expected {expected}, received {digest})",
            distribution.archive_filename
        ));
    }
    Ok(digest)
}

fn fetch_response(
    agent: &Agent,
    url: &str,
    deadline: Instant,
) -> Result<ureq::http::Response<Body>, String> {
    let remaining = remaining_time(deadline)?;
    let response = agent
        .get(url)
        .header("Accept-Encoding", "identity")
        .config()
        .timeout_global(Some(remaining))
        .build()
        .call()
        .map_err(|error| format!("download {url}: transport failure: {error}"))?;
    remaining_time(deadline)?;
    match response.status().as_u16() {
        200 => Ok(response),
        status @ (404 | 410) => Err(format!(
            "download {url}: release or artifact unavailable (HTTP {status})"
        )),
        status => Err(format!(
            "download {url}: expected HTTP 200, received {status}"
        )),
    }
}

fn parse_checksum(text: &str, archive_filename: &str) -> Result<String, String> {
    let mut matched = None;
    for line in text.lines() {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        if !fields.contains(&archive_filename) {
            continue;
        }
        if matched.is_some() {
            return Err(format!(
                "verify checksums: duplicate entry for {archive_filename}"
            ));
        }
        if fields.len() != 2
            || fields[1] != archive_filename
            || fields[0].len() != 64
            || !fields[0].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "verify checksums: malformed entry for {archive_filename}"
            ));
        }
        matched = Some(fields[0].to_ascii_lowercase());
    }
    matched.ok_or_else(|| format!("verify checksums: missing entry for {archive_filename}"))
}

fn remaining_time(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| "download deadline exceeded (15 minutes)".to_owned())
}

fn copy_bounded(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limit: u64,
    deadline: Instant,
) -> Result<String, String> {
    let mut buffer = [0; 64 * 1024];
    let mut total = 0_u64;
    let mut digest = Sha256::new();
    loop {
        remaining_time(deadline)?;
        let count = match reader.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("read response: {error}")),
        };
        remaining_time(deadline)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|total| *total <= limit)
            .ok_or_else(|| format!("response exceeds {limit}-byte limit"))?;
        writer
            .write_all(&buffer[..count])
            .map_err(|error| format!("write download: {error}"))?;
        digest.update(&buffer[..count]);
    }
    writer
        .flush()
        .map_err(|error| format!("flush download: {error}"))?;
    remaining_time(deadline)?;
    let hex_digits = b"0123456789abcdef";
    let mut hexadecimal = String::with_capacity(64);
    for byte in digest.finalize() {
        hexadecimal.push(char::from(hex_digits[(byte >> 4) as usize]));
        hexadecimal.push(char::from(hex_digits[(byte & 0x0f) as usize]));
    }
    Ok(hexadecimal)
}

// ureq's body timeout covers the entire body, so cap each transport read separately.
#[derive(Debug)]
struct ReadTimeoutConnector;

impl Connector for ReadTimeoutConnector {
    type Out = ReadTimeoutTransport;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<()>,
    ) -> Result<Option<Self::Out>, Error> {
        Ok(DefaultConnector::default()
            .connect(details, chained)?
            .map(ReadTimeoutTransport))
    }
}

#[derive(Debug)]
struct ReadTimeoutTransport(Box<dyn Transport>);

impl Transport for ReadTimeoutTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.0.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), Error> {
        self.0.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, mut timeout: NextTimeout) -> Result<bool, Error> {
        if *timeout.after > READ_TIMEOUT {
            timeout.after = READ_TIMEOUT.into();
            timeout.reason = Timeout::RecvBody;
        }
        self.0.await_input(timeout)
    }

    fn is_open(&mut self) -> bool {
        self.0.is_open()
    }

    fn is_tls(&self) -> bool {
        self.0.is_tls()
    }
}

#[cfg(test)]
mod tests;
