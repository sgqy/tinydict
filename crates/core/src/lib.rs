//! Immutable, independent English dictionaries with fully resident entries and FST indexes.

mod dictionary;
mod entry;
mod resources;

pub use dictionary::{BuildError, Dictionary, WordGroup, normalize};
pub use entry::{DictionaryInfo, Entry, Sense};
pub use resources::ResourceBudget;
