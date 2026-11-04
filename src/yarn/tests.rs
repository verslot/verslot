use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use sha2::{Digest, Sha512};

use super::*;
use crate::download::tests::{local_agent, response, serve};

pub(crate) fn archive_fixture(version: Version) -> Vec<u8> {
    let metadata =
        json!({"name": package_name(version), "version": version.to_string()}).to_string();
    let mut archive = tar::Builder::new(Vec::new());
    for (name, contents) in [
        ("package/package.json", metadata.as_bytes()),
        ("package/bin/yarn.js", b"offline Yarn".as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(contents.len() as u64);
        header.set_cksum();
        archive
            .append_data(&mut header, name, Cursor::new(contents))
            .unwrap();
    }
    gzip(&archive.into_inner().unwrap())
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn metadata(version: Version, bytes: &[u8]) -> Value {
    json!({"name": package_name(version), "version": version.to_string(), "dist": {
        "tarball": archive_url(version), "integrity": format!("sha512-{}", STANDARD.encode(Sha512::digest(bytes)))
    }})
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "verslot-yarn-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn remote_versions_merge_only_supported_stable_packages_with_sha512() {
    let classic = "yarn@1.22.22".parse::<Target>().unwrap().version;
    let modern = "yarn@4.9.2".parse::<Target>().unwrap().version;
    let old = "yarn@2.4.1".parse::<Target>().unwrap().version;
    let mut missing_integrity = metadata(classic, b"archive");
    missing_integrity["version"] = json!("1.0.0");
    missing_integrity["dist"]["integrity"] = Value::Null;
    let indexes = [
        json!({"name":"yarn", "versions": {"1.22.22":metadata(classic,b"archive"), "1.0.0":missing_integrity,
            "2.4.3":metadata(old,b"archive"), "1.23.0-rc.1":{}}}),
        json!({"name":"@yarnpkg/cli-dist", "versions": {"4.9.2":metadata(modern,b"archive"),
            "2.4.1":metadata(old,b"archive"), "04.9.2":{}, "4.10.0+build":{}}}),
    ];
    let (url, server) = serve(
        indexes
            .iter()
            .map(|index| response(index.to_string().as_bytes()))
            .collect(),
    );
    let versions = fetch_versions(
        &local_agent(),
        &[url.clone(), url],
        Instant::now() + Duration::from_secs(5),
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(versions, vec![classic, old, modern]);
}

#[test]
fn integrity_metadata_must_bind_the_exact_official_package_and_sha512() {
    let version = "yarn@4.9.2".parse::<Target>().unwrap().version;
    for field in [
        "name", "version", "url", "missing", "sha1", "invalid", "short",
    ] {
        let mut value = metadata(version, b"archive");
        match field {
            "name" => value["name"] = json!("other"),
            "version" => value["version"] = json!("4.9.3"),
            "url" => value["dist"]["tarball"] = json!("https://example.com/payload.tgz"),
            "missing" => value["dist"]["integrity"] = Value::Null,
            "sha1" => value["dist"]["integrity"] = json!("sha1-aabbcc"),
            "invalid" => value["dist"]["integrity"] = json!("sha512-!!!!"),
            "short" => value["dist"]["integrity"] = json!("sha512-YQ=="),
            _ => unreachable!(),
        }
        assert!(expected_digest(&value, version).is_err(), "{field}");
    }
}

#[test]
fn both_generations_download_verify_and_extract_without_running_node() {
    for input in ["yarn@1.22.22", "yarn@4.9.2"] {
        let target: Target = input.parse().unwrap();
        let fixture = Fixture::new();
        let bytes = archive_fixture(target.version);
        let meta = metadata(target.version, &bytes).to_string();
        let (url, server) = serve(vec![response(meta.as_bytes()), response(&bytes)]);
        let archive = fixture.0.join("archive");
        let digest = download_with(
            &local_agent(),
            target.version,
            &url,
            &url,
            &archive,
            &mut Progress::Silent,
        )
        .unwrap();
        server.join().unwrap();
        let staging = fixture.0.join("staging");
        extract_archive(&fixture.0, &archive, &staging, target.version, &digest).unwrap();
        validate_directory(&fixture.0, &staging, &target).unwrap();
        assert_eq!(
            fs::read(staging.join("bin/yarn.js")).unwrap(),
            b"offline Yarn"
        );
        assert!(
            fs::read_to_string(staging.join(RECEIPT))
                .unwrap()
                .contains(&digest)
        );
    }
}

#[test]
fn changed_archive_bytes_fail_sha512_before_extraction() {
    let version = "yarn@4.9.2".parse::<Target>().unwrap().version;
    let fixture = Fixture::new();
    let bytes = archive_fixture(version);
    let meta = metadata(version, &bytes).to_string();
    let (url, server) = serve(vec![
        response(meta.as_bytes()),
        response(b"tampered archive"),
    ]);
    let error = download_with(
        &local_agent(),
        version,
        &url,
        &url,
        &fixture.0.join("archive"),
        &mut Progress::Silent,
    )
    .unwrap_err();
    server.join().unwrap();
    assert!(error.contains("SHA-512 mismatch"));
    assert!(!fixture.0.join("staging").exists());
}

#[test]
fn npm_archives_reject_traversal_and_all_link_types_on_every_platform() {
    for (name, kind) in [
        ("package/../escape", tar::EntryType::Regular),
        ("package/link", tar::EntryType::Symlink),
        ("package/link", tar::EntryType::Link),
    ] {
        let fixture = Fixture::new();
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(0);
        header.set_entry_type(kind);
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name("bin/yarn.js").unwrap();
        }
        header.set_cksum();
        let mut archive = tar::Builder::new(Vec::new());
        archive.append(&header, io::empty()).unwrap();
        let path = fixture.0.join("archive");
        fs::write(&path, gzip(&archive.into_inner().unwrap())).unwrap();
        assert!(
            crate::installation::extract_verified_package(
                &fixture.0,
                &path,
                &fixture.0.join("staging")
            )
            .is_err(),
            "{name}"
        );
        assert!(!fixture.0.join("escape").exists());
    }
}

#[test]
fn installed_package_and_receipt_must_match_the_requested_target() {
    let fixture = Fixture::new();
    let target: Target = "yarn@4.9.2".parse().unwrap();
    let archive = fixture.0.join("archive");
    fs::write(&archive, archive_fixture(target.version)).unwrap();
    let staging = fixture.0.join("staging");
    extract_archive(
        &fixture.0,
        &archive,
        &staging,
        target.version,
        &"a".repeat(128),
    )
    .unwrap();
    fs::write(
        staging.join("package.json"),
        r#"{"name":"@yarnpkg/cli-dist","version":"4.9.3"}"#,
    )
    .unwrap();
    assert!(validate_directory(&fixture.0, &staging, &target).is_err());
}
