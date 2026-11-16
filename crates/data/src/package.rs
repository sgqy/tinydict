use crate::ImportReport;
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::Path;
use tinydict_core::{Dictionary, DictionaryInfo, Entry};

const SCHEMA_VERSION: u32 = 1;

/// Versioned metadata with source attribution, input hashes and import accounting.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PackageManifest {
    /// Package format version.
    pub schema_version: u32,
    /// Independent dictionary identity and attribution.
    pub dictionary: DictionaryInfo,
    /// Original entries stored in this package.
    pub entry_count: usize,
    /// Original definitions stored in this package.
    pub sense_count: usize,
    /// Estimated resident payload bytes, excluding allocator and renderer overhead.
    pub estimated_bytes: u64,
    /// Import accounting and source digests.
    pub import: ImportReport,
}

/// Recoverable source, package and filesystem failures.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    /// Filesystem or input stream failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Malformed serialized data.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// SQLite failure.
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// Invalid entries or index failure.
    #[error(transparent)]
    Build(#[from] tinydict_core::BuildError),
    /// Unsupported or inconsistent package.
    #[error("invalid dictionary package: {0}")]
    Invalid(String),
}

/// Publish a new package atomically without overwriting an existing installation.
///
/// # Errors
/// Fails for existing destinations, failed writes, or serialization errors.
pub fn write_package(
    path: &Path,
    dictionary: &Dictionary,
    report: ImportReport,
) -> Result<(), PackageError> {
    if path.exists() {
        return Err(PackageError::Invalid(format!(
            "output already exists: {}",
            path.display()
        )));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tdict.partial");
    File::options()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    if let Err(error) = write_database(&temporary, dictionary, report) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    // Hard-link publication is atomic and refuses to replace an existing destination.
    let result = fs::hard_link(&temporary, path);
    let _ = fs::remove_file(&temporary);
    result?;
    Ok(())
}

fn write_database(
    path: &Path,
    dictionary: &Dictionary,
    report: ImportReport,
) -> Result<(), PackageError> {
    let mut connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE metadata (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL); CREATE TABLE entries (id INTEGER PRIMARY KEY, json TEXT NOT NULL);")?;
    let metadata = PackageManifest {
        schema_version: SCHEMA_VERSION,
        dictionary: dictionary.info().clone(),
        entry_count: dictionary.entry_count(),
        sense_count: dictionary.sense_count(),
        estimated_bytes: dictionary.estimated_bytes() as u64,
        import: report,
    };
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO metadata VALUES (1, ?1)",
        [serde_json::to_string(&metadata)?],
    )?;
    {
        let mut insert = transaction.prepare("INSERT INTO entries (json) VALUES (?1)")?;
        for entry in dictionary.groups().iter().flat_map(|group| &group.entries) {
            insert.execute(params![serde_json::to_string(entry)?])?;
        }
    }
    transaction.commit()?;
    connection.close().map_err(|(_, error)| error)?;
    Ok(())
}

fn open_readonly(path: &Path) -> Result<Connection, PackageError> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}
fn manifest(connection: &Connection) -> Result<PackageManifest, PackageError> {
    let json: String =
        connection.query_row("SELECT json FROM metadata WHERE id=1", [], |row| row.get(0))?;
    let metadata: PackageManifest = serde_json::from_str(&json)?;
    if metadata.schema_version != SCHEMA_VERSION {
        return Err(PackageError::Invalid(format!(
            "unsupported schema {}",
            metadata.schema_version
        )));
    }
    Ok(metadata)
}

/// Read metadata without loading the dictionary body.
///
/// # Errors
/// Fails for missing, malformed or unsupported packages.
pub fn inspect_package(path: &Path) -> Result<PackageManifest, PackageError> {
    manifest(&open_readonly(path)?)
}

/// Load all entries and the index; the database connection closes before returning.
///
/// # Errors
/// Fails for unreadable packages, invalid entries or mismatched metadata counts.
pub fn load_package(path: &Path) -> Result<Dictionary, PackageError> {
    let connection = open_readonly(path)?;
    let metadata = manifest(&connection)?;
    let mut statement = connection.prepare("SELECT json FROM entries ORDER BY id")?;
    let mut rows = statement.query([])?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let json = row
            .get_ref(0)?
            .as_str()
            .map_err(|error| PackageError::Invalid(error.to_string()))?;
        entries.push(serde_json::from_str::<Entry>(json)?);
    }
    let dictionary = Dictionary::build(metadata.dictionary, entries)?;
    if dictionary.entry_count() != metadata.entry_count
        || dictionary.sense_count() != metadata.sense_count
    {
        return Err(PackageError::Invalid(
            "entry or sense count does not match manifest".into(),
        ));
    }
    Ok(dictionary)
}
