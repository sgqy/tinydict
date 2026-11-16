use std::path::{Path, PathBuf};

pub(crate) fn packages(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "tdict"));
    paths.sort();
    Ok(paths)
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
