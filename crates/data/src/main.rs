use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;
use tinydict_core::DictionaryInfo;
use tinydict_data::{Source, import_source, write_package};

#[derive(Parser)]
#[command(about = "Build an independent TinyDict package from a local English dictionary")]
struct Args {
    #[arg(value_enum)]
    source: Source,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    license: PathBuf,
    #[arg(long, default_value = "local")]
    revision: String,
    #[arg(long)]
    workers: Option<usize>,
}
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (id, name, url) = match args.source {
        Source::Cloudbytes => (
            "cloudbytes",
            "CloudBytes English Dictionary",
            "https://github.com/CloudBytes-Academy/English-Dictionary-Open-Source",
        ),
        Source::Wordset => (
            "wordset",
            "Wordset Dictionary",
            "https://github.com/wordset/wordset-dictionary",
        ),
    };
    let license = std::fs::read_to_string(&args.license).context("reading source data license")?;
    anyhow::ensure!(!license.trim().is_empty(), "source data license is empty");
    let info = DictionaryInfo {
        id: id.into(),
        name: name.into(),
        source_url: url.into(),
        revision: args.revision,
        license,
    };
    let workers = args.workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .clamp(1, 24)
            .saturating_sub(2)
            .max(1)
    });
    let started = Instant::now();
    let (dictionary, report) =
        import_source(args.source, &args.input, info, workers).context("importing dictionary")?;
    println!(
        "{}: {} entries, {} senses, {} headwords, {:.1} MiB estimated resident; skipped {} entries / {} empty senses",
        dictionary.info().name,
        dictionary.entry_count(),
        dictionary.sense_count(),
        dictionary.groups().len(),
        dictionary.estimated_bytes() as f64 / 1048576.0,
        report.skipped_entries,
        report.skipped_senses
    );
    write_package(&args.output, &dictionary, report).context("writing package")?;
    println!(
        "Saved {} in {:.2}s",
        args.output.display(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
