use flate2::{Compression, write::GzEncoder};
use std::fs;
use std::io::Write;
use tinydict_core::DictionaryInfo;
use tinydict_data::{Source, import_source, inspect_package, load_package, write_package};

fn info(id: &str) -> DictionaryInfo {
    DictionaryInfo {
        id: id.into(),
        name: id.into(),
        source_url: "https://example.test".into(),
        revision: "test-revision".into(),
        license: "test license".into(),
    }
}

#[test]
fn cloudbytes_gzip_preserves_subsources_and_queries_survive_package_removal() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.jsonl.gz");
    let mut encoder = GzEncoder::new(fs::File::create(&input).unwrap(), Compression::fast());
    writeln!(encoder, r#"{{"id":"old-1","word":"widget","pos":null,"wordtype":"n.","definition":"An old test definition.","source":"webster1913"}}"#).unwrap();
    writeln!(encoder, r#"{{"id":"new-1","word":"widget","pos":"noun","definition":"A modern test definition.","examples":["Test this widget."],"synset_id":"set-1","source":"oewn"}}"#).unwrap();
    writeln!(
        encoder,
        r#"{{"id":"empty","word":"empty","definition":null,"source":"oewn"}}"#
    )
    .unwrap();
    encoder.finish().unwrap();
    let (dictionary, report) =
        import_source(Source::Cloudbytes, &input, info("cloudbytes"), 2).unwrap();
    assert_eq!(report.skipped_entries, 1);
    assert_eq!(report.skipped_senses, 1);
    assert_eq!(report.input_sha256[0].1.len(), 64);
    let package = temp.path().join("cloudbytes.tdict");
    write_package(&package, &dictionary, report).unwrap();
    let metadata = inspect_package(&package).unwrap();
    assert_eq!(metadata.dictionary.revision, "test-revision");
    let loaded = load_package(&package).unwrap();
    fs::remove_file(package).unwrap();
    fs::remove_file(input).unwrap();
    let group = &loaded.groups()[loaded.exact("WIDGET").unwrap()];
    assert_eq!(group.entries.len(), 2);
    assert_eq!(group.entries[0].senses[0].source, "webster1913");
    assert_eq!(group.entries[1].senses[0].examples, ["Test this widget."]);
    assert_eq!(
        group.entries[1].senses[0].synset_id.as_deref(),
        Some("set-1")
    );
    assert_eq!(loaded.prefix("wid", 10).len(), 1);
}

#[test]
fn wordset_retains_sense_order_labels_synonyms_and_contributors() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("w.json");
    fs::write(&input, r#"{"widget":{"word":"widget","wordset_id":"w1","labels":[{"name":"Test English","is_dialect":true}],"editors":["editor"],"contributors":["author"],"meanings":[{"id":"s1","def":"First test sense.","speech_part":"noun","example":"A widget.","synonyms":[null,"gadget"],"labels":[{"name":"informal"}]},{"id":"s2","def":"Second test sense."}]}}"#).unwrap();
    let (dictionary, report) =
        import_source(Source::Wordset, temp.path(), info("wordset"), 2).unwrap();
    assert_eq!(report.input_entries, 1);
    let entry = &dictionary.groups()[0].entries[0];
    assert_eq!(entry.labels, ["Test English (dialect)"]);
    assert_eq!(entry.contributors, ["author", "editor"]);
    assert_eq!(entry.senses[0].synonyms, ["gadget"]);
    assert_eq!(entry.senses[1].definition, "Second test sense.");
    let package = temp.path().join("wordset.tdict");
    write_package(&package, &dictionary, report.clone()).unwrap();
    assert!(write_package(&package, &dictionary, report).is_err());
    assert_eq!(
        load_package(&package).unwrap().groups()[0].entries[0],
        *entry
    );
}

#[test]
fn malformed_source_and_corrupt_package_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("bad.json");
    fs::write(&input, "{\"word\":").unwrap();
    assert!(import_source(Source::Wordset, &input, info("wordset"), 1).is_err());
    assert!(load_package(&input).is_err());
    let missing = temp.path().join("missing.tdict");
    assert!(inspect_package(&missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn wordset_entries_without_meanings_are_counted_and_skipped() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("words.json");
    fs::write(&input, r#"{"empty":{"word":"empty","wordset_id":"empty"},"valid":{"word":"valid","wordset_id":"valid","meanings":[{"id":"s1","def":"A test definition."}]}}"#).unwrap();
    let (dictionary, report) = import_source(Source::Wordset, &input, info("wordset"), 1).unwrap();
    assert_eq!(report.input_entries, 2);
    assert_eq!(report.skipped_entries, 1);
    assert_eq!(dictionary.entry_count(), 1);
    assert!(dictionary.exact("empty").is_none());
}
