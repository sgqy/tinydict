//! Source adapters and zstd/SQLite packages that load into fully resident dictionaries.
mod compressed;
mod disk;
mod import;
mod loaded;
mod package;
mod wiktionary;
pub use disk::{DiskDictionary, load_disk_package};
pub use import::{ImportReport, Source, import_source};
pub use loaded::{LoadMode, LoadedDictionary, load_for_query};
pub use package::{PackageError, PackageManifest, inspect_package, load_package, write_package};
