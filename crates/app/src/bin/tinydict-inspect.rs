#[path = "../cli.rs"]
mod cli;
#[path = "../paths.rs"]
mod paths;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Inspect or benchmark TinyDict dictionaries in either memory mode")]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long, conflicts_with = "benchmark")]
    lookup: Option<String>,
    #[arg(long)]
    benchmark: bool,
    /// Keep indexes resident and read definition text from compressed disk blocks.
    #[arg(long)]
    low_memory: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let directory = args.data_dir.unwrap_or_else(paths::default_directory);
    cli::run(
        &directory,
        args.lookup.as_deref(),
        if args.low_memory {
            tinydict_data::LoadMode::LowMemory
        } else {
            tinydict_data::LoadMode::Full
        },
    )
}
