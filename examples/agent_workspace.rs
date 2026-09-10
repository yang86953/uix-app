//! 无桌面宿主：通过 Agent 客户端控制后台示例，关闭视口或两分钟后自动清理。
//! 用法见《Agent 独立后台操作面》，不创建隐藏或可见原生窗口。

#[cfg(feature = "agent-control")]
use uix_app::prelude::StyleExt;

#[cfg(feature = "agent-control")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant};
    use uix_app::app::agent_workspace::AgentWorkspace;

    let workspace = AgentWorkspace::new(720, 420, || uix_app::uix!("demo/uix-lang-demo/src/agent.uix"))
        .title("UIX isolated workspace example")
        .require_confirm("demo-popconfirm-trigger")
        .spawn()?;
    let mut wait_lane = workspace.client()?;
    let view = wait_lane.list_windows()?[0];
    println!(
        "background workspace ready: pid={}; use agent_client.py apps and bind this instance",
        std::process::id()
    );
    let started = Instant::now();
    // 用阻塞 wait 观察关闭，不轮询快照、不访问桌面；示例最多存活两分钟。
    while started.elapsed() < Duration::from_secs(120) {
        let result = wait_lane.request(serde_json::json!({
            "type": "wait", "window_id": view.window_id, "generation": view.generation,
            "after_revision": u64::MAX, "timeout_ms": 30000
        }));
        match result {
            Ok(reply) if reply["outcome"] == "timeout" => {}
            Ok(reply) if reply["error"]["code"] == "timeout" => {}
            // 关闭会完成 wait 或终止本机连接，最终结果仍由工作面关闭回传。
            _ => break,
        }
    }
    workspace.close()?;
    Ok(())
}

#[cfg(not(feature = "agent-control"))]
fn main() {
    eprintln!("this example requires --features agent-control");
}
