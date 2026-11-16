//! Source adapters and SQLite packages that load into fully resident dictionaries.
mod import;
mod package;
mod wiktionary;
pub use import::{ImportReport, Source, import_source};
pub use package::{PackageError, PackageManifest, inspect_package, load_package, write_package};
