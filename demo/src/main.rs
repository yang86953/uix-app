// ============================================================================
// UIX 框架演示入口
// ============================================================================
// 运行方式：
//   cargo run --bin uix-demo          → GUI 演示
//   cargo run --bin uix-demo -- --cli → CLI 演示
// ============================================================================

mod demos;

use uix::diag::log::{info_fn, Level, Logger};

/// 将标准 log crate 桥接到自定义 Logger
struct LogCrateBridge;

impl log::Log for LogCrateBridge {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let level = match metadata.level() {
            log::Level::Trace => Level::Trace,
            log::Level::Debug => Level::Debug,
            log::Level::Info => Level::Info,
            log::Level::Warn => Level::Warn,
            log::Level::Error => Level::Error,
        };
        level as u8 >= Logger::instance().get_level() as u8
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Trace => Level::Trace,
            log::Level::Debug => Level::Debug,
            log::Level::Info => Level::Info,
            log::Level::Warn => Level::Warn,
            log::Level::Error => Level::Error,
        };
        let file: &'static str = record
            .file()
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &'static str)
            .unwrap_or("<unknown>");
        Logger::instance().log(
            level,
            format!("{}", record.args()),
            file,
            record.line().unwrap_or(0),
            Vec::new(),
        );
    }

    fn flush(&self) {
        Logger::instance().flush();
    }
}

fn main() {
    let log_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "trace" => Some(Level::Trace),
            "debug" => Some(Level::Debug),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        })
        .unwrap_or(Level::Info);
    Logger::instance().set_level(log_level);

    // 桥接标准 log crate，使 log::debug!() 等宏也能输出
    let _ = log::set_logger(&LogCrateBridge);
    log::set_max_level(log::LevelFilter::Trace);

    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|a| a == "--cli");

    if is_cli {
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = demos::cli::run_all_cli() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        info_fn("UIX GUI 演示启动中...");
        demos::dashboard::run_gui_demo();
    }
}
