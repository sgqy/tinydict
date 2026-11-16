use crate::PackageError;
use flate2::read::MultiGzDecoder;
use rayon::prelude::*;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use tinydict_core::{Dictionary, DictionaryInfo, Entry, Sense};

/// English data sources installed as independent dictionaries.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Source {
    /// CloudBytes v2 JSONL, optionally gzip compressed.
    Cloudbytes,
    /// Wordset's per-letter JSON directory or a single JSON file.
    Wordset,
}

/// Input checksums and accounting for an import.
#[derive(Clone, Debug, Default, Deserialize, serde::Serialize)]
pub struct ImportReport {
    /// Original entries before validation.
    pub input_entries: usize,
    /// Entries rejected for empty headwords or lack of valid definitions.
    pub skipped_entries: usize,
    /// Empty definitions rejected, retaining valid sibling senses.
    pub skipped_senses: usize,
    /// Source filenames and SHA-256 hashes in deterministic order.
    pub input_sha256: Vec<(String, String)>,
}

/// Import files with bounded parallelism and preserve original entry and sense order.
///
/// # Errors
/// Fails on unreadable files, malformed JSON, no usable entries, or index construction errors.
pub fn import_source(
    source: Source,
    input: &Path,
    info: DictionaryInfo,
    workers: usize,
) -> Result<(Dictionary, ImportReport), PackageError> {
    let files = if input.is_dir() {
        let mut files = fs::read_dir(input)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        files.retain(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().is_some_and(|name| name != "source.json")
        });
        files.sort();
        files
    } else {
        vec![input.to_path_buf()]
    };
    if files.is_empty() {
        return Err(PackageError::Invalid("no source files found".into()));
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.clamp(1, 22))
        .build()
        .map_err(|error| PackageError::Invalid(error.to_string()))?;
    let parts = pool.install(|| {
        files
            .par_iter()
            .map(|path| {
                let entries = match source {
                    Source::Cloudbytes => cloudbytes(path),
                    Source::Wordset => wordset(path),
                }?;
                let mut file = File::open(path)?;
                let mut hash = Sha256::new();
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hash.update(&buffer[..count]);
                }
                Ok((
                    entries,
                    (
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        format!("{:x}", hash.finalize()),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, PackageError>>()
    })?;
    let mut report = ImportReport::default();
    let mut entries = Vec::new();
    for (part, digest) in parts {
        report.input_sha256.push(digest);
        for mut entry in part {
            report.input_entries += 1;
            let count = entry.senses.len();
            entry
                .senses
                .retain(|sense| !sense.definition.trim().is_empty());
            report.skipped_senses += count - entry.senses.len();
            if entry.word.trim().is_empty() || entry.senses.is_empty() {
                report.skipped_entries += 1;
            } else {
                entries.push(entry);
            }
        }
    }
    if entries.is_empty() {
        return Err(PackageError::Invalid(
            "source contains no usable entries".into(),
        ));
    }
    Ok((Dictionary::build(info, entries)?, report))
}

fn reader(path: &Path) -> Result<Box<dyn BufRead>, PackageError> {
    let file = File::open(path)?;
    if path.extension().is_some_and(|ext| ext == "gz") {
        Ok(Box::new(BufReader::new(MultiGzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

#[derive(Deserialize)]
struct CloudRow {
    id: String,
    word: String,
    pos: Option<String>,
    wordtype: Option<String>,
    definition: Option<String>,
    #[serde(default)]
    examples: Vec<String>,
    synset_id: Option<String>,
    source: String,
}

fn cloudbytes(path: &Path) -> Result<Vec<Entry>, PackageError> {
    let mut entries = Vec::new();
    for (line_no, line) in reader(path)?.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: CloudRow = serde_json::from_str(&line).map_err(|error| {
            PackageError::Invalid(format!("{}:{}: {error}", path.display(), line_no + 1))
        })?;
        entries.push(Entry {
            id: row.id.clone(),
            word: row.word,
            labels: vec![],
            contributors: vec![],
            senses: vec![Sense {
                id: row.id,
                definition: row.definition.unwrap_or_default(),
                part_of_speech: row.pos,
                original_wordtype: row.wordtype,
                examples: row.examples,
                source: row.source,
                synset_id: row.synset_id,
                ..Sense::default()
            }],
        });
    }
    Ok(entries)
}

#[derive(Deserialize)]
struct WordsetEntry {
    word: String,
    wordset_id: String,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    contributors: Vec<String>,
    #[serde(default)]
    editors: Vec<String>,
    #[serde(default)]
    meanings: Vec<Meaning>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Label {
    Text(String),
    Named {
        name: String,
        #[serde(default)]
        is_dialect: bool,
    },
}
impl Label {
    fn text(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Named {
                name,
                is_dialect: true,
            } => format!("{name} (dialect)"),
            Self::Named {
                name,
                is_dialect: false,
            } => name,
        }
    }
}

#[derive(Deserialize)]
struct Meaning {
    id: String,
    def: Option<String>,
    speech_part: Option<String>,
    example: Option<String>,
    #[serde(default)]
    synonyms: Vec<Option<String>>,
    #[serde(default)]
    labels: Vec<Label>,
}

struct WordsetVisitor;
impl<'de> Visitor<'de> for WordsetVisitor {
    type Value = Vec<Entry>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Wordset object keyed by headword")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        // Parse one entry at a time, avoiding an intermediate JSON Value tree.
        while let Some((_key, row)) = map.next_entry::<String, WordsetEntry>()? {
            let mut contributors = row.contributors;
            for editor in row.editors {
                if !contributors.contains(&editor) {
                    contributors.push(editor);
                }
            }
            entries.push(Entry {
                id: row.wordset_id,
                word: row.word,
                labels: row.labels.into_iter().map(Label::text).collect(),
                contributors,
                senses: row
                    .meanings
                    .into_iter()
                    .map(|meaning| Sense {
                        id: meaning.id,
                        definition: meaning.def.unwrap_or_default(),
                        part_of_speech: meaning.speech_part,
                        examples: meaning.example.into_iter().collect(),
                        synonyms: meaning.synonyms.into_iter().flatten().collect(),
                        labels: meaning.labels.into_iter().map(Label::text).collect(),
                        source: "wordset".into(),
                        ..Sense::default()
                    })
                    .collect(),
            });
        }
        Ok(entries)
    }
}
fn wordset(path: &Path) -> Result<Vec<Entry>, PackageError> {
    let mut deserializer = serde_json::Deserializer::from_reader(reader(path)?);
    let entries = (&mut deserializer).deserialize_map(WordsetVisitor)?;
    deserializer.end()?;
    Ok(entries)
}
