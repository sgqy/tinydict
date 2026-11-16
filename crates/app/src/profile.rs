use eframe::egui;
use serde::Serialize;
use std::fs::{self, File};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Optional instrumentation; normal application runs never collect or write frame samples.
pub(crate) struct FrameProbe {
    path: PathBuf,
    duration: Duration,
    motion: bool,
    started: Option<Instant>,
    previous: Option<Instant>,
    capture_previous: bool,
    previous_focused: bool,
    ui_ms: f64,
    samples: Vec<Sample>,
    finished: bool,
    ticks: usize,
}

#[derive(Serialize)]
struct Sample {
    ui_ms: f64,
    cpu_including_render_ms: f64,
    frame_interval_ms: f64,
    focused: bool,
}

#[derive(Debug, Serialize)]
struct Distribution {
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
}

impl FrameProbe {
    pub fn new(path: PathBuf, seconds: u64, motion: bool) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let log = File::create(path.with_extension("log"))?;
        env_logger::Builder::new()
            .filter_module("egui_glow", log::LevelFilter::Debug)
            .filter_module("eframe", log::LevelFilter::Info)
            .filter_module("egui_wgpu", log::LevelFilter::Debug)
            .target(env_logger::Target::Pipe(Box::new(log)))
            .try_init()?;
        Ok(Self {
            path,
            duration: Duration::from_secs(seconds),
            motion,
            started: None,
            previous: None,
            capture_previous: false,
            previous_focused: false,
            ui_ms: 0.0,
            samples: Vec::new(),
            finished: false,
            ticks: 0,
        })
    }

    pub fn input(&mut self, input: &mut egui::RawInput) {
        if self.motion {
            let step = self.ticks as f32;
            input.events.push(egui::Event::PointerMoved(egui::pos2(
                130.0 + (step * 0.08).sin() * 100.0,
                350.0 + (step * 0.05).cos() * 130.0,
            )));
        }
        self.ticks += 1;
    }

    pub fn record_ui(&mut self, elapsed: Duration) {
        self.ui_ms += elapsed.as_secs_f64() * 1000.0;
    }

    pub fn frame(&mut self, ctx: &egui::Context, cpu_seconds: Option<f32>, ready: bool) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        let focused = ctx.input(|input| input.focused);
        if self.previous.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.capture_previous
            && let (Some(previous), Some(cpu)) = (self.previous, cpu_seconds)
        {
            self.samples.push(Sample {
                ui_ms: self.ui_ms,
                cpu_including_render_ms: f64::from(cpu) * 1000.0,
                frame_interval_ms: now.duration_since(previous).as_secs_f64() * 1000.0,
                focused: self.previous_focused && focused,
            });
        }
        self.ui_ms = 0.0;
        self.previous = Some(now);
        self.previous_focused = focused;
        if ready {
            let elapsed = now.duration_since(*self.started.get_or_insert(now));
            self.capture_previous = elapsed >= Duration::from_secs(1);
            if elapsed >= self.duration + Duration::from_secs(1) {
                self.finished = true;
                let foreground: Vec<_> = self
                    .samples
                    .iter()
                    .filter(|sample| sample.focused)
                    .collect();
                let report = serde_json::json!({
                    "description": "Continuous native repaint after loading, with 1s warmup. Frame intervals include event-loop/presentation wait; CPU excludes swap wait. Not physical mouse-to-photon latency.",
                    "synthetic_pointer_motion": self.motion,
                    "frames": self.samples.len(),
                    "ui_ms": distribution(self.samples.iter().map(|s| s.ui_ms)),
                    "cpu_including_render_ms": distribution(self.samples.iter().map(|s| s.cpu_including_render_ms)),
                    "frame_interval_ms": distribution(self.samples.iter().map(|s| s.frame_interval_ms)),
                    "pixels_per_point": ctx.pixels_per_point(),
                    "viewport_points": [ctx.viewport_rect().width(), ctx.viewport_rect().height()],
                    "foreground": {
                        "frames": foreground.len(),
                        "ui_ms": distribution(foreground.iter().map(|s| s.ui_ms)),
                        "cpu_including_render_ms": distribution(foreground.iter().map(|s| s.cpu_including_render_ms)),
                        "frame_interval_ms": distribution(foreground.iter().map(|s| s.frame_interval_ms)),
                    },
                    "samples": self.samples,
                });
                if let Err(error) = fs::write(
                    &self.path,
                    serde_json::to_vec_pretty(&report).expect("finite frame timings serialize"),
                ) {
                    log::error!("cannot save frame profile: {error}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        ctx.request_repaint();
    }
}

fn distribution(values: impl Iterator<Item = f64>) -> Distribution {
    let mut values: Vec<f64> = values.collect();
    values.sort_unstable_by(f64::total_cmp);
    let at = |percent: usize| {
        values
            .get(values.len().saturating_sub(1) * percent / 100)
            .copied()
            .unwrap_or(0.0)
    };
    Distribution {
        p50: at(50),
        p95: at(95),
        p99: at(99),
        max: at(100),
    }
}
