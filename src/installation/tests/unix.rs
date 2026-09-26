use flate2::Compression;
use flate2::write::GzEncoder;
use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;

struct Member {
    name: String,
    kind: u8,
    bytes: Vec<u8>,
    mode: u32,
    link: String,
}

impl Member {
    fn regular_file(name: String) -> Self {
        Self {
            name,
            kind: b'0',
            bytes: b"node fixture".to_vec(),
            mode: 0o755,
            link: String::new(),
        }
    }

    fn link(name: String, target: &str) -> Self {
        Self {
            name,
            kind: b'2',
            bytes: Vec::new(),
            mode: 0o777,
            link: target.to_owned(),
        }
    }

    fn metadata(kind: u8, bytes: Vec<u8>) -> Self {
        Self {
            name: "././@LongLink".to_owned(),
            kind,
            bytes,
            mode: 0o644,
            link: String::new(),
        }
    }
}

fn tar_bytes(members: Vec<Member>) -> Vec<u8> {
    let mut builder = ::tar::Builder::new(Vec::new());
    for member in members {
        let mut header = ::tar::Header::new_gnu();
        header.set_mode(member.mode);
        header.set_size(member.bytes.len() as u64);
        header.set_entry_type(::tar::EntryType::new(member.kind));
        assert!(member.name.len() <= 100 && member.link.len() <= 100);
        header.as_mut_bytes()[..member.name.len()].copy_from_slice(member.name.as_bytes());
        header.as_mut_bytes()[157..157 + member.link.len()].copy_from_slice(member.link.as_bytes());
        header.set_cksum();
        builder.append(&header, Cursor::new(member.bytes)).unwrap();
    }
    builder.into_inner().unwrap()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn extract_bytes(fixture: &Fixture, bytes: &[u8]) -> io::Result<()> {
    let archive = fixture.root.join("archive");
    fs::write(&archive, bytes).unwrap();
    extract_verified_archive(
        &fixture.root,
        &archive,
        &fixture.root.join("staging"),
        fixture.target().version,
        DIGEST,
    )
}

fn pax(key: &str, value: &str) -> Vec<u8> {
    let body = format!(" {key}={value}\n");
    let mut length = body.len() + 1;
    loop {
        let next = length.to_string().len() + body.len();
        if next == length {
            return format!("{length}{body}").into_bytes();
        }
        length = next;
    }
}

#[test]
fn tar_preserves_bundled_files_and_safe_npm_link_chains_without_special_permissions() {
    let fixture = Fixture::new();
    let root = fixture.archive_root();
    let mut node = Member::regular_file(format!("{root}/bin/node"));
    node.mode = 0o6755;
    let bytes = gzip(&tar_bytes(vec![
        node,
        Member::link(format!("{root}/bin/npm"), "npm-next"),
        Member::link(
            format!("{root}/bin/npm-next"),
            "../lib/node_modules/npm/bin/npm-cli.js",
        ),
        Member::regular_file(format!("{root}/lib/node_modules/npm/bin/npm-cli.js")),
    ]));
    extract_bytes(&fixture, &bytes).unwrap();
    let staging = fixture.root.join("staging");
    assert_eq!(fs::read(staging.join("bin/npm")).unwrap(), b"node fixture");
    assert_eq!(
        fs::read_link(staging.join("bin/npm")).unwrap(),
        Path::new("npm-next")
    );
    assert_eq!(
        fs::metadata(staging.join("bin/node"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    assert!(staging.join(RECEIPT).is_file());
    fs::create_dir_all(fixture.root.join("installs/node")).unwrap();
    fs::rename(staging, fixture.root.join("installs/node/22.0.0")).unwrap();
    validate_installation(&fixture.root, &fixture.target()).unwrap();
}

#[test]
fn tar_rejects_traversal_wrong_roots_reserved_paths_and_special_entries() {
    for suffix in [
        "../sentinel",
        "/absolute",
        "C:/drive",
        "a\\b",
        "file:ads",
        "CON",
        "trail.",
        ".verslot-install",
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.base.join("sentinel"), b"preserve").unwrap();
        let bytes = gzip(&tar_bytes(vec![Member::regular_file(format!(
            "{}/{suffix}",
            fixture.archive_root()
        ))]));
        assert!(extract_bytes(&fixture, &bytes).is_err());
        assert_eq!(
            fs::read(fixture.base.join("sentinel")).unwrap(),
            b"preserve"
        );
        assert!(!fixture.root.join("staging").join(RECEIPT).exists());
    }
    for kind in [b'1', b'3', b'4', b'6', b'S', b'7'] {
        let fixture = Fixture::new();
        let mut member = Member::link(format!("{}/special", fixture.archive_root()), "bin/node");
        member.kind = kind;
        assert!(extract_bytes(&fixture, &gzip(&tar_bytes(vec![member]))).is_err());
    }
    let fixture = Fixture::new();
    assert!(
        extract_bytes(
            &fixture,
            &gzip(&tar_bytes(vec![Member::regular_file(
                "wrong/bin/node".to_owned()
            )]))
        )
        .is_err()
    );
}

#[test]
fn tar_rejects_link_escapes_dangling_cycles_and_payload_link_ancestors() {
    for target in [
        "/absolute",
        "../../sentinel",
        "C:/drive",
        "..\\escape",
        "missing",
        "link",
    ] {
        let fixture = Fixture::new();
        let root = fixture.archive_root();
        let bytes = gzip(&tar_bytes(vec![
            Member::regular_file(format!("{root}/bin/node")),
            Member::link(format!("{root}/bin/link"), target),
        ]));
        assert!(extract_bytes(&fixture, &bytes).is_err(), "{target}");
        assert!(!fixture.root.join("staging").join(RECEIPT).exists());
    }
    for reversed in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.archive_root();
        let link = Member::link(format!("{root}/alias"), "bin");
        let child = Member::regular_file(format!("{root}/alias/child"));
        let members = if reversed {
            vec![child, link]
        } else {
            vec![link, child]
        };
        assert!(extract_bytes(&fixture, &gzip(&tar_bytes(members))).is_err());
    }
    let fixture = Fixture::new();
    let root = fixture.archive_root();
    assert!(
        extract_bytes(
            &fixture,
            &gzip(&tar_bytes(vec![
                Member::regular_file(format!("{root}/bin/node")),
                Member::link(format!("{root}/a"), "b"),
                Member::link(format!("{root}/b"), "a"),
            ]))
        )
        .is_err()
    );
}

#[test]
fn tar_directory_links_allow_acyclic_targets_but_reject_indirect_directory_cycles() {
    for cyclic in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.archive_root();
        let mut members = vec![
            Member::regular_file(format!("{root}/bin/node")),
            Member::regular_file(format!("{root}/a/file")),
            Member::regular_file(format!("{root}/b/file")),
            Member::link(format!("{root}/a/to-b"), "../b"),
        ];
        if cyclic {
            members.push(Member::link(format!("{root}/b/to-a"), "../a"));
        }
        assert_eq!(
            extract_bytes(&fixture, &gzip(&tar_bytes(members))).is_err(),
            cyclic
        );
    }
}

#[test]
fn gnu_and_pax_effective_names_are_validated_and_metadata_entries_count_toward_limits() {
    for kind in [b'L', b'x'] {
        for malicious in [false, true] {
            let fixture = Fixture::new();
            let root = fixture.archive_root();
            let name = if malicious {
                format!("{root}/../escape")
            } else {
                format!("{root}/bin/node")
            };
            let metadata = if kind == b'L' {
                format!("{name}\0").into_bytes()
            } else {
                pax("path", &name)
            };
            let bytes = gzip(&tar_bytes(vec![
                Member::metadata(kind, metadata),
                Member::regular_file("placeholder".to_owned()),
            ]));
            assert_eq!(extract_bytes(&fixture, &bytes).is_err(), malicious);
        }
    }
    let fixture = Fixture::new();
    let name = format!("{}/bin/node", fixture.archive_root());
    let archive_path = fixture.root.join("archive");
    fs::write(
        &archive_path,
        gzip(&tar_bytes(vec![
            Member::metadata(b'x', pax("path", &name)),
            Member::regular_file(name),
        ])),
    )
    .unwrap();
    let staging = fixture.root.join("staging");
    let mut destination =
        Extraction::new(&fixture.root, &staging, &fixture.distribution(), 1000, 1).unwrap();
    assert!(
        super::super::tar::extract(File::open(archive_path).unwrap(), &mut destination).is_err()
    );
}

#[test]
fn tar_rejects_sparse_pax_size_overrides_orphan_metadata_and_extraction_limits() {
    for metadata in [pax("GNU.sparse.map", "0,1"), pax("size", "9999")] {
        let fixture = Fixture::new();
        let bytes = gzip(&tar_bytes(vec![
            Member::metadata(b'x', metadata),
            Member::regular_file(format!("{}/bin/node", fixture.archive_root())),
        ]));
        assert!(extract_bytes(&fixture, &bytes).is_err());
    }
    let fixture = Fixture::new();
    assert!(
        extract_bytes(
            &fixture,
            &gzip(&tar_bytes(vec![Member::metadata(
                b'L',
                b"unused\0".to_vec()
            )]))
        )
        .is_err()
    );
    let fixture = Fixture::new();
    let archive = fixture.root.join("archive");
    fs::write(
        &archive,
        gzip(&tar_bytes(vec![Member::regular_file(format!(
            "{}/bin/node",
            fixture.archive_root()
        ))])),
    )
    .unwrap();
    let staging = fixture.root.join("staging");
    let mut destination =
        Extraction::new(&fixture.root, &staging, &fixture.distribution(), 1, 10).unwrap();
    assert!(super::super::tar::extract(File::open(archive).unwrap(), &mut destination).is_err());
}

#[test]
fn tar_and_gzip_corruption_and_missing_end_markers_never_create_receipts() {
    let fixture = Fixture::new();
    let raw = tar_bytes(vec![Member::regular_file(format!(
        "{}/bin/node",
        fixture.archive_root()
    ))]);
    let mut corrupt_header = raw.clone();
    corrupt_header[0] ^= 1;
    let mut trailing = raw.clone();
    trailing.extend_from_slice(b"hidden payload");
    let mut bad_gzip = gzip(&raw);
    let index = bad_gzip.len() - 8;
    bad_gzip[index] ^= 1;
    for bytes in [
        gzip(&corrupt_header),
        gzip(&raw[..1024]),
        gzip(&raw[..520]),
        gzip(&trailing),
        bad_gzip,
        vec![1, 2, 3],
    ] {
        // Use the same distribution root as the fixture which produced the raw headers.
        let staging = fixture.root.join("staging");
        assert!(extract_bytes(&fixture, &bytes).is_err());
        assert!(!staging.join(RECEIPT).exists());
        fs::remove_dir_all(staging).unwrap();
    }
}

#[test]
fn linked_mutation_ancestors_receipts_and_executables_are_rejected() {
    for linked in ["installs", "receipt", "executable"] {
        let fixture = Fixture::new();
        let directory = fixture.installed_directory();
        let sentinel = fixture.base.join("sentinel");
        fs::write(&sentinel, b"preserve").unwrap();
        match linked {
            "installs" => {
                fs::rename(fixture.root.join("installs"), fixture.base.join("outside")).unwrap();
                symlink(fixture.base.join("outside"), fixture.root.join("installs")).unwrap();
            }
            "receipt" => {
                fs::remove_file(directory.join(RECEIPT)).unwrap();
                symlink(&sentinel, directory.join(RECEIPT)).unwrap();
            }
            _ => {
                let executable = fixture.executable_path(&directory);
                fs::remove_file(&executable).unwrap();
                symlink(&sentinel, executable).unwrap();
            }
        }
        assert!(validate_installation(&fixture.root, &fixture.target()).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"preserve");
    }
}

#[test]
fn nonexecutable_unix_payloads_and_archive_executable_links_are_incomplete() {
    for linked in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.archive_root();
        let members = if linked {
            vec![
                Member::link(format!("{root}/bin/node"), "real-node"),
                Member::regular_file(format!("{root}/bin/real-node")),
            ]
        } else {
            let mut node = Member::regular_file(format!("{root}/bin/node"));
            node.mode = 0o644;
            vec![node]
        };
        assert!(extract_bytes(&fixture, &gzip(&tar_bytes(members))).is_err());
        assert!(!fixture.root.join("staging").join(RECEIPT).exists());
    }
}

#[test]
fn linked_staging_ancestors_are_rejected_and_canonicalized_storage_root_links_work() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserve").unwrap();
    symlink(&outside, fixture.root.join("tmp")).unwrap();
    assert!(
        Extraction::new(
            &fixture.root,
            &fixture.root.join("tmp/staging"),
            &fixture.distribution(),
            100,
            10,
        )
        .is_err()
    );
    assert!(!outside.join("staging").exists());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserve");
    let alias = fixture.base.join("storage-link");
    symlink(&fixture.root, &alias).unwrap();
    let canonical = fs::canonicalize(alias).unwrap();
    let staging = canonical.join("staging");
    Extraction::new(&canonical, &staging, &fixture.distribution(), 100, 10).unwrap();
}

#[test]
fn oversized_tar_metadata_is_rejected_before_materializing_the_payload() {
    let fixture = Fixture::new();
    let bytes = gzip(&tar_bytes(vec![Member::metadata(
        b'L',
        vec![b'a'; 1024 * 1024 + 1],
    )]));
    assert!(
        extract_bytes(&fixture, &bytes)
            .unwrap_err()
            .to_string()
            .contains("metadata exceeds")
    );
    assert!(!fixture.root.join("staging").join(RECEIPT).exists());
}

#[test]
fn tar_nul_suffixes_and_malformed_pax_records_are_rejected() {
    for metadata in [None, Some(b"not a PAX record\n".to_vec())] {
        let fixture = Fixture::new();
        let mut members = Vec::new();
        if let Some(bytes) = metadata {
            members.push(Member::metadata(b'x', bytes));
            members.push(Member::regular_file(format!(
                "{}/bin/node",
                fixture.archive_root()
            )));
        } else {
            members.push(Member::regular_file(format!(
                "{}/bin/node\0hidden",
                fixture.archive_root()
            )));
        }
        assert!(extract_bytes(&fixture, &gzip(&tar_bytes(members))).is_err());
    }
}

#[test]
fn gnu_and_pax_link_targets_are_validated_after_resolution() {
    for kind in [b'K', b'x'] {
        for escaping in [false, true] {
            let fixture = Fixture::new();
            let root = fixture.archive_root();
            let target = if escaping {
                "../../sentinel"
            } else {
                "../lib/npm.js"
            };
            let metadata = if kind == b'K' {
                format!("{target}\0").into_bytes()
            } else {
                pax("linkpath", target)
            };
            let bytes = gzip(&tar_bytes(vec![
                Member::regular_file(format!("{root}/bin/node")),
                Member::metadata(kind, metadata),
                Member::link(format!("{root}/bin/npm"), "placeholder"),
                Member::regular_file(format!("{root}/lib/npm.js")),
            ]));
            assert_eq!(extract_bytes(&fixture, &bytes).is_err(), escaping);
        }
    }
}
