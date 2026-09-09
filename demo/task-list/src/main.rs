use std::path::PathBuf;
use uix_task_list::app::TaskApp;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "uix=info".into()),
        )
        .with_ansi(false)
        .init();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        eprintln!(
            "用法：uix-task-list /绝对隔离路径/tasks.json（仅显式保存；不要同时启动两个实例写同一文件）"
        );
        std::process::exit(2);
    }
    let app = match TaskApp::new(PathBuf::from(&args[0])) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let code = app.application().run();
    if let Err(error) = app.shutdown() {
        eprintln!("关闭回收失败：{error}");
        std::process::exit(1);
    }
    std::process::exit(code);
}
