use std::fs;
use tinydict_core::{Dictionary, DictionaryInfo, Entry, Sense};
use tinydict_data::{ImportReport, inspect_package, load_package, write_package};

fn dictionary() -> Dictionary {
    Dictionary::build(
        DictionaryInfo {
            id: "source-a".into(),
            name: "Independent source".into(),
            source_url: "https://example.test".into(),
            revision: "snapshot-1".into(),
            license: "Full source license\nAttribution retained.".into(),
        },
        vec![Entry {
            id: "entry-1".into(),
            word: "café".into(),
            contributors: vec!["Original author".into()],
            labels: vec!["informal".into()],
            senses: vec![Sense {
                id: "sense-1".into(),
                definition: "A place serving coffee.\nAdditional context.".into(),
                examples: vec!["Meet at the café.\n— Author, 2026".into()],
                synonyms: vec!["coffeehouse".into()],
                source: "original".into(),
                ..Sense::default()
            }],
        }],
    )
    .unwrap()
}

#[test]
fn compressed_and_legacy_packages_preserve_content_without_live_files() {
    let temp = tempfile::tempdir().unwrap();
    let original = dictionary();
    for filename in ["book.tdict", "book.tdict.zst"] {
        let path = temp.path().join(filename);
        let report = ImportReport::default();
        write_package(&path, &original, report.clone()).unwrap();
        let manifest = inspect_package(&path).unwrap();
        assert_eq!(manifest.dictionary.license, original.info().license);
        assert_eq!(manifest.dictionary.revision, original.info().revision);
        if filename.ends_with(".zst") {
            let bytes = fs::read(&path).unwrap();
            assert_eq!(&bytes[..4], &[0x28, 0xb5, 0x2f, 0xfd]);
            let plain = zstd::decode_all(bytes.as_slice()).unwrap();
            let text = String::from_utf8(plain).unwrap();
            assert!(text.contains("Original author"));
            assert_eq!(text.lines().count(), 2);
        }
        let before = fs::read(&path).unwrap();
        assert!(write_package(&path, &original, report).is_err());
        assert_eq!(fs::read(&path).unwrap(), before, "existing package changed");
        let loaded = load_package(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.estimated_bytes(), original.estimated_bytes());
        assert_eq!(loaded.groups()[0].entries, original.groups()[0].entries);
        assert_eq!(loaded.exact("CAFÉ"), Some(0));
        assert_eq!(loaded.prefix("ca", 5), [0]);
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn compressed_packages_reject_truncation_corruption_and_trailing_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("book.tdict.zst");
    write_package(&path, &dictionary(), ImportReport::default()).unwrap();
    let original = fs::read(&path).unwrap();
    let mut corrupt = original.clone();
    *corrupt.last_mut().unwrap() ^= 0xff;
    let mut trailing = original.clone();
    trailing.extend_from_slice(b"unrecognized trailing bytes");
    for bytes in [&original[..original.len() - 4], &corrupt, &trailing] {
        fs::write(&path, bytes).unwrap();
        assert!(load_package(&path).is_err(), "invalid frame was accepted");
    }
}

#[test]
fn compressed_manifest_is_inspectable_without_loading_body_and_counts_are_checked() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("book.tdict.zst");
    write_package(&path, &dictionary(), ImportReport::default()).unwrap();
    let plain = zstd::decode_all(fs::File::open(&path).unwrap()).unwrap();
    let mut lines = plain.split(|&byte| byte == b'\n');
    let header = lines.next().unwrap();
    let entry = lines.next().unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(header).unwrap();
    let write = |header: &serde_json::Value, body: &[u8]| {
        let mut text = serde_json::to_vec(header).unwrap();
        text.push(b'\n');
        text.extend_from_slice(body);
        text.push(b'\n');
        fs::write(&path, zstd::encode_all(text.as_slice(), 1).unwrap()).unwrap();
    };
    write(&manifest, b"invalid body");
    assert_eq!(inspect_package(&path).unwrap().entry_count, 1);
    assert!(load_package(&path).is_err());
    for field in ["schema_version", "entry_count", "sense_count"] {
        let mut changed = manifest.clone();
        changed[field] = 999.into();
        write(&changed, entry);
        assert!(load_package(&path).is_err(), "unchecked field: {field}");
    }
    let mut extra = entry.to_vec();
    extra.push(b'\n');
    extra.extend_from_slice(entry);
    write(&manifest, &extra);
    assert!(load_package(&path).is_err(), "extra entry was accepted");
}

#[test]
fn oversized_records_are_rejected_without_leaving_partial_output() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("book.tdict.zst");
    let original = dictionary();
    let mut entries = original.groups()[0].entries.clone();
    entries[0].senses[0].examples = vec!["x".repeat(20 * 1024 * 1024)];
    let oversized = Dictionary::build(original.info().clone(), entries).unwrap();
    assert!(write_package(&path, &oversized, ImportReport::default()).is_err());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    let huge_header = " ".repeat(20 * 1024 * 1024);
    fs::write(&path, zstd::encode_all(huge_header.as_bytes(), 1).unwrap()).unwrap();
    assert!(inspect_package(&path).is_err());
}
