use crate::{DiskDictionary, PackageError};
use std::borrow::Cow;
use tinydict_core::{Dictionary, DictionaryInfo, Entry};

/// Select whether definition text is resident or fetched from compressed disk blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadMode {
    /// Load all definition text into memory.
    #[default]
    Full,
    /// Keep indexes in memory and read definition text on demand.
    LowMemory,
}

/// Load one independent dictionary in the selected memory mode.
///
/// # Errors
/// Fails for invalid packages, failed indexes, or unreadable/unwritable cache directories.
pub fn load_for_query(
    path: &std::path::Path,
    cache: &std::path::Path,
    mode: LoadMode,
) -> Result<LoadedDictionary, PackageError> {
    match mode {
        LoadMode::Full => crate::load_package(path).map(LoadedDictionary::Resident),
        LoadMode::LowMemory => crate::load_disk_package(path, cache).map(LoadedDictionary::OnDisk),
    }
}

/// A queryable dictionary with either resident text or resident indexes and compressed disk text.
#[derive(Debug)]
pub enum LoadedDictionary {
    /// All entries remain in memory.
    Resident(Dictionary),
    /// Only indexes remain in memory between lookups.
    OnDisk(DiskDictionary),
}

impl From<Dictionary> for LoadedDictionary {
    fn from(value: Dictionary) -> Self {
        Self::Resident(value)
    }
}

impl LoadedDictionary {
    /// Source identity and attribution.
    pub fn info(&self) -> &DictionaryInfo {
        match self {
            Self::Resident(d) => d.info(),
            Self::OnDisk(d) => d.info(),
        }
    }
    /// Number of normalized headwords.
    pub fn headword_count(&self) -> usize {
        match self {
            Self::Resident(d) => d.groups().len(),
            Self::OnDisk(d) => d.headword_count(),
        }
    }
    /// Original entry count.
    pub fn entry_count(&self) -> usize {
        match self {
            Self::Resident(d) => d.entry_count(),
            Self::OnDisk(d) => d.entry_count(),
        }
    }
    /// Original sense count.
    pub fn sense_count(&self) -> usize {
        match self {
            Self::Resident(d) => d.sense_count(),
            Self::OnDisk(d) => d.sense_count(),
        }
    }
    /// Estimated resident payload, excluding transient buffers and allocator overhead.
    pub fn estimated_bytes(&self) -> usize {
        match self {
            Self::Resident(d) => d.estimated_bytes(),
            Self::OnDisk(d) => d.estimated_bytes(),
        }
    }
    /// Whether definitions are read from compressed disk blocks.
    pub fn is_disk_backed(&self) -> bool {
        matches!(self, Self::OnDisk(_))
    }
    /// Original spelling available without disk I/O.
    pub fn headword(&self, id: usize) -> Option<&str> {
        match self {
            Self::Resident(d) => d.groups().get(id)?.entries.first().map(|e| e.word.as_str()),
            Self::OnDisk(d) => d.headword(id),
        }
    }
    /// Resolve a query using the resident index.
    pub fn exact(&self, query: &str) -> Option<usize> {
        match self {
            Self::Resident(d) => d.exact(query),
            Self::OnDisk(d) => d.exact(query),
        }
    }
    /// Suggest headwords using the resident index.
    pub fn prefix(&self, query: &str, limit: usize) -> Vec<usize> {
        match self {
            Self::Resident(d) => d.prefix(query, limit),
            Self::OnDisk(d) => d.prefix(query, limit),
        }
    }
    /// Access resident entries without cloning; disk-backed dictionaries return `None`.
    pub fn resident_entries(&self, id: usize) -> Option<&[Entry]> {
        match self {
            Self::Resident(d) => d.groups().get(id).map(|g| g.entries.as_slice()),
            Self::OnDisk(_) => None,
        }
    }
    /// Borrow resident text or read a selected disk block.
    ///
    /// # Errors
    /// Fails for unknown identifiers, unavailable disk files or corrupt compressed data.
    pub fn entries(&self, id: usize) -> Result<Cow<'_, [Entry]>, PackageError> {
        match self {
            Self::Resident(_) => self
                .resident_entries(id)
                .map(Cow::Borrowed)
                .ok_or_else(|| PackageError::Invalid("unknown headword identifier".into())),
            Self::OnDisk(d) => d.entries(id).map(Cow::Owned),
        }
    }
}
