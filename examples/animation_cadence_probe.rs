//! 动画帧节奏人工探针，不属于项目测试。
//! cargo run --locked --offline --example animation_cadence_probe
//! 记录声明式 Animated 与指令式 widget 动画共窗播放时的逐帧间隔，
//! 运行 10 秒后打印分位数统计并退出；配合 UIX_SLOW_FRAME_MS 观察阶段耗时。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use uix_app::prelude::*;

/// 两条逐帧时间戳流：声明式（根重建）与指令式（widget 动画推进）。
fn stamps(stream: &str) -> &'static Mutex<Vec<Instant>> {
    static DECLARATIVE: OnceLock<Mutex<Vec<Instant>>> = OnceLock::new();
    static IMPERATIVE: OnceLock<Mutex<Vec<Instant>>> = OnceLock::new();
    match stream {
        "declarative" => DECLARATIVE.get_or_init(|| Mutex::new(Vec::new())),
        _ => IMPERATIVE.get_or_init(|| Mutex::new(Vec::new())),
    }
}

fn record(stream: &str) {
    stamps(stream).lock().unwrap().push(Instant::now());
}

fn report(stream: &str, label: &str) {
    let samples = stamps(stream).lock().unwrap().clone();
    if samples.len() < 3 {
        println!("[{label}] insufficient samples: {}", samples.len());
        return;
    }
    let mut gaps: Vec<f64> = samples
        .windows(2)
        .map(|pair| pair[1].duration_since(pair[0]).as_secs_f64() * 1000.0)
        .collect();
    gaps.sort_by(|a, b| a.total_cmp(b));
    let count = gaps.len();
    let pick = |quantile: f64| gaps[((count as f64 - 1.0) * quantile).round() as usize];
    let long_gaps: Vec<f64> = gaps.iter().copied().filter(|gap| *gap > 25.0).collect();
    let mean = gaps.iter().sum::<f64>() / count as f64;
    println!(
        "[{label}] samples={} p50={:.2}ms p90={:.2}ms p99={:.2}ms max={:.2}ms mean={:.2}ms fps={:.1} gaps>25ms={}",
        count + 1,
        pick(0.50),
        pick(0.90),
        pick(0.99),
        gaps[count - 1],
        mean,
        1000.0 / mean,
        long_gaps.len(),
    );
    if !long_gaps.is_empty() {
        let preview: Vec<String> =
            long_gaps.iter().map(|gap| format!("{gap:.1}")).take(12).collect();
        println!("[{label}] long gaps (ms): {}", preview.join(" "));
    }
}

widget! {
    pub struct PulseRing {
        pub time: f32,
    }
    @new -> Self {
        Self { time: 0.0 }
    }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(48.0, 48.0))
    }
    dirty_rect => (&self, frame: Rect) -> Rect {
        let center_x = frame.x + frame.w * 0.5;
        let center_y = frame.y + frame.h * 0.5;
        Rect::new(center_x - 24.0, center_y - 24.0, 48.0, 48.0)
    }
    update_animation => (&mut self, delta_seconds: f64) -> bool {
        self.time += delta_seconds as f32;
        record("imperative");
        true
    }
    render => (&self, frame: Rect, ctx: &mut PaintContext) {
        let phase = (self.time * 1.5).sin() * 0.5 + 0.5;
        let radius = 6.0 + phase * 16.0;
        let color = ctx.tokens().color_primary();
        ctx.stroke_circle(
            frame.x + frame.w * 0.5,
            frame.y + frame.h * 0.5,
            radius,
            color,
            3.0,
        );
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "uix=info".into()),
        )
        .init();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        report("declarative", "declarative Animated");
        report("imperative", "imperative widget");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::thread::sleep(Duration::from_millis(500));
        std::process::exit(0);
    });

    let anim = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::ease_in_out)
        .loop_forever();

    let code = App::new()
        .title("UIX animation cadence probe")
        .size(360, 240)
        .root({
            let anim = anim.clone();
            move || {
                record("declarative");
                let opacity = anim.value();
                column((
                    label("declarative fade")
                        .opacity(opacity)
                        .width(200.0)
                        .height(64.0)
                        .bg(Color::from_rgb(90, 140, 220)),
                    ViewNode::leaf(PulseRing::new()),
                ))
                .gap(16.0)
            }
        })
        .run();
    tracing::info!(code, "probe finished");
}
