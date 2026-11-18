use std::fs;
use tinydict_core::{Dictionary, DictionaryInfo, Entry, Sense};
use tinydict_data::{ImportReport, load_disk_package, write_package};

fn dictionary(revision: &str) -> Dictionary {
    let words = ["US", "us", "café", "cafe\u{301}", "ice cream"];
    let mut entries: Vec<_> = words
        .into_iter()
        .enumerate()
        .map(|(i, word)| Entry {
            id: i.to_string(),
            word: word.into(),
            labels: vec!["label".into()],
            contributors: vec!["Original author".into()],
            senses: vec![Sense {
                definition: format!("Definition from {revision}, sense {i}"),
                examples: vec!["Original example.\n— Reference".into()],
                source: "source".into(),
                ..Sense::default()
            }],
        })
        .collect();
    for i in 0..800 {
        entries.push(Entry {
            id: format!("large-{i}"),
            word: format!("word-{i:04}"),
            labels: vec![],
            contributors: vec![],
            senses: vec![Sense {
                definition: format!(
                    "Definition {i}: {}",
                    "a fairly long example sentence ".repeat(40)
                ),
                ..Sense::default()
            }],
        });
    }
    Dictionary::build(
        DictionaryInfo {
            id: "test".into(),
            name: "Test".into(),
            source_url: "https://example.test".into(),
            revision: revision.into(),
            license: "Full original notice".into(),
        },
        entries,
    )
    .unwrap()
}

#[test]
fn disk_indexes_preserve_all_content_and_read_text_only_from_disk() {
    let temp = tempfile::tempdir().unwrap();
    let original = dictionary("one");
    for filename in ["legacy.tdict", "compressed.tdict.zst"] {
        let path = temp.path().join(filename);
        let cache = temp.path().join(format!("{filename}-cache"));
        write_package(&path, &original, ImportReport::default()).unwrap();
        let disk = load_disk_package(&path, &cache).unwrap();
        assert_eq!(disk.info(), original.info());
        assert_eq!(disk.headword_count(), original.groups().len());
        assert_eq!(disk.entry_count(), original.entry_count());
        assert_eq!(disk.sense_count(), original.sense_count());
        assert!(disk.estimated_bytes() < original.estimated_bytes() / 3);
        for (id, group) in original.groups().iter().enumerate() {
            assert_eq!(disk.entries(id).unwrap(), group.entries);
            assert_eq!(disk.headword(id), Some(group.entries[0].word.as_str()));
        }
        for query in [
            "US",
            "cafe\u{301}",
            "CAFÉ",
            "ice cream",
            "word-05",
            "",
            "missing",
        ] {
            assert_eq!(disk.exact(query), original.exact(query));
            assert_eq!(disk.prefix(query, 10), original.prefix(query, 10));
        }
        let cache_file = fs::read_dir(&cache)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let modified = fs::metadata(&cache_file).unwrap().modified().unwrap();
        let reopened = load_disk_package(&path, &cache).unwrap();
        assert_eq!(
            fs::metadata(&cache_file).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(reopened.entries(0).unwrap(), original.groups()[0].entries);
        fs::remove_file(path).unwrap();
        assert_eq!(disk.entries(0).unwrap(), original.groups()[0].entries);
        fs::remove_file(cache_file).unwrap();
        assert_eq!(disk.exact("US"), original.exact("US"));
        assert!(!disk.prefix("word-", 10).is_empty());
        assert!(
            disk.entries(0).is_err(),
            "definitions were incorrectly retained in memory"
        );
    }
}

#[test]
fn snapshot_changes_invalidate_cache_and_corrupt_input_never_publishes_one() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("book.tdict.zst");
    let replacement = temp.path().join("new.tdict.zst");
    let cache = temp.path().join("cache");
    write_package(&source, &dictionary("old"), ImportReport::default()).unwrap();
    write_package(&replacement, &dictionary("new"), ImportReport::default()).unwrap();
    assert_eq!(
        load_disk_package(&source, &cache).unwrap().info().revision,
        "old"
    );
    fs::copy(&replacement, &source).unwrap();
    assert_eq!(
        load_disk_package(&source, &cache).unwrap().info().revision,
        "new"
    );
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 2);
    let mut truncated = fs::read(&source).unwrap();
    truncated.truncate(truncated.len() - 4);
    fs::write(&source, truncated).unwrap();
    let invalid_cache = temp.path().join("failed-cache");
    assert!(load_disk_package(&source, &invalid_cache).is_err());
    assert_eq!(fs::read_dir(invalid_cache).unwrap().count(), 0);
}

#[test]
fn corrupt_index_and_block_checksums_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("book.tdict.zst");
    let cache = temp.path().join("cache");
    write_package(&source, &dictionary("one"), ImportReport::default()).unwrap();
    let disk = load_disk_package(&source, &cache).unwrap();
    let cache_file = fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read(&cache_file).unwrap();
    let number = |at| u64::from_le_bytes(original[at..at + 8].try_into().unwrap()) as usize;
    let metadata: serde_json::Value =
        serde_json::from_slice(&original[number(8)..number(16)]).unwrap();
    let first = &metadata["blocks"][0];
    let end = first["offset"].as_u64().unwrap() + first["compressed"].as_u64().unwrap();
    let mut damaged = original.clone();
    damaged[end as usize - 1] ^= 0xff;
    fs::write(&cache_file, damaged).unwrap();
    assert!(disk.entries(0).is_err());
    fs::write(&cache_file, &original[..original.len() / 2]).unwrap();
    assert!(load_disk_package(&source, &cache).is_err());
}
