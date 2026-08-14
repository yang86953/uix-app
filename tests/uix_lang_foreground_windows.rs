// 只在 Windows 同时启用 Agent 与图像编码能力时编译前台焦点和窗口状态验收。
#![cfg(all(windows, feature = "agent-control", feature = "image-codecs"))]
// 允许断言型验收代码在原生窗口、语义动作或像素证据缺失处定向失败。
#![allow(
    clippy::expect_used,
    reason = "前台视觉验收必须在首个窗口状态、焦点环或几何不变量失败处给出明确诊断"
)]

// 接入唯一拥有主演示子进程与 discovery 生命周期的 fixture。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只依赖公开 uix.agent.v1 的协议 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;
// 接入真实 HWND 状态与像素证据的 Windows Adapter。
#[path = "support/uix_lang_graphics_recovery/visual_capture.rs"]
mod visual_capture;

// 导入证据路径值。
use std::path::{Path, PathBuf};
// 导入有界语义几何等待能力。
use std::thread;
// 导入轮询预算与截止时点。
use std::time::{Duration, Instant};

// 导入 JSON 动作与快照值。
use serde_json::{json, Value};

// 引入主演示进程 fixture。
use process::DemoProcess;
// 引入公开 Agent 客户端与逐窗身份。
use protocol::{AgentClient, AgentWindow};

// 保存连续最大化与还原压力轮数，对齐已完成 #808 门禁。
const MAXIMIZE_RESTORE_ROUNDS: usize = 20;

// 在真实 D3D11 主演示中验收窗口最大化还原和键盘/指针焦点像素。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_uix_lang_demo_preserves_window_geometry_and_focus_visuals() {
    // 启动显式 Agent 与强制 D3D11 的普通主演示。
    let mut demo = DemoProcess::spawn_main();
    // 等待同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接 fixture 唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 前台验收必须只观察一个主演示窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(window, 1);

    // 创建当前仓库 target 下的稳定证据目录。
    let evidence_root = evidence_root();
    // 精确清理本测试上次生成的证据目录。
    reset_evidence_root(&evidence_root);
    // 读取初始还原态语义几何。
    let initial = client.snapshot(window);
    // 初始窗口必须明确处于非最大化状态。
    visual_capture::wait_for_maximized(&demo, false);
    // 保存初始还原态证据路径。
    let restore_initial = evidence_root.join("restore-initial.png");
    // 捕获完整初始客户区。
    visual_capture::capture_png(&demo, &restore_initial);
    // 读取初始语义根逻辑范围。
    let initial_root = root_extent(&initial);

    // 连续执行二十轮产品窗口控制动作。
    for round in 1..=MAXIMIZE_RESTORE_ROUNDS {
        // 从当前快照取得最大化/还原控制的 opaque node ID。
        let maximize_target = maximize_restore_target(&client.snapshot(window));
        // 通过公开语义 invoke 进入 WindowControl 正常事件路径。
        let maximize_revision = client.perform(
            // 保持当前窗口身份。
            window,
            // 使用当前 generation 内的 opaque 节点身份。
            Some(maximize_target),
            // 使用公开 invoke 动作。
            json!({ "kind": "invoke" }),
        );
        // 等待动作 revision 完成真实 present。
        client.wait_for_presented(window, maximize_revision);
        // Win32 原生窗口必须进入最大化状态。
        visual_capture::wait_for_maximized(&demo, true);
        // 等待语义根采用新的最大化客户区。
        let maximized = wait_for_root_extent(&mut client, window, |extent| {
            // 最大化宽高都必须大于初始还原态。
            extent.0 > initial_root.0 + 1.0 && extent.1 > initial_root.1 + 1.0
        });
        // 第一轮保存代表性最大化视觉证据。
        if round == 1 {
            // 捕获最大化客户区供 AI 审阅。
            visual_capture::capture_png(&demo, &evidence_root.join("maximized.png"));
        }
        // 最大化根必须完整填充非空逻辑客户区。
        assert_shell_fills_root(&maximized, "maximized");

        // 从最大化快照重新取得当前 WindowControl 节点身份。
        let restore_target = maximize_restore_target(&client.snapshot(window));
        // 通过同一产品动作请求还原。
        let restore_revision = client.perform(
            // 保持当前窗口身份。
            window,
            // 使用当前快照节点身份。
            Some(restore_target),
            // 使用公开 invoke 动作。
            json!({ "kind": "invoke" }),
        );
        // 等待动作 revision 完成真实 present。
        client.wait_for_presented(window, restore_revision);
        // Win32 原生窗口必须退出最大化状态。
        visual_capture::wait_for_maximized(&demo, false);
        // 等待语义根与全部关键壳层矩形共同恢复初始状态。
        let restored = wait_for_shell_geometry(&mut client, window, &initial);
        // 每轮关键壳层几何都必须恢复初始值。
        assert_shell_geometry_matches(&initial, &restored, round);
    }
    // 保存最终还原态证据路径。
    let restore_final = evidence_root.join("restore-final.png");
    // 捕获二十轮后的完整客户区。
    visual_capture::capture_png(&demo, &restore_final);
    // 生成初始、最大化与最终还原审阅板。
    visual_capture::write_contact_sheet(
        // 保持窗口状态时间顺序。
        &[
            restore_initial.clone(),
            evidence_root.join("maximized.png"),
            restore_final.clone(),
        ],
        // 保存统一窗口几何审阅入口。
        &evidence_root.join("maximize-restore-contact.png"),
        // 三列直接比较状态变化与恢复。
        3,
    );

    // 在最终还原态验收键盘与指针输入模态的焦点可视差异。
    let focus_summary = verify_focus_visuals(&demo, &mut client, window, &evidence_root);

    // 关闭协议连接让子进程退出时不等待客户端。
    drop(client);
    // 回收唯一主演示并收集原生图形日志。
    let output = demo.stop_and_collect();
    // 启动必须选择生产 D3D11 retained RHI recipe。
    assert!(
        output.contains(
            "Graphics bootstrap: selected recipe backend=d3d11; raster=gpu_native; present=swapchain"
        ),
        "foreground demo did not select D3D11 recipe: {output}"
    );
    // 验收不得静默退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "foreground demo unexpectedly used software fallback: {output}"
    );
    // 输出不含 token 的证据位置摘要。
    println!(
        "uix-lang foreground evidence: {}; maximize_restore_rounds={MAXIMIZE_RESTORE_ROUNDS}; keyboard_to_pointer={:.2}%; pointer_to_keyboard={:.2}%",
        evidence_root.display(),
        focus_summary.0 * 100.0,
        focus_summary.1 * 100.0
    );
}

// 返回仓库 target 下不进入提交的前台视觉证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入人工与 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离前台焦点与窗口状态证据。
        .join("uix-lang-foreground")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &Path) {
    // 存在时只删除本测试的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous foreground evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create foreground evidence root");
}

// 从当前快照取得最大化/还原控制的 opaque 节点目标。
fn maximize_restore_target(snapshot: &Value) -> Value {
    // 查找具有运行时默认可访问名称的窗口控制。
    let control = snapshot["nodes"]
        // 节点数组必须由协议提供。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历全部当前节点。
        .iter()
        // 精确匹配公开 WindowControl 名称。
        .find(|node| node["name"] == "Maximize or restore window")
        // 缺失窗口控制时不能退化到私有 HWND 命令。
        .expect("maximize or restore semantic control");
    // 控制必须公开 invoke 动作。
    assert!(control["actions"]
        // 动作必须是数组。
        .as_array()
        // 精确包含 invoke。
        .is_some_and(|actions| actions.contains(&json!("invoke"))));
    // 返回公开 node_id 目标形状。
    json!({ "node_id": control["node_id"].clone() })
}

// 有界等待语义根满足当前窗口状态的范围谓词。
fn wait_for_root_extent(
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收主演示窗口身份。
    window: AgentWindow,
    // 接收当前状态范围谓词。
    predicate: impl Fn((f64, f64)) -> bool,
) -> Value {
    // 设置窗口布局收敛最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续读取派生语义树直到范围匹配。
    loop {
        // 读取当前语义快照。
        let snapshot = client.snapshot(window);
        // 根范围满足状态事实时返回同一快照。
        if predicate(root_extent(&snapshot)) {
            // 返回供后续壳层断言使用。
            return snapshot;
        }
        // 超时表示原生窗口与语义布局没有收敛。
        assert!(
            Instant::now() < deadline,
            "timed out waiting for semantic root extent"
        );
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 有界等待还原态全部关键壳层矩形与初始快照一致。
fn wait_for_shell_geometry(
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收主演示窗口身份。
    window: AgentWindow,
    // 接收初始还原态快照。
    initial: &Value,
) -> Value {
    // 设置分阶段 resize 与布局协调的最长收敛时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续读取快照直到根与全部壳层共同匹配。
    loop {
        // 读取当前派生语义树。
        let snapshot = client.snapshot(window);
        // 全部关键矩形匹配时返回同一快照。
        if shell_geometry_matches(initial, &snapshot) {
            // 返回供逐字段诊断断言使用。
            return snapshot;
        }
        // 超时表示布局没有在原生还原后完整收敛。
        assert!(
            Instant::now() < deadline,
            "timed out waiting for restored shell geometry"
        );
        // 短暂等待下一次窗口布局协调。
        thread::sleep(Duration::from_millis(25));
    }
}

// 判断根与关键壳层矩形是否都恢复到初始值。
fn shell_geometry_matches(initial: &Value, current: &Value) -> bool {
    // 根和四个稳定壳层目标必须共同匹配。
    [
        // 用空标识代表语义根。
        None,
        // 首页侧栏项。
        Some("sidebar-page-0"),
        // 页面主滚动容器。
        Some("main-page-scroll"),
        // 底部状态栏。
        Some("app-status-bar"),
        // 自定义标题栏拖拽区。
        Some("window-titlebar-drag"),
    ]
    // 按目标顺序遍历全部矩形。
    .into_iter()
    // 每个目标四字段都必须匹配。
    .all(|automation_id| {
        // 根据是否具有稳定标识选择初始节点。
        let before = automation_id.map_or_else(
            // 空标识读取语义根。
            || semantic_root(initial),
            // 稳定标识读取对应壳层节点。
            |automation_id| node_by_automation_id(initial, automation_id),
        );
        // 根据同一选择读取当前节点。
        let after = automation_id.map_or_else(
            // 空标识读取当前语义根。
            || semantic_root(current),
            // 稳定标识读取当前壳层节点。
            |automation_id| node_by_automation_id(current, automation_id),
        );
        // 比较可见矩形四个逻辑字段。
        ["x", "y", "w", "h"].into_iter().all(|field| {
            // 缺失字段暂时视为尚未收敛。
            let Some(expected) = before["visible_bounds"][field].as_f64() else {
                // 初始快照缺失关键几何不能构成匹配。
                return false;
            };
            // 当前字段缺失时继续等待。
            let Some(actual) = after["visible_bounds"][field].as_f64() else {
                // 当前布局尚未发布完整几何。
                return false;
            };
            // 平台浮点布局允许半逻辑像素误差。
            (actual - expected).abs() <= 0.5
        })
    })
}

// 读取语义根逻辑宽高。
fn root_extent(snapshot: &Value) -> (f64, f64) {
    // 查找 parent 为空的唯一语义根。
    let root = semantic_root(snapshot);
    // 根宽度必须是正数。
    let width = root["visible_bounds"]["w"]
        // 读取逻辑宽度。
        .as_f64()
        // 缺失宽度属于布局快照错误。
        .expect("semantic root width");
    // 根高度必须是正数。
    let height = root["visible_bounds"]["h"]
        // 读取逻辑高度。
        .as_f64()
        // 缺失高度属于布局快照错误。
        .expect("semantic root height");
    // 返回当前客户区逻辑范围。
    (width, height)
}

// 查找当前快照唯一语义根。
fn semantic_root(snapshot: &Value) -> &Value {
    // 遍历当前窗口全部节点。
    snapshot["nodes"]
        // 节点数组必须存在。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历节点借用。
        .iter()
        // 根节点具有空 parent。
        .find(|node| node["parent"].is_null())
        // 缺失根节点表示语义树无效。
        .expect("semantic root")
}

// 验证当前语义根完整填充非空客户区。
fn assert_shell_fills_root(snapshot: &Value, label: &str) {
    // 取得当前语义根。
    let root = semantic_root(snapshot);
    // 根必须从客户区逻辑原点开始。
    assert_eq!(root["visible_bounds"]["x"].as_f64(), Some(0.0), "{label}");
    // 根必须从客户区逻辑原点开始。
    assert_eq!(root["visible_bounds"]["y"].as_f64(), Some(0.0), "{label}");
    // 根逻辑范围必须非空。
    let extent = root_extent(snapshot);
    // 两轴都必须是正数。
    assert!(extent.0 > 0.0 && extent.1 > 0.0, "{label}");
}

// 比较初始与每轮最终还原态的关键壳层矩形。
fn assert_shell_geometry_matches(initial: &Value, restored: &Value, round: usize) {
    // 覆盖根、侧栏项、主滚动区、底部状态栏和标题拖拽区。
    for automation_id in [
        // 首页侧栏项。
        "sidebar-page-0",
        // 页面主滚动容器。
        "main-page-scroll",
        // 底部状态栏。
        "app-status-bar",
        // 自定义标题栏拖拽区。
        "window-titlebar-drag",
    ] {
        // 读取初始还原态的当前可见矩形。
        let before = &node_by_automation_id(initial, automation_id)["visible_bounds"];
        // 读取本轮还原态的当前可见矩形。
        let after = &node_by_automation_id(restored, automation_id)["visible_bounds"];
        // 四个几何字段逐一要求稳定。
        for field in ["x", "y", "w", "h"] {
            // 初始字段必须是数值。
            let expected = before[field].as_f64().expect("initial shell geometry");
            // 还原字段必须是数值。
            let actual = after[field].as_f64().expect("restored shell geometry");
            // 平台浮点布局允许半像素以内差异。
            assert!(
                (actual - expected).abs() <= 0.5,
                "restore round {round}: {automation_id}.{field} expected {expected}, got {actual}"
            );
        }
    }
}

// 验证键盘与指针输入模态对焦点环产生对称可见变化。
fn verify_focus_visuals(
    // 接收主演示进程 fixture。
    demo: &DemoProcess,
    // 接收公开协议客户端。
    client: &mut AgentClient,
    // 接收主演示窗口身份。
    window: AgentWindow,
    // 接收本测试证据根目录。
    evidence_root: &Path,
) -> (f64, f64) {
    // 读取当前还原态快照。
    let snapshot = client.snapshot(window);
    // 读取首页侧栏项语义边界。
    let bounds = &node_by_automation_id(&snapshot, "sidebar-page-0")["visible_bounds"];
    // 读取目标左坐标。
    let x = bounds["x"].as_f64().expect("home nav x");
    // 读取目标上坐标。
    let y = bounds["y"].as_f64().expect("home nav y");
    // 读取目标宽度。
    let width = bounds["w"].as_f64().expect("home nav width");
    // 读取目标高度。
    let height = bounds["h"].as_f64().expect("home nav height");
    // 保存逻辑目标区域。
    let region = (x, y, width, height);
    // 保存当前语义根逻辑范围供 DPI 映射。
    let root = root_extent(&snapshot);

    // 先通过公开 focus 动作建立键盘焦点环基线。
    let keyboard_before_revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 使用稳定侧栏自动化标识。
        Some(json!({ "automation_id": "sidebar-page-0" })),
        // 使用公开 focus 动作。
        json!({ "kind": "focus" }),
    );
    // 等待键盘焦点基线完成真实 present。
    client.wait_for_presented(window, keyboard_before_revision);
    // 保存首次键盘焦点证据路径。
    let keyboard_before = evidence_root.join("focus-keyboard-before.png");
    // 捕获带键盘焦点环的客户区。
    visual_capture::capture_png(demo, &keyboard_before);

    // 使用公开窗口 click_at 动作切到指针输入模态。
    let pointer_revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 窗口级动作不能携带组件目标。
        None,
        // 点击侧栏项中心。
        json!({
            "kind": "click_at",
            "x": x + width * 0.5,
            "y": y + height * 0.5,
        }),
    );
    // 等待指针焦点状态完成真实 present。
    client.wait_for_presented(window, pointer_revision);
    // 指针点击后逻辑焦点仍应属于同一目标。
    assert_eq!(
        node_by_automation_id(&client.snapshot(window), "sidebar-page-0")["focused"],
        true
    );
    // 保存指针焦点证据路径。
    let pointer = evidence_root.join("focus-pointer.png");
    // 捕获隐藏键盘焦点环的指针态。
    visual_capture::capture_png(demo, &pointer);

    // 再次使用公开 focus 动作恢复键盘焦点模态。
    let keyboard_after_revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 使用同一稳定侧栏目标。
        Some(json!({ "automation_id": "sidebar-page-0" })),
        // 使用公开 focus 动作。
        json!({ "kind": "focus" }),
    );
    // 等待键盘焦点环恢复完成真实 present。
    client.wait_for_presented(window, keyboard_after_revision);
    // 保存最终键盘焦点证据路径。
    let keyboard_after = evidence_root.join("focus-keyboard-after.png");
    // 捕获恢复后的键盘焦点态。
    visual_capture::capture_png(demo, &keyboard_after);

    // 计算键盘基线到指针态的目标区域像素变化比例。
    let pointer_delta = visual_capture::changed_pixel_ratio_in_logical_region(
        // 左侧键盘基线。
        &keyboard_before,
        // 右侧指针态。
        &pointer,
        // 语义根逻辑宽度。
        root.0,
        // 语义根逻辑高度。
        root.1,
        // 侧栏目标逻辑矩形。
        region,
    );
    // 计算指针态到键盘恢复态的目标区域像素变化比例。
    let keyboard_delta = visual_capture::changed_pixel_ratio_in_logical_region(
        // 左侧指针态。
        &pointer,
        // 右侧键盘恢复态。
        &keyboard_after,
        // 语义根逻辑宽度。
        root.0,
        // 语义根逻辑高度。
        root.1,
        // 侧栏目标逻辑矩形。
        region,
    );
    // 指针态必须移除可见焦点环并产生至少百分之二区域变化。
    assert!(
        pointer_delta >= 0.02,
        "pointer modality changed only {:.2}% of navigation target",
        pointer_delta * 100.0
    );
    // 键盘态必须恢复可见焦点环并产生至少百分之二区域变化。
    assert!(
        keyboard_delta >= 0.02,
        "keyboard modality changed only {:.2}% of navigation target",
        keyboard_delta * 100.0
    );
    // 生成键盘、指针、键盘三格焦点审阅板。
    visual_capture::write_contact_sheet(
        // 保持输入模态时间顺序。
        &[
            keyboard_before.clone(),
            pointer.clone(),
            keyboard_after.clone(),
        ],
        // 保存统一焦点审阅入口。
        &evidence_root.join("focus-contact.png"),
        // 三列直接比较焦点环消失与恢复。
        3,
    );
    // 返回两段区域像素变化摘要。
    (pointer_delta, keyboard_delta)
}

// 按稳定 automation ID 查找语义节点。
fn node_by_automation_id<'a>(snapshot: &'a Value, automation_id: &str) -> &'a Value {
    // 读取节点数组并查找精确标识。
    snapshot["nodes"]
        // 转为数组借用。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历全部语义节点。
        .iter()
        // 精确匹配稳定标识。
        .find(|node| node["automation_id"] == automation_id)
        // 缺失时报告具体标识。
        .unwrap_or_else(|| panic!("missing semantic target {automation_id}"))
}
