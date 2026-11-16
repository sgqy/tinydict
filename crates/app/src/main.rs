#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod paths;
mod profile;
mod worker;

use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Copy, clap::ValueEnum)]
enum Renderer {
    Glow,
    Wgpu,
}

#[derive(Parser)]
#[command(
    about = "Offline English dictionaries, independently indexed and fully loaded into memory"
)]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long, default_value = "")]
    query: String,
    /// Record a bounded continuous-repaint diagnostic run as JSON, then exit.
    #[arg(long)]
    profile: Option<PathBuf>,
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=60))]
    profile_seconds: u64,
    /// Feed reproducible pointer-movement events into the diagnostic run.
    #[arg(long, requires = "profile")]
    profile_motion: bool,
    /// Disable presentation synchronization for a diagnostic comparison.
    #[arg(long)]
    no_vsync: bool,
    #[arg(long, value_enum, default_value_t = Renderer::Glow)]
    renderer: Renderer,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let directory = args.data_dir.unwrap_or_else(paths::default_directory);
    // Keep diagnostic window/layout state separate from the user's normal session.
    let profile_storage = args
        .profile
        .as_ref()
        .map(|path| path.with_extension("state.ron"));
    let probe = args
        .profile
        .map(|path| profile::FrameProbe::new(path, args.profile_seconds, args.profile_motion))
        .transpose()?;
    let mut options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([800.0, 560.0]),
        persist_window: profile_storage.is_none(),
        persistence_path: profile_storage,
        ..Default::default()
    };
    options.glow_options.vsync = !args.no_vsync;
    options.renderer = match args.renderer {
        Renderer::Glow => eframe::Renderer::Glow,
        Renderer::Wgpu => eframe::Renderer::Wgpu,
    };
    options.wgpu_options.surface = eframe::egui_wgpu::SurfaceConfig::LOW_LATENCY;
    if args.no_vsync {
        options.wgpu_options.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
    }
    if cfg!(target_os = "windows")
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup
    {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    }
    eframe::run_native(
        "TinyDict",
        options,
        Box::new(move |creation| {
            Ok(Box::new(app::TinyDict::new(
                creation, directory, args.query, probe,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("cannot start desktop window: {error}"))
}
