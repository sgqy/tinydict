#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod paths;
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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let directory = args.data_dir.unwrap_or_else(paths::default_directory);
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
