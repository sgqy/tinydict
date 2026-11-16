use tinydict_core::{Dictionary, DictionaryInfo, Entry, ResourceBudget, Sense, normalize};

fn dictionary(name: &str, words: &[(&str, &str)]) -> Dictionary {
    let info = DictionaryInfo {
        id: name.into(),
        name: name.into(),
        source_url: String::new(),
        revision: "test".into(),
        license: "test".into(),
    };
    let entries = words
        .iter()
        .enumerate()
        .map(|(id, (word, definition))| Entry {
            id: id.to_string(),
            word: (*word).into(),
            labels: vec![],
            contributors: vec![],
            senses: vec![Sense {
                definition: (*definition).into(),
                ..Sense::default()
            }],
        })
        .collect();
    Dictionary::build(info, entries).unwrap()
}

#[test]
fn dictionaries_and_case_collisions_keep_all_definitions() {
    let a = dictionary(
        "a",
        &[("US", "country"), ("us", "pronoun"), ("use", "verb")],
    );
    let b = dictionary("b", &[("us", "different definition")]);
    let group = &a.groups()[a.exact(" US ").unwrap()];
    assert_eq!(group.entries.len(), 2);
    assert_eq!(group.entries[0].word, "US");
    assert_eq!(group.entries[1].senses[0].definition, "pronoun");
    assert_eq!(
        b.groups()[b.exact("us").unwrap()].entries[0].senses[0].definition,
        "different definition"
    );
}

#[test]
fn unicode_phrase_and_punctuation_searches_are_consistent() {
    let d = dictionary(
        "a",
        &[
            ("Café", "coffee"),
            ("Straße", "street"),
            ("ice cream", "food"),
            ("can't", "cannot"),
        ],
    );
    assert!(d.exact("cafe\u{301}").is_some());
    assert!(d.exact("STRASSE").is_some());
    assert!(d.exact("cant").is_none());
    assert_eq!(d.prefix("ice ", 10).len(), 1);
    assert!(d.prefix("", 10).is_empty());
    assert_eq!(normalize(" Café "), "café");
}

#[test]
fn prefix_is_ordered_bounded_and_exact_is_not_hidden_by_limit() {
    let d = dictionary("a", &[("apple", "a"), ("application", "b"), ("apply", "c")]);
    assert_eq!(
        d.prefix("app", 2)
            .iter()
            .map(|&id| d.groups()[id].key.as_str())
            .collect::<Vec<_>>(),
        ["apple", "application"]
    );
    assert!(d.exact("apply").is_some());
    assert!(d.prefix("app", 0).is_empty());
}

#[test]
fn resource_admission_scales_and_respects_memory_pressure() {
    let gib = 1024_u64.pow(3);
    assert_eq!(
        ResourceBudget::detect(96 * gib, 90 * gib, 24),
        ResourceBudget {
            workers: 22,
            resident_bytes: 64 * gib
        }
    );
    assert_eq!(
        ResourceBudget::detect(4 * gib, 3 * gib, 8).resident_bytes,
        gib / 4
    );
    assert_eq!(
        ResourceBudget::detect(96 * gib, 10 * gib, 24).resident_bytes,
        0
    );
    assert_eq!(ResourceBudget::detect(4 * gib, 3 * gib, 1).workers, 1);
}
