use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;
use ureq::{Agent, Body};

use crate::download;
use crate::installation::extract_verified_self_update;

const LATEST: &str = "https://api.github.com/repos/verslot/verslot/releases/latest";
const DOWNLOAD_BASE: &str = "https://github.com/verslot/verslot/releases/download";
const ARCHIVE_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct Release {
    pub(super) version: String,
    filename: String,
    archive_url: String,
    checksum_url: String,
}

#[derive(Debug)]
pub(super) enum Selection {
    Current(String),
    Update(Release),
}

pub(super) fn target_for(os: &str, arch: &str, environment: &str) -> Result<&'static str, String> {
    match (os, arch, environment) {
        ("windows", "x86_64", "msvc") => Ok("x86_64-pc-windows-msvc"),
        ("macos", "aarch64", _) => Ok("aarch64-apple-darwin"),
        ("linux", "x86_64", "gnu") => Ok("x86_64-unknown-linux-gnu"),
        _ => Err(
            "no compatible official binary; upgrade manually or through your package manager"
                .to_owned(),
        ),
    }
}

pub(super) fn current_target() -> Result<&'static str, String> {
    let environment = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else {
        ""
    };
    target_for(std::env::consts::OS, std::env::consts::ARCH, environment)
}

pub(super) fn version_parts(version: &str) -> Result<[u32; 3], String> {
    let invalid = || format!("unsupported stable version: {version:?}");
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(invalid());
    }
    Ok([
        parts[0].parse().map_err(|_| invalid())?,
        parts[1].parse().map_err(|_| invalid())?,
        parts[2].parse().map_err(|_| invalid())?,
    ])
}

pub(super) fn latest(agent: &Agent, current: &str, target: &str) -> Result<Selection, String> {
    let bytes = fetch_bytes(agent, LATEST, 1024 * 1024, false)?;
    let metadata: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("read official release metadata: {error}"))?;
    select(&metadata, current, target)
}

pub(super) fn select(metadata: &Value, current: &str, target: &str) -> Result<Selection, String> {
    if metadata["draft"].as_bool() != Some(false) || metadata["prerelease"].as_bool() != Some(false)
    {
        return Err("official release is a draft, prerelease, or has invalid metadata".to_owned());
    }
    let tag = metadata["tag_name"].as_str().ok_or("missing release tag")?;
    let version = tag
        .strip_prefix('v')
        .ok_or("invalid official release tag")?;
    if version_parts(version)? <= version_parts(current)? {
        return Ok(Selection::Current(version.to_owned()));
    }
    let extension = if target.ends_with("windows-msvc") {
        "zip"
    } else {
        "tar.gz"
    };
    let filename = format!("verslot-{tag}-{target}.{extension}");
    let assets = metadata["assets"]
        .as_array()
        .ok_or("missing official release assets")?;
    let asset_url = |name: &str| -> Result<String, String> {
        let mut matches = assets
            .iter()
            .filter(|asset| asset["name"].as_str() == Some(name));
        let asset = matches
            .next()
            .ok_or_else(|| format!("official release lacks {name}"))?;
        let expected = format!("{DOWNLOAD_BASE}/{tag}/{name}");
        if matches.next().is_some()
            || asset["state"].as_str() != Some("uploaded")
            || asset["browser_download_url"].as_str() != Some(expected.as_str())
        {
            return Err(format!(
                "invalid or duplicate official release asset: {name}"
            ));
        }
        Ok(expected)
    };
    Ok(Selection::Update(Release {
        version: version.to_owned(),
        archive_url: asset_url(&filename)?,
        checksum_url: asset_url(&format!("{filename}.sha256"))?,
        filename,
    }))
}

pub(super) fn prepare(
    agent: &Agent,
    release: &Release,
    root: &Path,
    operation: &Path,
) -> Result<(), String> {
    let checksums = fetch_bytes(agent, &release.checksum_url, 4096, true)?;
    let text = std::str::from_utf8(&checksums).map_err(|_| "release checksum is not UTF-8")?;
    let expected = download::parse_checksum(text, &release.filename)?;
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    let mut response = fetch_response(agent, &release.archive_url, true, deadline)?;
    let archive_path = operation.join("release.archive");
    let mut archive = File::create_new(&archive_path).map_err(|error| error.to_string())?;
    let digest = download::copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut archive,
        ARCHIVE_LIMIT,
        deadline,
    )?;
    if digest != expected {
        return Err(format!(
            "release SHA-256 mismatch (expected {expected}, received {digest})"
        ));
    }
    archive.sync_all().map_err(|error| error.to_string())?;
    drop(archive);
    extract_verified_self_update(root, &archive_path, &operation.join("candidate"))
        .map_err(|error| format!("extract official release: {error}"))?;
    validate_binary(
        &operation
            .join("candidate")
            .join(crate::shim::executable_name("verslot")),
        current_target()?,
    )
}

fn fetch_bytes(agent: &Agent, url: &str, limit: u64, redirects: bool) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut response = fetch_response(agent, url, redirects, deadline)?;
    let mut bytes = Vec::new();
    download::copy_bounded(
        &mut response.body_mut().as_reader(),
        &mut bytes,
        limit,
        deadline,
    )?;
    Ok(bytes)
}

fn approved_url(url: &str) -> bool {
    let Ok(uri) = url.parse::<ureq::http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("https")
        && uri.authority().is_some_and(|authority| {
            matches!(
                authority.as_str(),
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        })
}

fn fetch_response(
    agent: &Agent,
    url: &str,
    redirects: bool,
    deadline: Instant,
) -> Result<ureq::http::Response<Body>, String> {
    let mut url = url.to_owned();
    for attempt in 0..=3 {
        if !approved_url(&url) {
            return Err("refused release download outside official HTTPS hosts".to_owned());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("release download timed out")?;
        let response = agent
            .get(&url)
            .header("User-Agent", concat!("verslot/", env!("CARGO_PKG_VERSION")))
            .header("Accept-Encoding", "identity")
            .config()
            .timeout_global(Some(remaining))
            .build()
            .call()
            .map_err(|error| format!("fetch official release: {error}"))?;
        match response.status().as_u16() {
            200 => return Ok(response),
            301 | 302 | 303 | 307 | 308 if redirects && attempt < 3 => {
                url = response.headers().get("location").and_then(|header| header.to_str().ok())
                    .ok_or("invalid release download redirect")?.to_owned();
            }
            404 => return Err("no published stable release or compatible artifact is available; upgrade manually or through your package manager".to_owned()),
            status => return Err(format!("fetch official release: expected HTTP 200, received {status}")),
        }
    }
    Err("too many release download redirects".to_owned())
}

// Check the file format and architecture before executing any verified release content.
pub(super) fn validate_binary(path: &Path, target: &str) -> Result<(), String> {
    let invalid = || "release executable does not match the supported target".to_owned();
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut header = [0; 64];
    file.read_exact(&mut header).map_err(|_| invalid())?;
    let compatible = match target {
        "x86_64-pc-windows-msvc" if &header[..2] == b"MZ" => {
            let offset = u32::from_le_bytes([header[60], header[61], header[62], header[63]]);
            file.seek(SeekFrom::Start(u64::from(offset)))
                .map_err(|_| invalid())?;
            let mut pe = [0; 26];
            file.read_exact(&mut pe).map_err(|_| invalid())?;
            &pe[..6] == b"PE\0\0\x64\x86" && pe[24..26] == [0x0b, 0x02]
        }
        "aarch64-apple-darwin" => header[..8] == [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1],
        "x86_64-unknown-linux-gnu" => {
            &header[..7] == b"\x7fELF\x02\x01\x01" && header[18..20] == [62, 0]
        }
        _ => false,
    };
    if compatible { Ok(()) } else { Err(invalid()) }
}
