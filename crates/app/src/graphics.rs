use anyhow::Context;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Renderer {
    #[default]
    Auto,
    Glow,
    Wgpu,
    Vulkan,
}

impl Renderer {
    fn argument(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Glow => "glow",
            Self::Wgpu => "wgpu",
            Self::Vulkan => "vulkan",
        }
    }

    fn backend(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Glow => "OpenGL",
            Self::Vulkan => "Vulkan",
            Self::Wgpu if cfg!(target_os = "windows") => "Direct3D 12",
            Self::Wgpu if cfg!(target_os = "macos") => "Metal",
            Self::Wgpu => "Vulkan",
        }
    }
}

fn candidates(requested: Renderer) -> Vec<Renderer> {
    let mut result = Vec::new();
    for candidate in [requested, Renderer::Wgpu, Renderer::Glow] {
        if candidate != Renderer::Auto
            && !result
                .iter()
                .any(|r: &Renderer| r.backend() == candidate.backend())
        {
            result.push(candidate);
        }
    }
    result
}

pub(crate) fn configure(renderer: Renderer, options: &mut eframe::NativeOptions) {
    options.renderer = if renderer == Renderer::Glow {
        eframe::Renderer::Glow
    } else {
        eframe::Renderer::Wgpu
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = if renderer == Renderer::Vulkan {
            eframe::wgpu::Backends::VULKAN
        } else if cfg!(target_os = "windows") {
            eframe::wgpu::Backends::DX12
        } else if cfg!(target_os = "macos") {
            eframe::wgpu::Backends::METAL
        } else {
            eframe::wgpu::Backends::VULKAN
        };
    }
}

enum Startup {
    Ready,
    Exited(ExitStatus),
    TimedOut,
}

fn await_startup(child: &mut Child, ready: &Path, timeout: Duration) -> std::io::Result<Startup> {
    let deadline = Instant::now() + timeout;
    loop {
        if ready.is_file() {
            return Ok(Startup::Ready);
        }
        if let Some(status) = child.try_wait()? {
            return Ok(Startup::Exited(status));
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Ok(Startup::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Keep native driver faults in a child: only retry failures before successful rendering.
pub(crate) fn launch(requested: Renderer) -> anyhow::Result<()> {
    let temporary = tempfile::tempdir().context("creating graphics startup handshake")?;
    let directory = eframe::storage_dir("TinyDict").unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&directory)?;
    let log_path = directory.join("graphics-startup.log");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let executable = std::env::current_exe()?;
    for renderer in candidates(requested) {
        let ready = temporary.path().join(renderer.argument());
        writeln!(
            log,
            "Starting {} in an isolated process",
            renderer.backend()
        )?;
        let mut command = Command::new(&executable);
        command
            .args(std::env::args_os().skip(1))
            .arg("--graphics-backend")
            .arg(renderer.argument())
            .arg("--graphics-ready-file")
            .arg(&ready)
            .stdin(Stdio::null())
            .stderr(Stdio::from(log.try_clone()?));
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // No extra console; the child's ordinary GUI window remains visible.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .context("starting isolated graphics process")?;
        match await_startup(&mut child, &ready, Duration::from_secs(30))? {
            Startup::Ready => {
                writeln!(log, "{} rendered successfully", renderer.backend())?;
                let status = child.wait()?;
                anyhow::ensure!(
                    status.success(),
                    "desktop process exited with {status}; see {}",
                    log_path.display()
                );
                return Ok(());
            }
            Startup::Exited(status) if status.success() => return Ok(()),
            Startup::Exited(status) => writeln!(
                log,
                "{} failed before rendering: {status}; trying fallback",
                renderer.backend()
            )?,
            Startup::TimedOut => writeln!(
                log,
                "{} timed out before rendering; trying fallback",
                renderer.backend()
            )?,
        }
    }
    anyhow::bail!("no working graphics backend; see {}", log_path.display())
}

pub(crate) struct StartupSignal {
    path: PathBuf,
    frames: usize,
    last_frame: Option<u64>,
}

impl StartupSignal {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            frames: 0,
            last_frame: None,
        }
    }

    pub(crate) fn frame(&mut self, ctx: &eframe::egui::Context) {
        let frame = ctx.cumulative_frame_nr();
        if self.last_frame == Some(frame) {
            return;
        }
        self.last_frame = Some(frame);
        self.frames += 1;
        if self.frames == 3 {
            // Two earlier UI passes have completed presentation before we report readiness.
            if let Err(error) = fs::write(&self.path, b"ready") {
                log::error!("cannot report graphics readiness: {error}");
            }
        } else if self.frames < 3 {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_selection_prefers_platform_backend_and_keeps_gl_fallback() {
        assert_eq!(candidates(Renderer::Auto), [Renderer::Wgpu, Renderer::Glow]);
        let list = candidates(Renderer::Vulkan);
        assert_eq!(list.first(), Some(&Renderer::Vulkan));
        assert_eq!(list.last(), Some(&Renderer::Glow));
        for (index, item) in list.iter().enumerate() {
            assert!(
                !list[..index]
                    .iter()
                    .any(|prior| prior.backend() == item.backend())
            );
        }
    }

    #[test]
    fn child_fixture() {
        let Ok(mode) = std::env::var("TINYDICT_TEST_STARTUP") else {
            return;
        };
        match mode.as_str() {
            "fail" => std::process::exit(42),
            "ready" => {
                fs::write(std::env::var_os("TINYDICT_TEST_READY").unwrap(), b"ready").unwrap()
            }
            "hang" => std::thread::sleep(Duration::from_secs(60)),
            _ => panic!("unknown fixture mode"),
        }
    }

    #[test]
    fn multiple_layout_passes_do_not_report_graphics_readiness_early() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ready");
        let mut signal = StartupSignal::new(path.clone());
        let ctx = eframe::egui::Context::default();
        for _ in 0..6 {
            signal.frame(&ctx);
        }
        assert!(!path.exists());
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |ui| signal.frame(ui.ctx()));
            output.textures_delta.clear();
        }
        assert!(path.exists());
    }

    #[test]
    fn child_failure_readiness_and_timeout_are_distinguished() {
        let temp = tempfile::tempdir().unwrap();
        for mode in ["fail", "ready", "hang"] {
            let ready = temp.path().join(mode);
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "graphics::tests::child_fixture", "--nocapture"])
                .env("TINYDICT_TEST_STARTUP", mode)
                .env("TINYDICT_TEST_READY", &ready)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let result = await_startup(&mut child, &ready, Duration::from_secs(2)).unwrap();
            match (mode, result) {
                ("fail", Startup::Exited(status)) => assert_eq!(status.code(), Some(42)),
                ("ready", Startup::Ready) => {
                    assert!(child.wait().unwrap().success());
                }
                ("hang", Startup::TimedOut) => {
                    assert!(child.try_wait().unwrap().is_some());
                }
                _ => panic!("wrong child startup classification for {mode}"),
            }
        }
    }
}
