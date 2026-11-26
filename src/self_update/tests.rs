use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
};

use super::*;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "verslot-update-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root: fs::canonicalize(root).unwrap(),
        }
    }

    fn operation(&self) -> PathBuf {
        let operation = allocate_operation(&self.root).unwrap();
        write_new(&operation.join(MARKER), MARKER_CONTENT.as_bytes()).unwrap();
        write_new(&operation.join("active.lock"), b"").unwrap();
        operation
    }

    fn install(&self) -> PathBuf {
        let source = self.root.join("source");
        fs::write(&source, b"old executable").unwrap();
        let _lock = acquire_exclusive_lock(&self.root).unwrap();
        setup::setup_locked(&self.root, &source).unwrap();
        self.root.join("shims").join(executable_name("verslot"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(
            self.root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn metadata(version: &str, target: &str) -> Value {
    let extension = if target.ends_with("windows-msvc") {
        "zip"
    } else {
        "tar.gz"
    };
    let name = format!("verslot-v{version}-{target}.{extension}");
    json!({"tag_name": format!("v{version}"), "draft": false, "prerelease": false, "assets": [
        {"name": name, "state": "uploaded", "browser_download_url": format!("https://github.com/verslot/verslot/releases/download/v{version}/{name}")},
        {"name": format!("{name}.sha256"), "state": "uploaded", "browser_download_url": format!("https://github.com/verslot/verslot/releases/download/v{version}/{name}.sha256")}
    ]})
}

fn selected_release() -> release::Release {
    let release::Selection::Update(release) = release::select(
        &metadata("1.0.0", release::current_target().unwrap()),
        "0.8.0",
        release::current_target().unwrap(),
    )
    .unwrap() else {
        panic!("expected update")
    };
    release
}

fn filename() -> String {
    let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!(
        "verslot-v1.0.0-{}.{extension}",
        release::current_target().unwrap()
    )
}

fn native_header(target: &str) -> Vec<u8> {
    let mut bytes = vec![0; 90];
    match target {
        "x86_64-pc-windows-msvc" => {
            bytes[..2].copy_from_slice(b"MZ");
            bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
            bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
            bytes[88..90].copy_from_slice(&[0x0b, 0x02]);
        }
        "aarch64-apple-darwin" => {
            bytes[..8].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1])
        }
        "x86_64-unknown-linux-gnu" => {
            bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            bytes[18..20].copy_from_slice(&[62, 0]);
        }
        _ => panic!("unsupported fixture target"),
    }
    bytes
}

#[test]
fn stable_selection_compares_versions_numerically_and_never_downgrades() {
    let target = "x86_64-pc-windows-msvc";
    assert!(matches!(
        release::select(&metadata("0.10.0", target), "0.9.9", target).unwrap(),
        release::Selection::Update(_)
    ));
    for current in ["0.10.0", "1.0.0"] {
        let mut value = metadata("0.10.0", target);
        value["assets"] = json!([]);
        assert!(
            matches!(release::select(&value, current, target).unwrap(), release::Selection::Current(version) if version == "0.10.0")
        );
    }
}

#[test]
fn release_selection_rejects_drafts_prereleases_and_unsafe_tags() {
    let target = "x86_64-pc-windows-msvc";
    for field in ["draft", "prerelease"] {
        let mut value = metadata("1.0.0", target);
        value[field] = json!(true);
        assert!(release::select(&value, "0.8.0", target).is_err());
    }
    for version in [
        "1.0.0-rc.1",
        "1.0.0+build",
        "1.0.0/../../evil",
        "01.0.0",
        "1.0",
        "4294967296.0.0",
    ] {
        assert!(
            release::select(&metadata(version, target), "0.8.0", target).is_err(),
            "{version}"
        );
    }
}

#[test]
fn release_selection_requires_unique_matching_official_assets_and_checksums() {
    let target = "x86_64-pc-windows-msvc";
    let base = metadata("1.0.0", target);
    let mut wrong_url = base.clone();
    wrong_url["assets"][0]["browser_download_url"] = json!("https://example.com/verslot.exe");
    let mut duplicate = base.clone();
    duplicate["assets"]
        .as_array_mut()
        .unwrap()
        .push(base["assets"][0].clone());
    let mut missing_checksum = base.clone();
    missing_checksum["assets"].as_array_mut().unwrap().pop();
    let mut incomplete = base.clone();
    incomplete["assets"][0]["state"] = json!("new");
    for value in [
        wrong_url,
        duplicate,
        missing_checksum,
        incomplete,
        metadata("1.0.0", "aarch64-apple-darwin"),
    ] {
        assert!(release::select(&value, "0.8.0", target).is_err());
    }
}

#[test]
fn supported_platforms_require_the_official_target_and_binary_architecture() {
    let fixture = Fixture::new();
    for (os, arch, environment, target) in [
        ("windows", "x86_64", "msvc", "x86_64-pc-windows-msvc"),
        ("macos", "aarch64", "", "aarch64-apple-darwin"),
        ("linux", "x86_64", "gnu", "x86_64-unknown-linux-gnu"),
    ] {
        assert_eq!(release::target_for(os, arch, environment).unwrap(), target);
        let executable = fixture.root.join(target);
        fs::write(&executable, native_header(target)).unwrap();
        release::validate_binary(&executable, target).unwrap();
        fs::write(&executable, b"#!/bin/sh\necho unsafe\n").unwrap();
        assert!(release::validate_binary(&executable, target).is_err());
    }
    for (os, arch, environment) in [
        ("linux", "x86_64", "musl"),
        ("windows", "aarch64", "msvc"),
        ("macos", "x86_64", ""),
    ] {
        assert!(release::target_for(os, arch, environment).is_err());
    }
}

#[test]
fn ownership_rejects_external_copies_and_modified_generations() {
    let fixture = Fixture::new();
    let executable = fixture.install();
    owned_receipt(&fixture.root, &executable).unwrap();
    let external = fixture.root.join("downloaded-verslot");
    fs::copy(&executable, &external).unwrap();
    assert!(owned_receipt(&fixture.root, &external).is_err());
    fs::write(&executable, b"modified executable").unwrap();
    assert!(owned_receipt(&fixture.root, &executable).is_err());
    assert_eq!(fs::read(external).unwrap(), b"old executable");
}

#[test]
fn active_handoff_is_not_cleaned_and_residue_is_removed_after_lease_release() {
    let fixture = Fixture::new();
    let operation = fixture.operation();
    fs::create_dir(operation.join("candidate")).unwrap();
    fs::write(candidate_executable(&operation), b"candidate").unwrap();
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .open(operation.join("active.lock"))
        .unwrap();
    lease.try_lock_shared().unwrap();
    assert!(cleanup_pending(&fixture.root).unwrap().is_none());
    assert!(operation.exists());
    drop(lease);
    assert!(cleanup_pending(&fixture.root).unwrap().is_none());
    assert!(!operation.exists());
}

#[test]
fn cleanup_preserves_unknown_files_invalid_markers_and_unrelated_operations() {
    let fixture = Fixture::new();
    let operation = fixture.operation();
    fs::write(operation.join("notes"), b"preserve").unwrap();
    let unrelated = allocate_operation(&fixture.root).unwrap();
    fs::write(unrelated.join("sentinel"), b"preserve").unwrap();
    assert!(
        cleanup_pending(&fixture.root)
            .unwrap()
            .unwrap()
            .contains("unknown entry")
    );
    assert_eq!(fs::read(operation.join("notes")).unwrap(), b"preserve");
    assert_eq!(fs::read(unrelated.join("sentinel")).unwrap(), b"preserve");
    fs::remove_file(operation.join("notes")).unwrap();
    fs::write(operation.join(MARKER), "invalid marker").unwrap();
    assert!(cleanup_operation(&fixture.root, &operation).is_err());
    assert!(cleanup_operation(&fixture.root, &fixture.root).is_err());
}

#[test]
fn cleanup_never_follows_directory_links() {
    let fixture = Fixture::new();
    let operation = fixture.operation();
    let outside = fixture.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserve").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, operation.join("candidate")).unwrap();
    #[cfg(windows)]
    junction::create(&outside, operation.join("candidate")).unwrap();
    assert!(cleanup_operation(&fixture.root, &operation).is_err());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserve");
    #[cfg(unix)]
    fs::remove_file(operation.join("candidate")).unwrap();
    #[cfg(windows)]
    junction::delete(operation.join("candidate")).unwrap();
}

#[test]
fn preparation_failure_cleans_temporary_files_and_preserves_the_installation() {
    let fixture = Fixture::new();
    let executable = fixture.install();
    let operation = fixture.operation();
    fs::write(operation.join("release.archive"), b"partial").unwrap();
    let error = preparation_error(
        &fixture.root,
        &operation,
        io::Error::other("failed download"),
    );
    assert!(error.contains("existing installation preserved"));
    assert!(!operation.exists());
    assert_eq!(fs::read(executable).unwrap(), b"old executable");
}

#[test]
fn updater_process_fixture() {
    let Some(root) = std::env::var_os("VERSLOT_SELF_UPDATE_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut signal = [0; HANDOFF.len()];
    io::stdin().read_exact(&mut signal).unwrap();
    assert_eq!(signal, HANDOFF);
    let _lock = acquire_exclusive_lock(&root).unwrap();
    fs::write(root.join("child-started"), b"ready").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !root.join("child-release").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "observer never released fixture child"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let code = std::env::var("VERSLOT_SELF_UPDATE_TEST_EXIT")
        .unwrap()
        .parse()
        .unwrap();
    std::process::exit(code);
}

#[test]
fn status_observer_releases_the_mutation_lock_waits_and_keeps_the_installer_exit_code() {
    for code in [0, 23] {
        let fixture = Fixture::new();
        let operation = fixture.operation();
        let lock = acquire_exclusive_lock(&fixture.root).unwrap();
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(operation.join("active.lock"))
            .unwrap();
        lease.try_lock_shared().unwrap();
        let root = fixture.root.clone();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "self_update::tests::updater_process_fixture",
                "--nocapture",
            ])
            .env("VERSLOT_SELF_UPDATE_TEST_ROOT", &root)
            .env("VERSLOT_SELF_UPDATE_TEST_EXIT", code.to_string())
            .stdout(Stdio::null());
        let (sent, received) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sent.send(wait_for_installer(
                &root,
                &operation,
                lock,
                lease,
                &mut command,
            ))
            .unwrap();
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !fixture.root.join("child-started").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "installer did not acquire the released mutation lock"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(
            received.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        fs::write(fixture.root.join("child-release"), b"continue").unwrap();
        let status = received
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert_eq!(status.code(), Some(code));
        assert_eq!(fs::read_dir(fixture.root.join("tmp")).unwrap().count(), 0);
    }
}

// A complete in-memory HTTP transport keeps official URL checks enabled without
// DNS, TLS fixtures, network access, or a production endpoint override.
#[derive(Debug)]
struct OfflineHttp {
    responses: Mutex<VecDeque<Vec<u8>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Resolver for OfflineHttp {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut addresses = self.empty();
        addresses.push("127.0.0.1:443".parse().unwrap());
        Ok(addresses)
    }
}

impl Connector for OfflineHttp {
    type Out = OfflineTransport;
    fn connect(
        &self,
        details: &ConnectionDetails,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        self.requests.lock().unwrap().push(details.uri.to_string());
        Ok(Some(OfflineTransport {
            response: Cursor::new(
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected HTTP request"),
            ),
            buffers: LazyBuffers::new(4096, 4096),
        }))
    }
}

#[derive(Debug)]
struct OfflineTransport {
    response: Cursor<Vec<u8>>,
    buffers: LazyBuffers,
}

impl Transport for OfflineTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }
    fn transmit_output(&mut self, _: usize, _: NextTimeout) -> Result<(), ureq::Error> {
        Ok(())
    }
    fn await_input(&mut self, _: NextTimeout) -> Result<bool, ureq::Error> {
        let count = self.response.read(self.buffers.input_append_buf())?;
        self.buffers.input_appended(count);
        Ok(count > 0)
    }
    fn is_open(&mut self) -> bool {
        false
    }
    fn is_tls(&self) -> bool {
        true
    }
}

fn offline_agent(responses: Vec<Vec<u8>>) -> (ureq::Agent, Arc<Mutex<Vec<String>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let connector = OfflineHttp {
        responses: Mutex::new(responses.into()),
        requests: Arc::clone(&requests),
    };
    let resolver = OfflineHttp {
        responses: Mutex::default(),
        requests: Arc::clone(&requests),
    };
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .build();
    (
        ureq::Agent::with_parts(config, connector, resolver),
        requests,
    )
}

fn response(bytes: &[u8]) -> Vec<u8> {
    crate::download::tests::response(bytes)
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    #[cfg(windows)]
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[cfg(unix)]
    {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut writer = tar::Builder::new(encoder);
        for (name, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
            header.set_cksum();
            writer.append(&header, Cursor::new(bytes)).unwrap();
        }
        writer.into_inner().unwrap().finish().unwrap()
    }
}

#[test]
fn verified_official_archive_is_prepared_without_touching_installed_files() {
    let fixture = Fixture::new();
    let executable = fixture.install();
    let operation = fixture.operation();
    let bytes = native_header(release::current_target().unwrap());
    let packed = archive(&[
        (&executable_name("verslot"), &bytes),
        ("LICENSE", b"license"),
    ]);
    let digest: String = Sha256::digest(&packed)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let checksums = format!("{digest}  {}\n", filename());
    let (agent, requests) = offline_agent(vec![response(checksums.as_bytes()), response(&packed)]);
    release::prepare(&agent, &selected_release(), &fixture.root, &operation).unwrap();
    assert_eq!(requests.lock().unwrap().len(), 2);
    assert_eq!(fs::read(candidate_executable(&operation)).unwrap(), bytes);
    assert_eq!(fs::read(executable).unwrap(), b"old executable");
}

#[test]
fn checksum_mismatch_is_rejected_before_extraction() {
    let fixture = Fixture::new();
    let operation = fixture.operation();
    let checksums = format!("{}  {}\n", "a".repeat(64), filename());
    let (agent, _) = offline_agent(vec![
        response(checksums.as_bytes()),
        response(b"untrusted archive"),
    ]);
    let error =
        release::prepare(&agent, &selected_release(), &fixture.root, &operation).unwrap_err();
    assert!(error.contains("SHA-256 mismatch"));
    assert!(!operation.join("candidate").exists());
}

#[test]
fn malformed_or_duplicate_checksums_prevent_archive_download() {
    for text in [
        format!("bad  {}\n", filename()),
        format!(
            "{}  {}\n{}  {}\n",
            "a".repeat(64),
            filename(),
            "b".repeat(64),
            filename()
        ),
    ] {
        let fixture = Fixture::new();
        let operation = fixture.operation();
        let (agent, requests) = offline_agent(vec![response(text.as_bytes())]);
        assert!(release::prepare(&agent, &selected_release(), &fixture.root, &operation).is_err());
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert!(!operation.join("release.archive").exists());
    }
}

#[test]
fn release_download_follows_only_official_https_redirects() {
    for url in [
        "http://release-assets.githubusercontent.com/asset",
        "https://example.com/asset",
        "https://github.com.evil.test/asset",
        "https://user@github.com/asset",
        "https://github.com:444/asset",
        "file:///tmp/asset",
    ] {
        let fixture = Fixture::new();
        let operation = fixture.operation();
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: {url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let (agent, requests) = offline_agent(vec![redirect.into_bytes()]);
        let error =
            release::prepare(&agent, &selected_release(), &fixture.root, &operation).unwrap_err();
        assert!(error.contains("official HTTPS hosts"), "{error}");
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn latest_metadata_errors_are_offline_and_do_not_select_an_update() {
    let (agent, _) = offline_agent(vec![
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    ]);
    assert!(
        release::latest(&agent, "0.8.0", release::current_target().unwrap())
            .unwrap_err()
            .contains("no published stable release")
    );
    let (agent, _) = offline_agent(vec![response(b"not JSON")]);
    assert!(release::latest(&agent, "0.8.0", release::current_target().unwrap()).is_err());
}

#[test]
fn manager_archives_reject_unsafe_paths_extra_files_and_missing_license() {
    for name in [
        "../escape",
        "/absolute",
        "C:/drive",
        "verslot/child",
        "LICENSE:stream",
        "extra-file",
    ] {
        let fixture = Fixture::new();
        let operation = fixture.operation();
        let path = operation.join("release.archive");
        fs::write(&path, archive(&[(name, b"unsafe")])).unwrap();
        assert!(
            crate::installation::extract_verified_self_update(
                &fixture.root,
                &path,
                &operation.join("candidate")
            )
            .is_err(),
            "{name}"
        );
        assert!(!fixture.root.join("escape").exists());
    }
    let fixture = Fixture::new();
    let operation = fixture.operation();
    let path = operation.join("release.archive");
    fs::write(
        &path,
        archive(&[(&executable_name("verslot"), b"executable")]),
    )
    .unwrap();
    assert!(
        crate::installation::extract_verified_self_update(
            &fixture.root,
            &path,
            &operation.join("candidate")
        )
        .is_err()
    );
}
