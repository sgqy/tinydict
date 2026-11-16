use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;
use tinydict_data::{inspect_package, load_package, write_package};

#[derive(Parser)]
#[command(about = "Repack a TinyDict dictionary as .tdict.zst or legacy .tdict")]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    /// Reload and compare every entry, attribution and example after writing.
    #[arg(long)]
    verify: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.output.to_string_lossy().ends_with(".tdict")
            || args.output.to_string_lossy().ends_with(".tdict.zst"),
        "output must end in .tdict or .tdict.zst"
    );
    anyhow::ensure!(!args.output.exists(), "output already exists");
    let started = Instant::now();
    let metadata = inspect_package(&args.input).context("reading package manifest")?;
    let dictionary = load_package(&args.input).context("loading package")?;
    write_package(&args.output, &dictionary, metadata.import.clone()).context("writing package")?;
    if args.verify {
        let output_manifest = inspect_package(&args.output)?;
        let loaded = load_package(&args.output).context("verifying output package")?;
        anyhow::ensure!(
            serde_json::to_value(loaded.info())? == serde_json::to_value(dictionary.info())?
                && serde_json::to_value(output_manifest.import)?
                    == serde_json::to_value(metadata.import)?
                && loaded.groups().len() == dictionary.groups().len()
                && loaded
                    .groups()
                    .iter()
                    .zip(dictionary.groups())
                    .all(|(a, b)| { a.key == b.key && a.entries == b.entries }),
            "output does not preserve all dictionary content and metadata"
        );
        println!("Verified every entry, sense, example and source notice.");
    }
    println!(
        "{}: {} entries, {} senses; {:.1} -> {:.1} MiB in {:.2}s",
        dictionary.info().name,
        dictionary.entry_count(),
        dictionary.sense_count(),
        std::fs::metadata(&args.input)?.len() as f64 / 1048576.0,
        std::fs::metadata(&args.output)?.len() as f64 / 1048576.0,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
