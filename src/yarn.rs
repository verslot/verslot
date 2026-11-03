use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;
use ureq::Agent;

use crate::download::{self, ARCHIVE_LIMIT};
use crate::installation::{real_directory, regular_file};
use crate::progress::Progress;
use crate::target::{Target, Tool, Version};

const REGISTRY: &str = "https://registry.npmjs.org";
const INDEX_LIMIT: u64 = 16 * 1024 * 1024;
const METADATA_LIMIT: u64 = 1024 * 1024;
const RECEIPT: &str = ".verslot-install";

fn package_name(version: Version) -> &'static str {
    if version.major == 1 {
        "yarn"
    } else {
        "@yarnpkg/cli-dist"
    }
}

fn package_path(version: Version) -> &'static str {
    if version.major == 1 {
        "yarn"
    } else {
        "@yarnpkg%2fcli-dist"
    }
}

pub(crate) fn archive_filename(version: Version) -> String {
    let name = if version.major == 1 {
        "yarn"
    } else {
        "cli-dist"
    };
    format!("{name}-{version}.tgz")
}

fn archive_url(version: Version) -> String {
    format!(
        "{REGISTRY}/{}/-/{}",
        package_name(version),
        archive_filename(version)
    )
}

pub(crate) fn validate_version(version: Version) -> Result<(), String> {
    if version.major == 0 {
        return Err("unsupported Yarn version: expected Classic 1.x or modern 2+".to_owned());
    }
    Ok(())
}

fn fetch_json(agent: &Agent, url: &str, limit: u64, deadline: Instant) -> Result<Value, String> {
    let mut response = download::fetch_response(agent, url, deadline)?;
    let mut bytes = Vec::new();
    download::copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut bytes,
        limit,
        deadline,
    )?;
    serde_json::from_slice(&bytes).map_err(|error| format!("read Yarn metadata: {error}"))
}

fn expected_digest(metadata: &Value, version: Version) -> Result<String, String> {
    if metadata["name"].as_str() != Some(package_name(version))
        || metadata["version"].as_str() != Some(version.to_string().as_str())
        || metadata["dist"]["tarball"].as_str() != Some(archive_url(version).as_str())
    {
        return Err(format!(
            "Yarn package name, version or official archive URL does not match yarn@{version}"
        ));
    }
    let integrity = metadata["dist"]["integrity"]
        .as_str()
        .and_then(|text| text.strip_prefix("sha512-"))
        .ok_or("Yarn package has no SHA-512 integrity metadata")?;
    let digest = STANDARD
        .decode(integrity)
        .map_err(|_| "Yarn package has invalid SHA-512 integrity metadata")?;
    if digest.len() != 64 {
        return Err("Yarn package SHA-512 digest must contain 64 bytes".to_owned());
    }
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn parse_versions(metadata: &Value, classic: bool) -> Result<Vec<Version>, String> {
    let name = if classic { "yarn" } else { "@yarnpkg/cli-dist" };
    if metadata["name"].as_str() != Some(name) {
        return Err("Yarn release index has an unexpected package name".to_owned());
    }
    let releases = metadata["versions"]
        .as_object()
        .ok_or("Yarn release index has no versions object")?;
    let mut versions = Vec::new();
    for (text, release) in releases {
        let Ok(target) = format!("yarn@{text}").parse::<Target>() else {
            continue;
        };
        let version = target.version;
        if (classic && version.major != 1) || (!classic && version.major < 2) {
            continue;
        }
        if expected_digest(release, version).is_ok() {
            versions.push(version);
        }
    }
    Ok(versions)
}

pub(crate) fn available_versions() -> Result<Vec<Version>, String> {
    fetch_versions(
        &download::download_agent(),
        &[
            format!("{REGISTRY}/yarn"),
            format!("{REGISTRY}/@yarnpkg%2fcli-dist"),
        ],
        Instant::now() + Duration::from_secs(60),
    )
}

fn fetch_versions(
    agent: &Agent,
    urls: &[String; 2],
    deadline: Instant,
) -> Result<Vec<Version>, String> {
    let mut versions = Vec::new();
    for (index, url) in urls.iter().enumerate() {
        versions.extend(parse_versions(
            &fetch_json(agent, url, INDEX_LIMIT, deadline)?,
            index == 0,
        )?);
    }
    versions.sort_unstable_by_key(|version| (version.major, version.minor, version.patch));
    versions.dedup();
    Ok(versions)
}

pub(crate) fn download_archive(
    version: Version,
    archive: &Path,
    progress: &mut Progress<'_>,
) -> Result<String, String> {
    validate_version(version)?;
    download_with(
        &download::download_agent(),
        version,
        &format!("{REGISTRY}/{}/{version}", package_path(version)),
        &archive_url(version),
        archive,
        progress,
    )
}

fn download_with(
    agent: &Agent,
    version: Version,
    metadata_url: &str,
    url: &str,
    archive: &Path,
    progress: &mut Progress<'_>,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    progress.stage("Fetching checksums");
    let metadata = fetch_json(agent, metadata_url, METADATA_LIMIT, deadline)?;
    let expected = expected_digest(&metadata, version)?;
    progress.stage("Downloading");
    let mut response = download::fetch_response(agent, url, deadline)?;
    let mut output =
        File::create_new(archive).map_err(|error| format!("create Yarn archive: {error}"))?;
    progress.set_download_total(download::archive_display_total(response.headers()));
    let digest = download::copy_sha512_with_progress(
        &mut response.body_mut().as_reader(),
        &mut output,
        ARCHIVE_LIMIT,
        deadline,
        progress,
    )?;
    progress.stage("Verifying");
    if digest != expected {
        return Err(format!(
            "verify Yarn archive: SHA-512 mismatch (expected {expected}, received {digest})"
        ));
    }
    Ok(digest)
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn validate_digest(digest: &str) -> io::Result<()> {
    if digest.len() != 128
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "Yarn receipt SHA-512 must contain 128 lowercase hexadecimal digits",
        ));
    }
    Ok(())
}

fn validate_payload(root: &Path, directory: &Path, version: Version) -> io::Result<()> {
    real_directory(root, directory)?;
    if regular_file(root, &directory.join("bin/yarn.js"))?.len() == 0 {
        return Err(invalid("Yarn JavaScript entry point is empty"));
    }
    let package = directory.join("package.json");
    regular_file(root, &package)?;
    let mut bytes = Vec::new();
    File::open(package)?
        .take(METADATA_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err(invalid("Yarn package metadata exceeds byte limit"));
    }
    let metadata: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("invalid Yarn package metadata: {error}")))?;
    if metadata["name"].as_str() != Some(package_name(version))
        || metadata["version"].as_str() != Some(version.to_string().as_str())
    {
        return Err(invalid(
            "installed Yarn package name or version does not match its target",
        ));
    }
    Ok(())
}

pub(crate) fn extract_archive(
    root: &Path,
    archive: &Path,
    staging: &Path,
    version: Version,
    digest: &str,
) -> io::Result<()> {
    validate_version(version).map_err(invalid)?;
    validate_digest(digest)?;
    crate::installation::extract_verified_package(root, archive, staging)?;
    validate_payload(root, staging, version)?;
    let text = format!(
        "verslot-install-v2\nyarn@{version}\n{}\n{digest}\n",
        archive_filename(version)
    );
    let mut receipt = File::create_new(staging.join(RECEIPT))?;
    receipt.write_all(text.as_bytes())?;
    receipt.flush()
}

pub(crate) fn validate_directory(root: &Path, directory: &Path, target: &Target) -> io::Result<()> {
    validate_version(target.version).map_err(invalid)?;
    validate_payload(root, directory, target.version)?;
    let receipt = directory.join(RECEIPT);
    regular_file(root, &receipt)?;
    let mut bytes = Vec::new();
    File::open(receipt)?.take(1025).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 {
        return Err(invalid("installation receipt exceeds 1024-byte limit"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("receipt is not UTF-8"))?;
    let fields: Vec<_> = text.split('\n').collect();
    if target.tool != Tool::Yarn
        || fields.len() != 5
        || fields[0] != "verslot-install-v2"
        || fields[1] != target.to_string()
        || fields[2] != archive_filename(target.version)
        || !fields[4].is_empty()
    {
        return Err(invalid(
            "Yarn receipt format, target or archive does not match",
        ));
    }
    validate_digest(fields[3])
}

#[cfg(test)]
pub(crate) mod tests;
