// 只在 Windows 同时启用 Agent 与图像编码能力时编译系统主题真窗验收。
#![cfg(all(windows, feature = "agent-control", feature = "image-codecs"))]
// 允许断言型验收代码在协议、注册表或证据缺失处定向失败。
#![allow(
    clippy::expect_used,
    reason = "系统主题验收必须在首个不可恢复偏好、窗口或证据处给出明确失败"
)]

// 接入唯一拥有主演示子进程与 discovery 生命周期的 fixture。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只依赖公开 uix.agent.v1 的协议 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;
// 接入可恢复的当前用户系统主题偏好 Adapter。
#[path = "support/uix_lang_graphics_recovery/system_theme.rs"]
mod system_theme;
// 接入只负责真实 HWND 像素证据的 Windows Adapter。
#[path = "support/uix_lang_graphics_recovery/visual_capture.rs"]
mod visual_capture;

// 导入证据路径值。
use std::path::{Path, PathBuf};

// 导入 JSON 快照值。
use serde_json::Value;

// 引入主演示进程 fixture。
use process::DemoProcess;
// 引入公开 Agent 客户端。
use protocol::AgentClient;
// 引入可恢复的系统主题偏好事务。
use system_theme::SystemThemePreference;

// 在真实 D3D11 主演示中验收 Windows 应用主题实时跟随与恢复。
#[test]
#[ignore = "requires an interactive Windows desktop and temporarily toggles AppsUseLightTheme"]
fn real_uix_lang_demo_follows_and_restores_windows_system_theme() {
    // 独占当前测试进程的全局 Windows 应用主题偏好变更。
    let _theme_lock = system_theme::lock();
    // 在任何写入前捕获可恢复的精确原值。
    let mut preference = SystemThemePreference::capture().expect("read AppsUseLightTheme");
    // 保存原值供明暗方向与日志断言。
    let original = preference.original();

    // 启动显式系统主题跟随、Agent 与强制 D3D11 的主演示。
    let mut demo = DemoProcess::spawn_main_with_args(&["--follow-system-theme"]);
    // 等待同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接 fixture 唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 系统主题验收只允许一个主演示窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(window, 1);
    // 通过公开侧边栏进入展示主题策略的应用能力页。
    let runtime_revision = client.invoke(window, "sidebar-page-1");
    // 等待应用能力页完成真实呈现。
    client.wait_for_presented(window, runtime_revision);
    // 读取切换前的完整语义快照。
    let baseline_snapshot = client.snapshot(window);
    // UI 必须明确报告由 App System 跟随系统，而不是伪报静态明暗值。
    assert_node_name(
        &baseline_snapshot,
        "runtime-theme-state",
        "主题策略：跟随系统",
    );
    // 保存外部事件前的语义 revision。
    let baseline_revision = baseline_snapshot["revision"]
        // revision 必须是协议数值。
        .as_u64()
        // 缺失值属于语义快照错误。
        .expect("baseline semantic revision");

    // 创建本测试独占的稳定证据目录。
    let evidence_root = evidence_root();
    // 精确清理上次系统主题证据。
    reset_evidence_root(&evidence_root);
    // 保存原始系统主题证据路径。
    let baseline = evidence_root.join("system-original.png");
    // 捕获首次真实客户区像素。
    visual_capture::capture_png(&demo, &baseline);

    // 把精确偏好切换到与原值相反的明暗模式并读回验证。
    let toggled_value = preference.toggle().expect("toggle AppsUseLightTheme");
    // 向主演示真实 HWND 发送系统设置变化消息。
    system_theme::notify_demo(&demo);
    // 等待系统消息进入正常协调管线并完成真实 present。
    let toggled_revision = client.wait_for_changed_and_presented(window, baseline_revision);
    // 保存切换后的系统主题证据路径。
    let toggled = evidence_root.join("system-toggled.png");
    // 捕获切换后真实客户区像素。
    visual_capture::capture_png(&demo, &toggled);

    // 在测试继续前显式恢复并读回原始用户偏好。
    preference.restore().expect("restore AppsUseLightTheme");
    // 向主演示通知恢复后的系统主题事实。
    system_theme::notify_demo(&demo);
    // 等待恢复 revision 完成真实 present。
    let _restored_revision = client.wait_for_changed_and_presented(window, toggled_revision);
    // 保存恢复后的系统主题证据路径。
    let restored = evidence_root.join("system-restored.png");
    // 捕获恢复后真实客户区像素。
    visual_capture::capture_png(&demo, &restored);

    // 计算系统主题切换影响的完整客户区像素比例。
    let toggled_ratio = visual_capture::changed_pixel_ratio(&baseline, &toggled);
    // 计算恢复证据相对原始证据的残余变化比例。
    let restored_ratio = visual_capture::changed_pixel_ratio(&baseline, &restored);
    // 系统主题信号必须改变至少一成客户区像素。
    assert!(
        toggled_ratio >= 0.10,
        "system theme signal changed only {:.2}% of pixels",
        toggled_ratio * 100.0
    );
    // 恢复后只允许计时文本等小范围动态差异。
    assert!(
        restored_ratio <= 0.05 && restored_ratio < toggled_ratio * 0.5,
        "restored system theme differs by {:.2}% (toggle delta {:.2}%)",
        restored_ratio * 100.0,
        toggled_ratio * 100.0
    );
    // 根据原始偏好方向验证真实平均亮度变化。
    if original == 1 {
        // 原始亮色切换到暗色必须显著变暗。
        visual_capture::assert_theme_luminance_delta(&baseline, &toggled, 80.0);
    } else {
        // 原始暗色切换到亮色必须显著变亮。
        visual_capture::assert_theme_luminance_delta(&toggled, &baseline, 80.0);
    }
    // 生成原始、切换、恢复三格审阅板。
    visual_capture::write_contact_sheet(
        // 保持可审计的时间顺序。
        &[baseline.clone(), toggled.clone(), restored.clone()],
        // 保存统一审阅入口。
        &evidence_root.join("system-theme-contact.png"),
        // 三列直接比较切换和恢复。
        3,
    );

    // 关闭协议连接让子进程退出时不等待客户端。
    drop(client);
    // 回收唯一主演示并收集原生图形日志。
    let output = demo.stop_and_collect();
    // 启动必须选择生产 D3D11 retained RHI recipe。
    assert!(
        output.contains(
            "Graphics bootstrap: selected recipe backend=d3d11; raster=gpu_native; present=swapchain"
        ),
        "system theme demo did not select D3D11 recipe: {output}"
    );
    // 验收不得静默退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "system theme demo unexpectedly used software fallback: {output}"
    );
    // 显式释放已经完成恢复的偏好事务。
    drop(preference);
    // 输出不含 token 的偏好变化与证据摘要。
    println!(
        "uix-lang system theme: AppsUseLightTheme {original} -> {toggled_value} -> {original}; delta={:.2}%; restored={:.2}%; evidence={}",
        toggled_ratio * 100.0,
        restored_ratio * 100.0,
        evidence_root.display()
    );
}

// 返回仓库 target 下不进入提交的系统主题证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入人工与 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离系统主题视觉证据。
        .join("uix-lang-system-theme")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &Path) {
    // 存在时只删除本测试的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous system theme evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create system theme evidence root");
}

// 验证稳定目标具有精确可访问名称。
fn assert_node_name(snapshot: &Value, automation_id: &str, expected: &str) {
    // 目标必须存在于当前窗口语义树。
    let node = snapshot["nodes"]
        // 节点数组必须由协议提供。
        .as_array()
        // 缺失节点数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历全部语义节点。
        .iter()
        // 精确匹配稳定自动化标识。
        .find(|node| node["automation_id"] == automation_id)
        // 缺失时报告稳定标识。
        .unwrap_or_else(|| panic!("missing semantic target {automation_id}"));
    // 名称必须精确匹配唯一策略事实。
    assert_eq!(node["name"], expected, "unexpected node name: {node}");
}
