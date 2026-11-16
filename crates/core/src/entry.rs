use serde::{Deserialize, Serialize};

/// Provenance and attribution for one independently installed dictionary.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DictionaryInfo {
    /// Stable installation identity, independent of release revision.
    pub id: String,
    /// Human-readable dictionary title.
    pub name: String,
    /// Upstream repository URL.
    pub source_url: String,
    /// Revision or local import description.
    pub revision: String,
    /// Original data license and required attribution.
    pub license: String,
}

/// A source entry, preserving original spelling and ordered senses.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Entry {
    /// Original source identifier.
    pub id: String,
    /// Original headword, including case and punctuation.
    pub word: String,
    /// Entry-level dialect and usage labels.
    pub labels: Vec<String>,
    /// Source editors and contributors, retained for attribution.
    pub contributors: Vec<String>,
    /// Definitions in their original order.
    pub senses: Vec<Sense>,
}

/// One definition with source-specific attribution and optional lexical details.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sense {
    /// Source sense identifier.
    pub id: String,
    /// English definition.
    pub definition: String,
    /// Part of speech supplied by the source.
    pub part_of_speech: Option<String>,
    /// Original abbreviated part of speech, where available.
    pub original_wordtype: Option<String>,
    /// Original examples, without substituting the headword for synonyms.
    pub examples: Vec<String>,
    /// Explicit source synonyms.
    pub synonyms: Vec<String>,
    /// Usage and dialect labels.
    pub labels: Vec<String>,
    /// Upstream subsource, such as webster1913 or oewn.
    pub source: String,
    /// Original synonym group identifier.
    pub synset_id: Option<String>,
}
