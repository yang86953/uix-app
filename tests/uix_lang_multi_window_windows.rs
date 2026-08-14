// 只在 Windows 同时启用 Agent 与图像编码能力时编译真窗多窗口验收。
#![cfg(all(windows, feature = "agent-control", feature = "image-codecs"))]
// 允许断言型验收代码在协议或窗口缺失处给出精确失败。
#![allow(
    clippy::expect_used,
    reason = "多窗口验收必须在首个缺失窗口、节点或证据文件处定向失败"
)]

// 接入唯一拥有主演示子进程与 discovery 生命周期的 fixture。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只依赖公开 uix.agent.v1 的协议 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;
// 接入只负责真实 HWND 像素证据的 Windows Adapter。
#[path = "support/uix_lang_graphics_recovery/visual_capture.rs"]
mod visual_capture;

// 导入证据路径值。
use std::path::{Path, PathBuf};
// 导入有界窗口状态轮询能力。
use std::thread;
// 导入轮询预算与截止时点。
use std::time::{Duration, Instant};

// 导入 JSON 动作与快照值。
use serde_json::{Value, json};

// 引入主演示进程 fixture。
use process::DemoProcess;
// 引入公开 Agent 客户端与逐窗身份。
use protocol::{AgentClient, AgentWindow};

// 保存次级主题窗口的稳定公开标题。
const THEME_WINDOW_TITLE: &str = "UIX Theme Window";
// 保存主演示与次窗共享的浅色状态文本。
const LIGHT_THEME_STATUS: &str = "当前主题：亮色";
// 保存主演示与次窗共享的暗色状态文本。
const DARK_THEME_STATUS: &str = "当前主题：暗色";

// 在真实 D3D11 主演示中验收按需开窗、跨窗主题与逐窗关闭。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_uix_lang_demo_opens_and_controls_independent_theme_window() {
    // 启动显式 Agent 与强制 D3D11 的普通主演示。
    let mut demo = DemoProcess::spawn_main();
    // 等待同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接 fixture 唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 启动时只能存在主演示窗口。
    let main_window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(main_window, 1);
    // 通过公开侧边栏进入应用能力页。
    let runtime_revision = client.invoke(main_window, "sidebar-page-1");
    // 等待应用能力页完成真实呈现。
    client.wait_for_presented(main_window, runtime_revision);
    // 揭示多窗口入口并验证主窗初始状态。
    let main_light_snapshot = reveal_target(
        // 使用公开 Agent 客户端。
        &mut client,
        // 保持主演示窗口身份。
        main_window,
        // 揭示按需开窗按钮。
        "runtime-open-theme-window",
    );
    // 主窗初始主题状态必须来自唯一共享 State。
    assert_node_name(
        &main_light_snapshot,
        "runtime-theme-state",
        LIGHT_THEME_STATUS,
    );
    // 开窗入口必须公开可调用动作。
    assert_node_action(&main_light_snapshot, "runtime-open-theme-window", "invoke");

    // 创建本测试独占的稳定证据目录。
    let evidence_root = evidence_root();
    // 精确清理上次多窗口证据。
    reset_evidence_root(&evidence_root);
    // 保存主窗浅色证据路径。
    let main_light = evidence_root.join("main-light.png");
    // 捕获当前多窗口卡片可见的主演示客户区。
    visual_capture::capture_png(&demo, &main_light);

    // 通过公开回调请求 Application System 创建次窗。
    let open_revision = client.invoke(main_window, "runtime-open-theme-window");
    // 等待主窗创建状态完成真实呈现。
    client.wait_for_presented(main_window, open_revision);
    // 等待协议登记两个独立窗口并取得次窗身份。
    let theme_window = wait_for_window_count(&mut client, 2)
        // 查找稳定公开标题对应的次窗。
        .into_iter()
        // 只保留主题联动窗口。
        .find_map(|(window, title)| (title == THEME_WINDOW_TITLE).then_some(window))
        // 缺失次窗表示 open_window 没有形成公开生命周期。
        .expect("theme window listed by public Agent protocol");
    // 次窗首个 View revision 必须完成真实 present。
    client.wait_for_presented(theme_window, 1);
    // 读取次窗完整语义快照。
    let child_light_snapshot = client.snapshot(theme_window);
    // 次窗标题内容必须具有稳定语义目标。
    assert_node_name(
        &child_light_snapshot,
        "theme-window-heading",
        "跨窗口主题联动",
    );
    // 次窗必须观察与主窗相同的浅色主题状态。
    assert_node_name(
        &child_light_snapshot,
        "theme-window-theme-state",
        LIGHT_THEME_STATUS,
    );
    // 自定义标题栏关闭控件必须公开 invoke 动作。
    assert_node_action(&child_light_snapshot, "theme-window-close", "invoke");
    // 主窗必须显示窗口创建成功事实。
    wait_for_node_name(
        &mut client,
        main_window,
        "runtime-multi-window-status",
        "主题联动窗口：已打开",
    );
    // 保存次窗浅色证据路径。
    let child_light = evidence_root.join("child-light.png");
    // 按稳定标题捕获次窗而不是面积更大的主窗。
    visual_capture::capture_png_by_title(&demo, &child_light, THEME_WINDOW_TITLE);

    // 从次窗经公开 AppHandle 边界请求应用暗色主题。
    let dark_revision = client.invoke(theme_window, "theme-window-dark");
    // 等待发起窗口完成暗色真实呈现。
    client.wait_for_presented(theme_window, dark_revision);
    // 次窗共享状态必须变为暗色。
    wait_for_node_name(
        &mut client,
        theme_window,
        "theme-window-theme-state",
        DARK_THEME_STATUS,
    );
    // 主窗必须观察同一主题 State，而不是维护第二份状态。
    wait_for_node_name(
        &mut client,
        main_window,
        "runtime-theme-state",
        DARK_THEME_STATUS,
    );
    // 保存次窗暗色证据路径。
    let child_dark = evidence_root.join("child-dark.png");
    // 捕获暗色次窗客户区。
    visual_capture::capture_png_by_title(&demo, &child_dark, THEME_WINDOW_TITLE);
    // 保存主窗暗色证据路径。
    let main_dark = evidence_root.join("main-dark.png");
    // 捕获暗色主演示客户区。
    visual_capture::capture_png(&demo, &main_dark);
    // 主窗主题必须产生显著像素亮度变化。
    visual_capture::assert_theme_luminance_delta(&main_light, &main_dark, 80.0);
    // 次窗主题也必须产生显著像素亮度变化。
    visual_capture::assert_theme_luminance_delta(&child_light, &child_dark, 80.0);
    // 生成四格跨窗主题审阅板。
    visual_capture::write_contact_sheet(
        // 保持主浅、子浅、主暗、子暗的对照顺序。
        &[
            main_light.clone(),
            child_light.clone(),
            main_dark.clone(),
            child_dark.clone(),
        ],
        // 保存统一审阅入口。
        &evidence_root.join("multi-window-contact.png"),
        // 两列形成明暗两行对照。
        2,
    );

    // 通过次窗自定义标题栏关闭当前 WindowSession。
    let _ = client.invoke(theme_window, "theme-window-close");
    // 只允许次窗消失，主演示必须继续存在。
    let remaining = wait_for_window_count(&mut client, 1);
    // 剩余窗口必须是最初的主演示身份。
    assert_eq!(remaining[0].0.id, main_window.id);
    // 主窗在次窗关闭后仍可查询完整语义树。
    assert_node_name(
        &client.snapshot(main_window),
        "runtime-theme-state",
        DARK_THEME_STATUS,
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
        "multi-window demo did not select D3D11 recipe: {output}"
    );
    // 验收不得静默退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "multi-window demo unexpectedly used software fallback: {output}"
    );
    // 输出不含 token 的证据位置摘要。
    println!(
        "uix-lang multi-window evidence: {}",
        evidence_root.display()
    );
}

// 返回仓库 target 下不进入提交的多窗口证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入人工与 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离多窗口视觉证据。
        .join("uix-lang-multi-window")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &Path) {
    // 存在时只删除本测试的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous multi-window evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create multi-window evidence root");
}

// 逐步滚动直到一个稳定目标具有可视觉辨认边界。
fn reveal_target(
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收当前主演示窗口身份。
    window: AgentWindow,
    // 接收需要揭示的稳定自动化标识。
    automation_id: &str,
) -> Value {
    // 最多执行十二次有界滚动。
    for _ in 0..12 {
        // 读取当前语义快照。
        let snapshot = client.snapshot(window);
        // 目标具有有效可见边界时完成揭示。
        if node_by_automation_id(&snapshot, automation_id).is_some_and(has_usable_bounds) {
            // 返回可供后续语义断言的同一快照。
            return snapshot;
        }
        // 选择当前页面宽度最大的公开滚动动作节点。
        let target = scroll_target(&snapshot);
        // 使用归一化单步增量向下滚动。
        let revision = client.perform_optional(
            // 保持当前窗口身份。
            window,
            // 使用公开 node_id 目标。
            target,
            // 使用公开 scroll 动作。
            json!({ "kind": "scroll", "delta_x": 0.0, "delta_y": 1.0 }),
        );
        // 只有滚动状态变化时才等待真实 present。
        if let Some(revision) = revision {
            // 等待当前滚动批次呈现。
            client.wait_for_presented(window, revision);
        }
    }
    // 超过预算后报告目标当前语义记录。
    let snapshot = client.snapshot(window);
    // 缺失可见目标必须阻止截图产生伪证据。
    panic!(
        "target {automation_id} not visible after bounded scroll: {:?}",
        node_by_automation_id(&snapshot, automation_id)
    );
}

// 从语义树选择主内容区的真实 Scroll 动作节点。
fn scroll_target(snapshot: &Value) -> Value {
    // 在可滚动节点中选择可见宽度最大的主内容容器。
    let node = snapshot["nodes"]
        // 节点数组必须由协议提供。
        .as_array()
        // 缺失节点数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历当前窗口全部节点。
        .iter()
        // 只保留公开动作集合包含 scroll 的节点。
        .filter(|node| {
            node["actions"]
                // 动作必须是数组。
                .as_array()
                // 精确匹配公开 scroll 动作。
                .is_some_and(|actions| actions.contains(&json!("scroll")))
        })
        // 按可见宽度选择主内容区。
        .max_by(|left, right| {
            // 读取左节点可见宽度。
            let left_width = left["visible_bounds"]["w"].as_f64().unwrap_or_default();
            // 读取右节点可见宽度。
            let right_width = right["visible_bounds"]["w"].as_f64().unwrap_or_default();
            // 使用总序比较浮点宽度。
            left_width.total_cmp(&right_width)
        })
        // 缺失滚动节点表示页面不能揭示目标。
        .expect("main page semantic scroll target");
    // 返回公开 node_id 目标形状。
    json!({ "node_id": node["node_id"].clone() })
}

// 判断语义目标是否具有可视觉辨认的裁剪边界。
fn has_usable_bounds(node: &Value) -> bool {
    // 读取协议裁剪后的可见宽度。
    let width = node["visible_bounds"]["w"].as_f64().unwrap_or_default();
    // 读取协议裁剪后的可见高度。
    let height = node["visible_bounds"]["h"].as_f64().unwrap_or_default();
    // 读取目标顶部位置。
    let top = node["visible_bounds"]["y"].as_f64().unwrap_or(f64::MAX);
    // 拒绝零尺寸、仅露出数像素或被底部状态栏遮挡的伪可见状态。
    width >= 16.0 && height >= 24.0 && top + height <= 730.0
}

// 有界等待 Agent 公布期望数量的窗口。
fn wait_for_window_count(
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收期望窗口数量。
    expected: usize,
) -> Vec<(AgentWindow, String)> {
    // 设置窗口创建或销毁最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续查询直到数量匹配或超时。
    loop {
        // 读取当前公开窗口表。
        let windows = client.list_windows();
        // 精确匹配时返回完整条目。
        if windows.len() == expected {
            // 返回供调用方进一步验证标题与身份。
            return windows;
        }
        // 超时前保持短间隔轮询。
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} windows; observed={}",
            windows.len()
        );
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 有界等待指定语义节点取得精确可访问名称。
fn wait_for_node_name(
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收目标窗口身份。
    window: AgentWindow,
    // 接收稳定自动化标识。
    automation_id: &str,
    // 接收期望可访问名称。
    expected: &str,
) {
    // 设置跨窗状态传播最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续查询直到共享状态一致或超时。
    loop {
        // 读取当前窗口语义快照。
        let snapshot = client.snapshot(window);
        // 精确名称匹配时完成等待。
        if node_by_automation_id(&snapshot, automation_id)
            .is_some_and(|node| node["name"] == expected)
        {
            // 状态已经传播到目标窗口。
            return;
        }
        // 超时前继续短轮询。
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {automation_id}={expected:?}"
        );
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 验证稳定目标具有精确可访问名称。
fn assert_node_name(snapshot: &Value, automation_id: &str, expected: &str) {
    // 目标必须存在于当前窗口语义树。
    let node = node_by_automation_id(snapshot, automation_id)
        // 缺失时报告稳定标识。
        .unwrap_or_else(|| panic!("missing semantic target {automation_id}"));
    // 名称必须精确匹配唯一状态事实。
    assert_eq!(node["name"], expected, "unexpected node name: {node}");
}

// 验证稳定目标公开指定语义动作。
fn assert_node_action(snapshot: &Value, automation_id: &str, expected: &str) {
    // 目标必须存在于当前窗口语义树。
    let node = node_by_automation_id(snapshot, automation_id)
        // 缺失时报告稳定标识。
        .unwrap_or_else(|| panic!("missing semantic target {automation_id}"));
    // 动作数组必须包含期望公开动作。
    assert!(
        node["actions"]
            // 动作字段必须是数组。
            .as_array()
            // 精确匹配期望动作字符串。
            .is_some_and(|actions| actions.contains(&json!(expected))),
        "target {automation_id} missing action {expected}: {node}"
    );
}

// 按稳定 automation ID 查找语义节点。
fn node_by_automation_id<'a>(snapshot: &'a Value, automation_id: &str) -> Option<&'a Value> {
    // 读取节点数组并在缺失时返回 None。
    snapshot["nodes"]
        // 转为数组借用。
        .as_array()?
        // 遍历全部语义节点。
        .iter()
        // 精确匹配稳定标识。
        .find(|node| node["automation_id"] == automation_id)
}
