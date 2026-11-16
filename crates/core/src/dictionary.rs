use fst::{Automaton, IntoStreamer, Streamer};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

use crate::{DictionaryInfo, Entry};

/// Normalize lookup keys without removing meaningful accents or punctuation.
pub fn normalize(text: &str) -> String {
    text.trim().nfc().case_fold().nfc().collect()
}

/// Entries sharing one normalized lookup key, without merging their meanings.
#[derive(Debug)]
pub struct WordGroup {
    /// Normalized lookup key.
    pub key: String,
    /// All original entries, in source order.
    pub entries: Vec<Entry>,
}

/// A fully resident dictionary. Queries perform no file access or deserialization.
#[derive(Debug)]
pub struct Dictionary {
    info: DictionaryInfo,
    groups: Vec<WordGroup>,
    index: fst::Map<Vec<u8>>,
    entry_count: usize,
    sense_count: usize,
    resident_bytes: usize,
}

/// Invalid source entries or an index construction failure.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// A source entry has no usable word or definition.
    #[error("invalid entry {0}: empty headword or senses")]
    InvalidEntry(String),
    /// The finite-state index could not be constructed.
    #[error("cannot build dictionary index: {0}")]
    Index(#[from] fst::Error),
}

impl Dictionary {
    /// Build one dictionary, preserving normalized collisions and source sense order.
    ///
    /// # Errors
    /// Returns an error for empty entries or a failed index build.
    pub fn build(info: DictionaryInfo, entries: Vec<Entry>) -> Result<Self, BuildError> {
        let mut keyed = Vec::with_capacity(entries.len());
        for entry in entries {
            let key = normalize(&entry.word);
            if key.is_empty()
                || entry.senses.is_empty()
                || entry
                    .senses
                    .iter()
                    .any(|sense| sense.definition.trim().is_empty())
            {
                return Err(BuildError::InvalidEntry(entry.id));
            }
            keyed.push((key, entry));
        }
        // Stable sorting retains upstream order for case-folding collisions.
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let entry_count = keyed.len();
        let mut groups: Vec<WordGroup> = Vec::new();
        for (key, entry) in keyed {
            if let Some(group) = groups.last_mut().filter(|group| group.key == key) {
                group.entries.push(entry);
            } else {
                groups.push(WordGroup {
                    key,
                    entries: vec![entry],
                });
            }
        }
        let mut builder = fst::MapBuilder::memory();
        for (id, group) in groups.iter().enumerate() {
            builder.insert(&group.key, id as u64)?;
        }
        let index = fst::Map::new(builder.into_inner()?)?;
        let sense_count = groups
            .iter()
            .flat_map(|group| &group.entries)
            .map(|entry| entry.senses.len())
            .sum();
        let resident_bytes = estimate_bytes(&groups) + index.as_fst().as_bytes().len();
        Ok(Self {
            info,
            groups,
            index,
            entry_count,
            sense_count,
            resident_bytes,
        })
    }

    /// Source identity and attribution.
    pub fn info(&self) -> &DictionaryInfo {
        &self.info
    }

    /// Groups ordered by normalized headword.
    pub fn groups(&self) -> &[WordGroup] {
        &self.groups
    }

    /// Number of original source entries, including case-folding collisions.
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Number of original definitions.
    pub fn sense_count(&self) -> usize {
        self.sense_count
    }

    /// Estimated owned entry and index bytes, excluding allocator overhead and renderer memory.
    pub fn estimated_bytes(&self) -> usize {
        self.resident_bytes
    }

    /// Resolve an original query to a dictionary-local group identifier.
    pub fn exact(&self, query: &str) -> Option<usize> {
        self.index.get(normalize(query)).map(|id| id as usize)
    }

    /// Return at most `limit` matching groups; empty input returns no suggestions.
    pub fn prefix(&self, query: &str, limit: usize) -> Vec<usize> {
        let key = normalize(query);
        if key.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut stream = self
            .index
            .search(fst::automaton::Str::new(&key).starts_with())
            .into_stream();
        let mut ids = Vec::with_capacity(limit.min(100));
        while ids.len() < limit {
            let Some((_, id)) = stream.next() else {
                break;
            };
            ids.push(id as usize);
        }
        ids
    }
}

fn estimate_bytes(groups: &[WordGroup]) -> usize {
    let mut bytes = std::mem::size_of_val(groups);
    for group in groups {
        bytes += group.key.capacity() + group.entries.capacity() * size_of::<Entry>();
        for entry in &group.entries {
            bytes += entry.word.capacity() + entry.id.capacity();
            bytes += strings_bytes(&entry.labels) + strings_bytes(&entry.contributors);
            bytes += entry.senses.capacity() * size_of::<crate::Sense>();
            for sense in &entry.senses {
                bytes +=
                    sense.id.capacity() + sense.definition.capacity() + sense.source.capacity();
                bytes += strings_bytes(&sense.examples)
                    + strings_bytes(&sense.synonyms)
                    + strings_bytes(&sense.labels);
                for value in [
                    &sense.part_of_speech,
                    &sense.original_wordtype,
                    &sense.synset_id,
                ]
                .into_iter()
                .flatten()
                {
                    bytes += value.capacity();
                }
            }
        }
    }
    bytes
}

fn strings_bytes(values: &Vec<String>) -> usize {
    values.capacity() * size_of::<String>() + values.iter().map(String::capacity).sum::<usize>()
}
