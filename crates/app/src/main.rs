mod app;
mod cli;
mod worker;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    about = "Offline English dictionaries, independently indexed and fully loaded into memory"
)]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long, default_value = "")]
    query: String,
    /// Print an exact lookup without opening a window.
    #[arg(long)]
    lookup: Option<String>,
    /// Measure full-memory lookups and prefix queries without opening a window.
    #[arg(long)]
    benchmark: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let directory = args.data_dir.unwrap_or_else(default_directory);
    if args.benchmark || args.lookup.is_some() {
        return cli::run(&directory, args.lookup.as_deref());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([800.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TinyDict",
        options,
        Box::new(move |creation| {
            Ok(Box::new(app::TinyDict::new(
                creation, directory, args.query,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("cannot start desktop window: {error}"))
}

fn default_directory() -> PathBuf {
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
