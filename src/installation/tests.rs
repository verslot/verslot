use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    base: PathBuf,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "verslot-extraction-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let root = base.join("storage");
        fs::create_dir(&root).unwrap();
        Self { base, root }
    }

    fn distribution(&self) -> NodeDistribution {
        NodeDistribution::for_current_build(self.target().version).unwrap()
    }

    fn target(&self) -> Target {
        "node@22.0.0".parse().unwrap()
    }

    fn archive_root(&self) -> String {
        self.distribution()
            .archive_filename
            .trim_end_matches(".zip")
            .trim_end_matches(".tar.gz")
            .to_owned()
    }

    fn executable_path(&self, directory: &Path) -> PathBuf {
        directory.join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        })
    }

    fn installed_directory(&self) -> PathBuf {
        let directory = self.root.join("installs/node/22.0.0");
        fs::create_dir_all(&directory).unwrap();
        let executable = self.executable_path(&directory);
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"fixture node").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        write_receipt(
            &self.root,
            &directory,
            self.target().version,
            &self.distribution(),
            DIGEST,
        )
        .unwrap();
        directory
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        if self.base.parent() == Some(temporary.as_path())
            && fs::canonicalize(&self.base).ok().as_ref() == Some(&self.base)
        {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
}

#[test]
fn unsafe_entry_names_and_reserved_receipt_paths_are_rejected() {
    let fixture = Fixture::new();
    let staging = fixture.root.join("staging");
    let destination = Extraction::new(
        &fixture.root,
        &staging,
        &fixture.distribution(),
        BYTE_LIMIT,
        ENTRY_LIMIT,
    )
    .unwrap();
    let root = fixture.archive_root();
    for suffix in [
        "../escape",
        "/absolute",
        "a/../../escape",
        "C:/drive",
        "\\\\server\\share",
        "a\\b",
        "file:stream",
        "CON",
        "aux.txt",
        "CON .txt",
        "COM1.js",
        "LPT².txt",
        "NUL",
        "trailing.",
        "trailing ",
        "a\0b",
        "a//b",
        "./file",
        ".verslot-install",
        ".VERSLOT-INSTALL",
        ".verslot-install/child",
    ] {
        assert!(
            destination
                .validate_payload_path(format!("{root}/{suffix}").as_bytes(), false)
                .is_err(),
            "{suffix:?}"
        );
    }
    for name in ["/absolute", "../escape", "other/node.exe", "C:/node.exe"] {
        assert!(
            destination
                .validate_payload_path(name.as_bytes(), false)
                .is_err()
        );
    }
    assert_eq!(
        destination
            .validate_payload_path(format!("{root}/bin/node").as_bytes(), false)
            .unwrap(),
        "bin/node"
    );
    assert_eq!(
        destination
            .validate_payload_path(format!("{root}/").as_bytes(), true)
            .unwrap(),
        ""
    );
}

#[test]
fn duplicates_conflicts_and_implicit_parent_rules_are_enforced() {
    let fixture = Fixture::new();
    let staging = fixture.root.join("staging");
    let mut destination =
        Extraction::new(&fixture.root, &staging, &fixture.distribution(), 100, 10).unwrap();
    destination
        .write_file_entry("bin/node", 3, 0o755, &mut Cursor::new(b"abc"))
        .unwrap();
    destination.add_directory_entry("bin").unwrap();
    assert!(destination.add_directory_entry("bin").is_err());
    assert!(destination.add_directory_entry("bin/node").is_err());
    assert!(
        destination
            .write_file_entry("bin", 0, 0o755, &mut io::empty())
            .is_err()
    );
    assert!(
        destination
            .write_file_entry("bin/node", 3, 0o755, &mut Cursor::new(b"bad"))
            .is_err()
    );
    assert!(
        destination
            .write_file_entry("bin/node/child", 0, 0o755, &mut io::empty())
            .is_err()
    );
    assert_eq!(fs::read(staging.join("bin/node")).unwrap(), b"abc");
}

#[test]
fn byte_entry_and_truncation_limits_fail_without_a_receipt() {
    let fixture = Fixture::new();
    let staging = fixture.root.join("staging");
    let mut destination =
        Extraction::new(&fixture.root, &staging, &fixture.distribution(), 3, 1).unwrap();
    destination.count_entry().unwrap();
    assert!(destination.count_entry().is_err());
    assert!(
        destination
            .write_file_entry("big", 4, 0o644, &mut Cursor::new(b"abcd"))
            .is_err()
    );
    assert!(!staging.join("big").exists());
    assert!(
        destination
            .write_file_entry("short", 3, 0o644, &mut Cursor::new(b"ab"))
            .is_err()
    );
    assert!(!staging.join(RECEIPT).exists());
}

#[test]
fn staging_must_be_new_and_within_the_canonical_boundary() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserve").unwrap();
    assert!(
        Extraction::new(
            &fixture.root,
            &outside.join("staging"),
            &fixture.distribution(),
            10,
            10
        )
        .is_err()
    );
    let staging = fixture.root.join("staging");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("sentinel"), b"preserve").unwrap();
    assert!(Extraction::new(&fixture.root, &staging, &fixture.distribution(), 10, 10).is_err());
    assert_eq!(fs::read(staging.join("sentinel")).unwrap(), b"preserve");
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserve");
}

#[test]
fn receipts_require_exact_fields_target_platform_and_lowercase_digest() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let distribution = fixture.distribution();
    let text = format!(
        "verslot-install-v1\n{target}\n{}\n{DIGEST}\n",
        distribution.archive_filename
    );
    parse_receipt(text.as_bytes(), &target, &distribution).unwrap();
    for malformed in [
        text.trim_end().to_owned(),
        format!("{text}extra\n"),
        text.replace("v1", "v2"),
        text.replace("22.0.0", "22.0.1"),
        text.replace(&distribution.archive_filename, "wrong-archive.zip"),
        text.replace(DIGEST, &DIGEST.to_uppercase()),
        text.replace(DIGEST, "bad"),
        text.replace('\n', "\r\n"),
    ] {
        assert!(parse_receipt(malformed.as_bytes(), &target, &distribution).is_err());
    }
    assert!(parse_receipt(&[0xff], &target, &distribution).is_err());
}

#[test]
fn complete_installations_require_a_bounded_receipt_and_nonempty_regular_executable() {
    for failure in [
        "none",
        "missing receipt",
        "oversized receipt",
        "empty executable",
        "directory executable",
        "wrong platform",
    ] {
        let fixture = Fixture::new();
        let directory = fixture.installed_directory();
        let receipt = directory.join(RECEIPT);
        let executable = fixture.executable_path(&directory);
        match failure {
            "missing receipt" => fs::remove_file(receipt).unwrap(),
            "oversized receipt" => fs::write(receipt, vec![b'a'; 1025]).unwrap(),
            "empty executable" => fs::write(executable, b"").unwrap(),
            "directory executable" => {
                fs::remove_file(&executable).unwrap();
                fs::create_dir(executable).unwrap();
            }
            "wrong platform" => {
                let text = fs::read_to_string(&receipt).unwrap().replace(
                    &fixture.distribution().archive_filename,
                    "other-platform.zip",
                );
                fs::write(receipt, text).unwrap();
            }
            _ => {}
        }
        assert_eq!(
            validate_installation(&fixture.root, &fixture.target()).is_ok(),
            failure == "none",
            "{failure}"
        );
    }
}

#[test]
fn existing_receipts_are_preserved_by_exclusive_creation() {
    let fixture = Fixture::new();
    let directory = fixture.installed_directory();
    let before = fs::read(directory.join(RECEIPT)).unwrap();
    assert!(
        write_receipt(
            &fixture.root,
            &directory,
            fixture.target().version,
            &fixture.distribution(),
            DIGEST
        )
        .is_err()
    );
    assert_eq!(fs::read(directory.join(RECEIPT)).unwrap(), before);
}

#[test]
fn filesystem_case_aliases_are_rejected_when_the_filesystem_aliases_them() {
    let fixture = Fixture::new();
    let staging = fixture.root.join("staging");
    let mut destination =
        Extraction::new(&fixture.root, &staging, &fixture.distribution(), 10, 10).unwrap();
    destination.add_directory_entry("Bin").unwrap();
    if staging.join("bin").exists() {
        assert!(destination.add_directory_entry("bin").is_err());
        assert!(
            destination
                .write_file_entry("bin/node", 1, 0o755, &mut Cursor::new(b"a"))
                .is_err()
        );
    } else {
        destination.add_directory_entry("bin").unwrap();
    }
}
