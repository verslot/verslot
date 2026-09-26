use ::zip::write::SimpleFileOptions;
use ::zip::{CompressionMethod, ZipWriter};

use super::*;

fn zip_bytes(names: &[String], compression: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .unwrap();
        writer.write_all(b"node fixture").unwrap();
    }
    writer.finish().unwrap().into_inner()
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

#[test]
fn stored_and_deflated_zip_preserve_bundled_files_and_write_valid_receipts() {
    for compression in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let fixture = Fixture::new();
        let root = fixture.archive_root();
        let bytes = zip_bytes(
            &[
                format!("{root}/node.exe"),
                format!("{root}/node_modules/npm/bin/npm-cli.js"),
            ],
            compression,
        );
        extract_bytes(&fixture, &bytes).unwrap();
        let staging = fixture.root.join("staging");
        assert_eq!(
            fs::read(staging.join("node_modules/npm/bin/npm-cli.js")).unwrap(),
            b"node fixture"
        );
        assert!(staging.join(RECEIPT).is_file());
        fs::create_dir_all(fixture.root.join("installs/node")).unwrap();
        fs::rename(staging, fixture.root.join("installs/node/22.0.0")).unwrap();
        validate_installation(&fixture.root, &fixture.target()).unwrap();
    }
}

#[test]
fn zip_rejects_unsafe_paths_and_preserves_an_external_sentinel() {
    for suffix in [
        "../sentinel",
        "/absolute",
        "C:/drive",
        "\\\\server\\share",
        "file:ads",
        "a\\b",
        "NUL.txt",
        "CON .txt",
        "COM1",
        "a.",
        "a ",
        ".verslot-install",
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.base.join("sentinel"), b"preserve").unwrap();
        let bytes = zip_bytes(
            &[format!("{}/{suffix}", fixture.archive_root())],
            CompressionMethod::Stored,
        );
        assert!(extract_bytes(&fixture, &bytes).is_err(), "{suffix:?}");
        assert_eq!(
            fs::read(fixture.base.join("sentinel")).unwrap(),
            b"preserve"
        );
        assert!(!fixture.root.join("staging").join(RECEIPT).exists());
    }
}

#[test]
fn duplicate_zip_records_cannot_be_hidden_by_the_library_name_index() {
    let fixture = Fixture::new();
    let root = fixture.archive_root();
    let mut bytes = zip_bytes(
        &[format!("{root}/one"), format!("{root}/two")],
        CompressionMethod::Stored,
    );
    let needle = format!("{root}/two").into_bytes();
    let replacement = format!("{root}/one").into_bytes();
    for position in 0..bytes.len().saturating_sub(needle.len()) {
        if bytes[position..position + needle.len()] == needle {
            bytes[position..position + needle.len()].copy_from_slice(&replacement);
        }
    }
    assert!(
        extract_bytes(&fixture, &bytes)
            .unwrap_err()
            .to_string()
            .contains("duplicate ZIP")
    );
}

#[test]
fn zip_rejects_symlinks_and_filesystem_case_aliases() {
    let fixture = Fixture::new();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            format!("{}/node.exe", fixture.archive_root()),
            "other",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
    assert!(extract_bytes(&fixture, &writer.finish().unwrap().into_inner()).is_err());
    let fixture = Fixture::new();
    let root = fixture.archive_root();
    let bytes = zip_bytes(
        &[format!("{root}/node.exe"), format!("{root}/NODE.EXE")],
        CompressionMethod::Stored,
    );
    assert!(extract_bytes(&fixture, &bytes).is_err());
}

#[test]
fn zip_crc_truncation_encryption_and_unsupported_compression_fail_before_receipt_creation() {
    let fixture = Fixture::new();
    let bytes = zip_bytes(
        &[format!("{}/node.exe", fixture.archive_root())],
        CompressionMethod::Stored,
    );
    let local = bytes
        .windows(4)
        .position(|bytes| bytes == b"PK\x03\x04")
        .unwrap();
    let central = bytes
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    let name_length =
        u16::from_le_bytes(bytes[local + 26..local + 28].try_into().unwrap()) as usize;
    let extra_length =
        u16::from_le_bytes(bytes[local + 28..local + 30].try_into().unwrap()) as usize;
    let mut corrupt = bytes.clone();
    corrupt[local + 30 + name_length + extra_length] ^= 1;
    let mut encrypted = bytes.clone();
    encrypted[local + 6] |= 1;
    encrypted[central + 8] |= 1;
    let mut unsupported = bytes.clone();
    unsupported[local + 8..local + 10].copy_from_slice(&12_u16.to_le_bytes());
    unsupported[central + 10..central + 12].copy_from_slice(&12_u16.to_le_bytes());
    for archive in [
        corrupt,
        bytes[..bytes.len() - 5].to_vec(),
        encrypted,
        unsupported,
        vec![1, 2, 3],
    ] {
        assert!(extract_bytes(&fixture, &archive).is_err());
        let staging = fixture.root.join("staging");
        assert!(!staging.join(RECEIPT).exists());
        fs::remove_dir_all(staging).unwrap();
    }
}

#[test]
fn zip_byte_and_entry_limits_are_enforced_before_payload_writes() {
    for (byte_limit, entry_limit) in [(1, 10), (1000, 0)] {
        let fixture = Fixture::new();
        let archive = fixture.root.join("archive");
        fs::write(
            &archive,
            zip_bytes(
                &[format!("{}/node.exe", fixture.archive_root())],
                CompressionMethod::Stored,
            ),
        )
        .unwrap();
        let staging = fixture.root.join("staging");
        let mut destination = Extraction::new(
            &fixture.root,
            &staging,
            &fixture.distribution(),
            byte_limit,
            entry_limit,
        )
        .unwrap();
        assert!(
            super::super::zip::extract(File::open(archive).unwrap(), &mut destination).is_err()
        );
        assert!(!staging.join("node.exe").exists());
    }
}

#[test]
fn windows_junction_mutation_ancestors_and_final_installations_are_rejected() {
    for final_entry in [false, true] {
        let fixture = Fixture::new();
        let directory = fixture.installed_directory();
        let original = if final_entry {
            directory
        } else {
            fixture.root.join("installs")
        };
        let outside = fixture.base.join("outside");
        fs::rename(&original, &outside).unwrap();
        junction::create(&outside, &original).unwrap();
        assert!(validate_installation(&fixture.root, &fixture.target()).is_err());
        junction::delete(original).unwrap();
        assert!(outside.exists());
    }
}

#[test]
fn linked_staging_ancestors_are_rejected_and_canonicalized_storage_root_junctions_work() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"preserve").unwrap();
    let temporary = fixture.root.join("tmp");
    junction::create(&outside, &temporary).unwrap();
    assert!(
        Extraction::new(
            &fixture.root,
            &temporary.join("staging"),
            &fixture.distribution(),
            100,
            10
        )
        .is_err()
    );
    assert!(!outside.join("staging").exists());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"preserve");
    junction::delete(temporary).unwrap();
    let alias = fixture.base.join("storage-link");
    junction::create(&fixture.root, &alias).unwrap();
    let canonical = fs::canonicalize(&alias).unwrap();
    let staging = canonical.join("staging");
    Extraction::new(&canonical, &staging, &fixture.distribution(), 100, 10).unwrap();
    junction::delete(alias).unwrap();
}

#[test]
fn zip64_directory_records_are_bounded_and_supported() {
    let fixture = Fixture::new();
    let mut bytes = zip_bytes(
        &[format!("{}/node.exe", fixture.archive_root())],
        CompressionMethod::Stored,
    );
    let end = bytes.len() - 22;
    let mut footer = bytes.split_off(end);
    let directory_size = u32::from_le_bytes(footer[12..16].try_into().unwrap()) as u64;
    let directory_offset = u32::from_le_bytes(footer[16..20].try_into().unwrap()) as u64;
    let mut record = vec![0; 56];
    record[..4].copy_from_slice(b"PK\x06\x06");
    record[4..12].copy_from_slice(&44_u64.to_le_bytes());
    record[12..14].copy_from_slice(&45_u16.to_le_bytes());
    record[14..16].copy_from_slice(&45_u16.to_le_bytes());
    record[24..32].copy_from_slice(&1_u64.to_le_bytes());
    record[32..40].copy_from_slice(&1_u64.to_le_bytes());
    record[40..48].copy_from_slice(&directory_size.to_le_bytes());
    record[48..56].copy_from_slice(&directory_offset.to_le_bytes());
    bytes.extend(record);
    let mut locator = vec![0; 20];
    locator[..4].copy_from_slice(b"PK\x06\x07");
    locator[8..16].copy_from_slice(&(end as u64).to_le_bytes());
    locator[16..20].copy_from_slice(&1_u32.to_le_bytes());
    bytes.extend(locator);
    footer[8..12].fill(0xff);
    footer[12..20].fill(0xff);
    bytes.extend(footer);
    extract_bytes(&fixture, &bytes).unwrap();
    assert!(fixture.root.join("staging").join(RECEIPT).is_file());
}
