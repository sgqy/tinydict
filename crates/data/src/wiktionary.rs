use crate::PackageError;
use crate::import::{ParsedEntries, reader};
use serde::Deserialize;
use std::path::Path;
use tinydict_core::{Entry, Sense};

#[derive(Deserialize)]
struct Row {
    word: Option<String>,
    lang_code: Option<String>,
    redirect: Option<String>,
    pos: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    raw_tags: Vec<String>,
    #[serde(default)]
    senses: Vec<Definition>,
}

#[derive(Deserialize)]
struct Definition {
    #[serde(default)]
    glosses: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    raw_tags: Vec<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    examples: Vec<Example>,
    #[serde(default)]
    synonyms: Vec<Synonym>,
}

#[derive(Deserialize)]
struct Example {
    text: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
}

#[derive(Deserialize)]
struct Synonym {
    word: String,
}

pub(crate) fn read(path: &Path, source: &str) -> Result<ParsedEntries, PackageError> {
    let mut input = reader(path)?;
    let mut line = String::new();
    let mut line_number = 0;
    let mut parsed = ParsedEntries::default();
    // Reuse the JSONL buffer. Unknown translations/media are skipped by serde,
    // without constructing a large intermediate JSON tree.
    while input.read_line(&mut line)? != 0 {
        line_number += 1;
        if !line.trim().is_empty() {
            let row: Row = serde_json::from_str(&line).map_err(|error| {
                PackageError::Invalid(format!("{}:{line_number}: {error}", path.display()))
            })?;
            if row.redirect.is_some() {
                parsed.redirects += 1;
            } else if row.lang_code.as_deref().is_some_and(|lang| lang != "en") {
                parsed.non_english += 1;
            } else if let (Some("en"), Some(word)) = (row.lang_code.as_deref(), row.word) {
                let id = format!(
                    "{}:{line_number}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                parsed.entries.push(Entry {
                    id: id.clone(),
                    word,
                    labels: row.tags.into_iter().chain(row.raw_tags).collect(),
                    contributors: vec![],
                    senses: row
                        .senses
                        .into_iter()
                        .enumerate()
                        .map(|(index, sense)| Sense {
                            id: format!("{id}:{}", index + 1),
                            // Nested glosses carry parent context; preserve their order.
                            definition: sense
                                .glosses
                                .into_iter()
                                .filter(|text| !text.trim().is_empty())
                                .collect::<Vec<_>>()
                                .join("\n"),
                            part_of_speech: row.pos.clone(),
                            examples: sense
                                .examples
                                .into_iter()
                                .filter_map(|example| {
                                    let mut text =
                                        example.text.filter(|text| !text.trim().is_empty())?;
                                    if let Some(reference) =
                                        example.reference.filter(|text| !text.trim().is_empty())
                                    {
                                        text.push_str("\n— ");
                                        text.push_str(&reference);
                                    }
                                    Some(text)
                                })
                                .collect(),
                            // Only sense-scoped synonyms are safe to associate with this definition.
                            synonyms: sense
                                .synonyms
                                .into_iter()
                                .map(|synonym| synonym.word)
                                .filter(|word| !word.trim().is_empty())
                                .collect(),
                            labels: sense
                                .tags
                                .into_iter()
                                .chain(sense.raw_tags)
                                .chain(sense.topics)
                                .collect(),
                            source: source.into(),
                            ..Sense::default()
                        })
                        .collect(),
                });
            } else {
                return Err(PackageError::Invalid(format!(
                    "{}:{line_number}: lexical entry lacks word or lang_code",
                    path.display()
                )));
            }
        }
        line.clear();
    }
    Ok(parsed)
}
