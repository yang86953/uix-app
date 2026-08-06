// ============================================================================
// UIX CLI 演示入口 — core / draw / ui / app / data 功能域（无 GUI 输出）
// ============================================================================
//   cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo
// ============================================================================

mod cli;

fn main() {
    println!("UIX CLI 演示启动中...");
    if let Err(e) = cli::run() {
        eprintln!("CLI 演示出错: {}", e.short_what());
        std::process::exit(1);
    }
}
