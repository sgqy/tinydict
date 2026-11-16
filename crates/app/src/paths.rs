use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) fn packages(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.is_file());
    Ok(preferred_packages(paths))
}

fn preferred_packages(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let replaced: HashSet<_> = paths
        .iter()
        .filter(|path| path.to_string_lossy().ends_with(".tdict.zst"))
        .map(|path| path.with_extension(""))
        .collect();
    paths.retain(|path| {
        path.to_string_lossy().ends_with(".tdict.zst")
            || (path.extension().is_some_and(|ext| ext == "tdict") && !replaced.contains(path))
    });
    paths.sort();
    paths
}

pub(crate) fn default_directory() -> PathBuf {
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let beside_executable = parent.join("data/dictionaries");
        if beside_executable.is_dir() {
            return beside_executable;
        }
    }
    PathBuf::from("data/dictionaries")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_package_replaces_only_its_matching_legacy_filename() {
        let files = [
            "books/b.tdict",
            "books/a.tdict",
            "books/a.tdict.zst",
            "books/c.tdict.zst",
            "books/source.jsonl.zst",
            "books/a.tdict.zst.partial",
        ]
        .map(PathBuf::from);
        assert_eq!(
            preferred_packages(files.to_vec()),
            ["books/a.tdict.zst", "books/b.tdict", "books/c.tdict.zst"].map(PathBuf::from)
        );
    }
}
