#[path = "../cli.rs"]
mod cli;
#[path = "../paths.rs"]
mod paths;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Inspect or benchmark fully resident TinyDict dictionaries")]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long, conflicts_with = "benchmark")]
    lookup: Option<String>,
    #[arg(long)]
    benchmark: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let directory = args.data_dir.unwrap_or_else(paths::default_directory);
    cli::run(&directory, args.lookup.as_deref())
}
