// 只在 Windows 同时启用 Agent 与图像编码能力时编译真窗视觉验收。
#![cfg(all(windows, feature = "agent-control", feature = "image-codecs"))]
// 允许断言型验收代码使用场景化 expect 诊断。
#![allow(
    clippy::expect_used,
    reason = "视觉验收必须在首个缺失窗口、节点或证据文件处给出明确失败"
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

// 引入证据路径值。
use std::path::PathBuf;

// 引入 JSON 动作与快照值。
use serde_json::{Value, json};

// 引入主演示进程 fixture。
use process::DemoProcess;
// 引入公开 Agent 客户端与逐窗身份。
use protocol::{AgentClient, AgentWindow};

// 保存十二个公开页面的索引、文件名与可访问标题。
const PAGES: [(usize, &str, &str); 12] = [
    // 首页。
    (0, "home", "首页"),
    // 应用能力页。
    (1, "runtime", "应用能力"),
    // 通用组件页。
    (2, "general", "通用组件"),
    // 布局组件页。
    (3, "layout", "布局组件"),
    // 导航组件页。
    (4, "navigation", "导航组件"),
    // 输入组件页。
    (5, "input", "输入组件"),
    // 数据展示页。
    (6, "data", "数据展示"),
    // 反馈组件页。
    (7, "feedback", "反馈组件"),
    // 图表页。
    (8, "charts", "图表"),
    // 其他组件页。
    (9, "other", "其他组件"),
    // 框架能力页。
    (10, "framework", "框架能力"),
    // 覆盖清单页。
    (11, "gallery", "覆盖清单"),
];

// 保存新增七类组件所在页面、语义标识与负责布局揭示的目标。
const TARGET_GROUPS: [(usize, &str, &[&str], &[&str], f64); 3] = [
    // 数据页覆盖选择、折叠与徽标。
    (
        6,
        "data-components",
        &["demo-selectable-list", "demo-collapse", "demo-badge"],
        &["demo-selectable-list", "demo-collapse", "demo-badge"],
        // 徽标高度较小，完整可辨认边界为十六像素。
        16.0,
    ),
    // 反馈页覆盖消息、通知与确认气泡。
    (
        7,
        "feedback-components",
        &["demo-message", "demo-notification", "demo-popconfirm"],
        // Message 与 Notification 是零布局租约，使用 Popconfirm 揭示所属面板。
        &["demo-popconfirm"],
        // 确认按钮需要至少二十四像素高度。
        24.0,
    ),
    // 其他页覆盖受控上传。
    (
        9,
        "upload-component",
        &["demo-upload"],
        &["demo-upload"],
        // 上传拖放区至少揭示其完整一百像素绘制主体。
        96.0,
    ),
];

// 在真实 D3D11 主演示中生成并验证十二页与新增组件视觉证据。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_uix_lang_demo_captures_public_pages_and_registered_components() {
    // 启动默认页面、显式 Agent 与强制 D3D11 的主演示。
    let mut demo = DemoProcess::spawn_main();
    // 等待同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接 fixture 唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 普通主演示必须只创建一个根窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(window, 1);
    // 创建当前仓库 target 下的稳定证据目录。
    let evidence_root = evidence_root();
    // 清理本测试上次生成的精确证据目录。
    reset_evidence_root(&evidence_root);

    // 保存十二页浅色顶部证据路径。
    let mut page_images = Vec::with_capacity(PAGES.len());
    // 按公开侧边栏顺序逐页导航与捕获。
    for (index, slug, title) in PAGES {
        // 通过公开 invoke 进入目标页面。
        navigate(&mut client, window, index);
        // 读取当前页面语义树。
        let snapshot = client.snapshot(window);
        // 页面标题必须可见并具有非空语义边界。
        assert_visible_named_node(&snapshot, title);
        // 组合当前页面 PNG 路径。
        let path = evidence_root.join(format!("light-{index:02}-{slug}.png"));
        // 从真实 DWM 客户区捕获无损像素。
        visual_capture::capture_png(&demo, &path);
        // 保存路径供审阅板拼接。
        page_images.push(path);
    }
    // 生成十二页浅色总览审阅板。
    visual_capture::write_contact_sheet(
        &page_images,
        &evidence_root.join("light-pages-contact.png"),
        4,
    );

    // 保存新增组件的浅色与深色证据路径。
    let mut component_images = Vec::with_capacity(TARGET_GROUPS.len() * 2);
    // 逐页揭示并验证七类新组件。
    for (page, slug, targets, visible_targets, minimum_visible_height) in TARGET_GROUPS {
        // 导航到组件所属页面。
        navigate(&mut client, window, page);
        // 通过公开 Scroll 动作逐步揭示全部目标。
        let snapshot = reveal_targets(&mut client, window, visible_targets, minimum_visible_height);
        // 每个目标必须存在且提供语义角色与状态对象。
        assert_target_semantics(&snapshot, targets);
        // 组合浅色组件证据路径。
        let path = evidence_root.join(format!("light-{slug}.png"));
        // 捕获组件全部可见时的窗口客户区。
        visual_capture::capture_png(&demo, &path);
        // 保存浅色证据路径。
        component_images.push(path);
    }

    // 导航到持有应用主题请求入口的运行时页面。
    navigate(&mut client, window, 1);
    // 通过公开 setTheme 按钮请求应用 Provider 切换到深色主题。
    let themed_revision = client.invoke(window, "theme-dark");
    // 等待主题 revision 完成真实 present。
    client.wait_for_presented(window, themed_revision);
    // 在深色主题下重复三组新增组件证据。
    for (page, slug, targets, visible_targets, minimum_visible_height) in TARGET_GROUPS {
        // 导航到组件所属页面。
        navigate(&mut client, window, page);
        // 重新揭示当前页面目标。
        let snapshot = reveal_targets(&mut client, window, visible_targets, minimum_visible_height);
        // 深色主题不能改变语义目标契约。
        assert_target_semantics(&snapshot, targets);
        // 组合深色组件证据路径。
        let path = evidence_root.join(format!("dark-{slug}.png"));
        // 捕获深色真实客户区。
        visual_capture::capture_png(&demo, &path);
        // 保存深色证据路径。
        component_images.push(path);
    }
    // 每组深色证据都必须相对对应浅色证据显著降低平均亮度。
    for index in 0..TARGET_GROUPS.len() {
        // 使用八十级亮度差排除仅图标变化或无效主题请求。
        visual_capture::assert_theme_luminance_delta(
            &component_images[index],
            &component_images[index + TARGET_GROUPS.len()],
            80.0,
        );
    }
    // 生成六格新增组件主题对照审阅板。
    visual_capture::write_contact_sheet(
        &component_images,
        &evidence_root.join("registered-components-contact.png"),
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
        "visual demo did not select D3D11 recipe: {output}"
    );
    // 视觉验收不得静默退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "visual demo unexpectedly used software fallback: {output}"
    );
    // 输出不含 token 的证据位置摘要。
    println!("uix-lang visual evidence: {}", evidence_root.display());
}

// 返回仓库 target 下不进入提交的视觉证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入人工与 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离主演示视觉证据。
        .join("uix-lang-visual")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &PathBuf) {
    // 存在时只删除本测试的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous visual evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create visual evidence root");
}

// 通过稳定侧边栏按钮导航并等待真实 present。
fn navigate(client: &mut AgentClient, window: AgentWindow, page: usize) {
    // 构造主演示公开导航标识。
    let automation_id = format!("sidebar-page-{page}");
    // 执行公开 invoke Command。
    let revision = client.invoke(window, &automation_id);
    // 等待页面协调与绘制完成。
    client.wait_for_presented(window, revision);
}

// 逐步滚动直到一组组件目标全部可见。
fn reveal_targets(
    client: &mut AgentClient,
    window: AgentWindow,
    targets: &[&str],
    minimum_visible_height: f64,
) -> Value {
    // 读取当前页面初始语义树并定位真实可滚动节点。
    let initial = client.snapshot(window);
    // 使用公开动作集合选择滚动节点而不依赖包装器身份。
    let initial_scroll_target = scroll_target(&initial);
    // 先把共享页面滚动容器恢复到顶部，避免上一页面位置泄漏。
    let reset_revision = client.perform_optional(
        // 保持当前窗口身份。
        window,
        // 使用公开 node_id 定位真实滚动动作节点。
        initial_scroll_target,
        // 使用足够大的负增量回到顶部。
        json!({ "kind": "scroll", "delta_x": 0.0, "delta_y": -10_000.0 }),
    );
    // 等待滚动复位完成 present。
    if let Some(reset_revision) = reset_revision {
        // 只有滚动状态实际变化时才等待对应 present。
        client.wait_for_presented(window, reset_revision);
    }
    // 最多执行十二次有界滚动。
    for _ in 0..12 {
        // 读取本轮语义快照。
        let snapshot = client.snapshot(window);
        // 全部目标具有可见边界时返回证据。
        if targets.iter().all(|target| {
            // 按稳定 ID 查找目标。
            node_by_automation_id(&snapshot, target)
                // 目标存在时检查可供视觉验收的有效裁剪边界。
                .is_some_and(|node| has_usable_visible_bounds(node, minimum_visible_height))
        }) {
            // 返回已经满足可见性的快照。
            return snapshot;
        }
        // 对主演示唯一页面滚动容器执行向下滚动。
        let revision = client.perform_optional(
            // 保持当前窗口身份。
            window,
            // 每个 revision 重新取得当前滚动节点身份。
            scroll_target(&snapshot),
            // 使用与真实滚轮一致的归一化单步增量，避免跨过中段目标。
            json!({ "kind": "scroll", "delta_x": 0.0, "delta_y": 1.0 }),
        );
        // 等待滚动后的新 revision 完成 present。
        if let Some(revision) = revision {
            // 只有滚动状态实际变化时才等待对应 present。
            client.wait_for_presented(window, revision);
        }
    }
    // 超过滚动预算后保留最终快照用于详细断言。
    let snapshot = client.snapshot(window);
    // 报告缺失可见目标的完整列表。
    let missing = targets
        // 遍历预期目标。
        .iter()
        // 保留不存在或没有可见边界的标识。
        .filter(|target| {
            !node_by_automation_id(&snapshot, target)
                .is_some_and(|node| has_usable_visible_bounds(node, minimum_visible_height))
        })
        // 复制标识借用供诊断。
        .copied()
        // 收集稳定顺序。
        .collect::<Vec<_>>();
    // 收集当前页面全部 demo 自动化节点供失败诊断。
    let observed = snapshot["nodes"]
        // 节点数组已经由快照协议保证。
        .as_array()
        // 遍历全部节点。
        .into_iter()
        .flatten()
        // 只保留主演示稳定标识。
        .filter_map(|node| {
            // 读取自动化标识字符串。
            let automation_id = node["automation_id"].as_str()?;
            // 过滤非 demo 目标。
            automation_id.starts_with("demo-").then(|| {
                // 投影标识、名称、边界与动作供诊断。
                json!({
                    "automation_id": automation_id,
                    "name": node["name"],
                    "visible_bounds": node["visible_bounds"],
                    "actions": node["actions"],
                })
            })
        })
        // 收集确定顺序。
        .collect::<Vec<_>>();
    // 有任何缺失时立即失败。
    assert!(
        missing.is_empty(),
        "visual targets not visible: {missing:?}; observed={observed:?}"
    );
    // 返回最终快照。
    snapshot
}

// 判断语义目标是否具有足以在截图中辨认的可见边界。
fn has_usable_visible_bounds(node: &Value, minimum_visible_height: f64) -> bool {
    // 读取协议裁剪后的可见宽度。
    let width = node["visible_bounds"]["w"].as_f64().unwrap_or_default();
    // 读取协议裁剪后的可见高度。
    let height = node["visible_bounds"]["h"].as_f64().unwrap_or_default();
    // 读取目标在固定验收窗口中的顶部位置。
    let top = node["visible_bounds"]["y"].as_f64().unwrap_or(f64::MAX);
    // 拒绝零尺寸、仅露出数像素或被底部状态栏遮挡的伪可见状态。
    width >= 16.0 && height >= minimum_visible_height && top + height <= 730.0
}

// 从语义树选择主内容区的真实 Scroll 动作节点。
fn scroll_target(snapshot: &Value) -> Value {
    // 读取扁平语义节点数组。
    let nodes = snapshot["nodes"].as_array().expect("snapshot nodes");
    // 在所有可滚动节点中选择可见宽度最大的主内容容器。
    let node = nodes
        // 遍历全部当前页面节点。
        .iter()
        // 只保留公开动作集合包含 scroll 的节点。
        .filter(|node| {
            node["actions"]
                // 动作必须是数组。
                .as_array()
                // 精确匹配公开 scroll 动作。
                .is_some_and(|actions| actions.contains(&json!("scroll")))
        })
        // 按可见宽度选择主内容区而不是内部小组件。
        .max_by(|left, right| {
            // 读取左节点可见宽度。
            let left_width = left["visible_bounds"]["w"].as_f64().unwrap_or_default();
            // 读取右节点可见宽度。
            let right_width = right["visible_bounds"]["w"].as_f64().unwrap_or_default();
            // 使用总序比较浮点宽度。
            left_width.total_cmp(&right_width)
        })
        // 缺失滚动节点表示页面无法揭示视口外组件。
        .expect("main page semantic scroll target");
    // node_id 必须是协议公开且非空的稳定身份值。
    let node_id = node["node_id"].clone();
    // 缺失身份不能用于后续动作。
    assert!(!node_id.is_null(), "scroll target missing node_id: {node}");
    // 返回公开 node_id 目标形状。
    json!({ "node_id": node_id })
}

// 验证当前页面包含一个可见的精确标题语义节点。
fn assert_visible_named_node(snapshot: &Value, expected: &str) {
    // 读取扁平语义节点数组。
    let nodes = snapshot["nodes"].as_array().expect("snapshot nodes");
    // 至少一个同名节点必须具有可见边界。
    assert!(
        nodes
            // 遍历当前窗口全部节点。
            .iter()
            // 匹配精确可访问名称与可见边界。
            .any(|node| node["name"] == expected && !node["visible_bounds"].is_null()),
        "missing visible page title {expected}"
    );
}

// 验证新增组件的可访问语义投影完整。
fn assert_target_semantics(snapshot: &Value, targets: &[&str]) {
    // 逐一验证全部稳定目标。
    for target in targets {
        // 每个目标必须在语义树中唯一可查。
        let node = node_by_automation_id(snapshot, target)
            // 缺失时报告具体自动化标识。
            .unwrap_or_else(|| panic!("missing semantic target {target}"));
        // 角色必须是非空协议字符串。
        assert!(
            node["role"].as_str().is_some_and(|role| !role.is_empty()),
            "target {target} missing role: {node}"
        );
        // 状态必须使用公开对象形状。
        assert!(node["state"].is_object(), "target {target} missing state");
        // 动作集合必须使用公开数组形状。
        assert!(
            node["actions"].is_array(),
            "target {target} missing actions"
        );
    }
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
