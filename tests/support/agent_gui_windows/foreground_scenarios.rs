//! 真实 Win32 主题、尺寸与输入焦点可视场景。

// 复用 foreground owner 提供的窗口控制、捕获与 agent 交换辅助。
use super::*;

// 让 agent_gui_windows 父模块继续调用主题与尺寸场景入口。
pub(in super::super) fn verify_theme_and_resize_capture(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
) {
    let window = demo.window_handle();
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let before = capture_client(window);
    assert_meaningful_capture(&before, "before theme");

    let toggled = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "capture-theme-toggle",
        Some(json!({ "automation_id": "theme-toggle" })),
        json!({ "kind": "invoke" }),
    );
    let theme_revision = toggled["revision"].as_u64().expect("theme revision");
    wait_for_presented(
        connection,
        window_id,
        generation,
        theme_revision,
        "capture-theme-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let themed = capture_client(window);
    assert_meaningful_capture(&themed, "after theme");
    assert_eq!((themed.width, themed.height), (before.width, before.height));
    let changed_pixels = before
        .pixels
        .iter()
        .zip(&themed.pixels)
        .filter(|(left, right)| (*left ^ *right) & 0x00FF_FFFF != 0)
        .count();
    let changed_ratio = changed_pixels as f64 / before.pixels.len().max(1) as f64;
    assert!(
        changed_ratio >= 0.10,
        "theme toggle changed only {:.2}% of foreground pixels",
        changed_ratio * 100.0
    );

    let snapshot = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "capture-before-resize",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&snapshot, "capture-before-resize");
    let before_layout = assert_shell_fills_snapshot(&snapshot["snapshot"], "before resize");
    let presented_revision = snapshot["snapshot"]["presented_revision"]
        .as_u64()
        .expect("presented revision");
    resize_window(window);
    wait_for_presented(
        connection,
        window_id,
        generation,
        presented_revision + 1,
        "capture-resize-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let resized = capture_client(window);
    assert_meaningful_capture(&resized, "after resize");
    assert_ne!(
        (resized.width, resized.height),
        (themed.width, themed.height)
    );
    let resized_snapshot = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "capture-after-resize",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&resized_snapshot, "capture-after-resize");
    let resized_layout = assert_shell_fills_snapshot(&resized_snapshot["snapshot"], "after resize");
    assert_ne!(
        (before_layout.0, before_layout.1),
        (resized_layout.0, resized_layout.1),
        "root logical viewport must change after the native client resize"
    );
    assert_ne!(
        (before_layout.2, before_layout.3),
        (resized_layout.2, resized_layout.3),
        "page scroll viewport must reflow after the native client resize"
    );
    println!(
        "foreground capture: {}x{} -> theme delta {:.2}% -> {}x{}",
        before.width,
        before.height,
        changed_ratio * 100.0,
        resized.width,
        resized.height
    );
}

// 让 agent_gui_windows 父模块继续调用指针与键盘焦点场景入口。
pub(in super::super) fn verify_pointer_and_keyboard_focus_visuals(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    _snapshot: &Value,
) {
    // 最大化与还原可能改变窗口代际内的逻辑范围，焦点场景必须读取当前快照。
    let current = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "focus-visual-current-snapshot",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    // 快照失败时不得继续用旧坐标制造无效像素结论。
    assert_success(&current, "focus-visual-current-snapshot");
    // 后续目标定位与物理映射统一使用同一份当前语义快照。
    let snapshot = &current["snapshot"];
    let window = demo.window_handle();
    let bounds = &node_by_automation_id(snapshot, "sidebar-page-0")["visible_bounds"];
    let x = bounds["x"].as_f64().expect("home nav x");
    let y = bounds["y"].as_f64().expect("home nav y");
    let w = bounds["w"].as_f64().expect("home nav width");
    let h = bounds["h"].as_f64().expect("home nav height");
    let region = (x, y, w, h);

    demo.raise_for_interaction();
    request_foreground_focus(window);
    // 先显式建立键盘焦点基线，避免继承标题栏操作留下的不确定输入模态。
    let keyboard_before = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "focus-visual-keyboard-before",
        Some(json!({ "automation_id": "sidebar-page-0" })),
        json!({ "kind": "focus" }),
    );
    // 等待键盘焦点基线真正呈现后再采集像素。
    wait_for_presented(
        connection,
        window_id,
        generation,
        keyboard_before["revision"]
            .as_u64()
            .expect("keyboard baseline revision"),
        "focus-visual-keyboard-before-presented",
    );
    // 固定窗口前台状态，排除非客户区激活变化。
    demo.raise_for_interaction();
    // 保持原生窗口拥有输入焦点。
    request_foreground_focus(window);
    // 等待桌面合成器提交键盘焦点基线。
    flush_desktop_composition();
    // 捕获带键盘焦点环的稳定基线。
    let keyboard_before = capture_client(window);
    // 保存首次键盘聚焦态，供前后对称性审阅。
    capture_focus_visual_evidence(demo, "focus-keyboard-before.png");
    // 将语义逻辑坐标映射到客户区物理像素，避免 DPI 缩放稀释焦点环比例。
    let region = physical_capture_region(snapshot, &keyboard_before, region);

    let pointer = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "focus-visual-pointer",
        None,
        json!({ "kind": "click_at", "x": x + w * 0.5, "y": y + h * 0.5 }),
    );
    wait_for_presented(
        connection,
        window_id,
        generation,
        pointer["revision"].as_u64().expect("pointer revision"),
        "focus-visual-pointer-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let pointer_focused = capture_client(window);
    // 保存指针聚焦态，供焦点环与普通活动项重绘分离比对。
    capture_focus_visual_evidence(demo, "focus-pointer.png");
    // 指针输入应移除键盘焦点环并产生明确像素差异。
    let pointer_delta = capture_region_change_ratio(&keyboard_before, &pointer_focused, region);

    let keyboard = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "focus-visual-keyboard",
        Some(json!({ "automation_id": "sidebar-page-0" })),
        json!({ "kind": "focus" }),
    );
    wait_for_presented(
        connection,
        window_id,
        generation,
        keyboard["revision"].as_u64().expect("keyboard revision"),
        "focus-visual-keyboard-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let keyboard_focused = capture_client(window);
    // 保存键盘聚焦态，供失败时直接审阅可见像素而不依赖日志推断。
    capture_focus_visual_evidence(demo, "focus-keyboard.png");
    let keyboard_delta = capture_region_change_ratio(&pointer_focused, &keyboard_focused, region);
    // 指针态相对键盘基线必须移除可见焦点环。
    assert!(
        pointer_delta >= 0.02,
        "pointer focus changed only {:.2}% of the navigation item; keyboard focus ring must be removed",
        pointer_delta * 100.0
    );
    assert!(
        keyboard_delta >= 0.02,
        "keyboard focus changed only {:.2}% of the navigation item; focus ring must remain visible",
        keyboard_delta * 100.0
    );
    println!(
        "focus visual capture: keyboard-to-pointer delta {:.2}% -> pointer-to-keyboard delta {:.2}%",
        pointer_delta * 100.0,
        keyboard_delta * 100.0
    );
}

// 将语义快照中的逻辑矩形转换为 Win32 客户区捕获使用的物理像素矩形。
fn physical_capture_region(
    snapshot: &Value,
    capture: &ClientCapture,
    region: (f64, f64, f64, f64),
) -> (f64, f64, f64, f64) {
    // 语义根节点拥有当前窗口完整逻辑范围。
    let root = snapshot["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["parent"].is_null()))
        .expect("focus snapshot root");
    // 逻辑宽度必须可用于建立水平缩放。
    let root_width = root["visible_bounds"]["w"]
        .as_f64()
        .expect("focus snapshot root width");
    // 逻辑高度必须可用于建立垂直缩放。
    let root_height = root["visible_bounds"]["h"]
        .as_f64()
        .expect("focus snapshot root height");
    // 空逻辑客户区不能产生有效的像素门禁。
    assert!(root_width > 0.0 && root_height > 0.0);
    // 水平比例来自同一客户区的物理捕获宽度。
    let scale_x = capture.width as f64 / root_width;
    // 垂直比例来自同一客户区的物理捕获高度。
    let scale_y = capture.height as f64 / root_height;
    // 输出稳定映射证据，便于 DPI 环境下审计门禁实际覆盖区域。
    println!(
        "focus pixel mapping: logical_root={root_width:.2}x{root_height:.2}; capture={}x{}; scale={scale_x:.4}x{scale_y:.4}; logical_region={region:?}",
        capture.width, capture.height
    );
    // 对位置与尺寸应用各自轴向比例，兼容非整数 DPI。
    (
        region.0 * scale_x,
        region.1 * scale_y,
        region.2 * scale_x,
        region.3 * scale_y,
    )
}

// 按需保存 #995 的真实客户区证据，不配置目录时不增加测试产物。
fn capture_focus_visual_evidence(demo: &DemoProcess, file_name: &str) {
    // 使用独立环境变量，避免覆盖全页面视觉矩阵的产物目录。
    let Some(root) = std::env::var_os("UIX_FOCUS_VISUAL_EVIDENCE_DIR") else {
        // 常规真窗回归保持无额外文件副作用。
        return;
    };
    // 将稳定文件名拼入本轮授权的精确证据目录。
    let path = std::path::PathBuf::from(root).join(file_name);
    // 复用同一 Win32 客户区捕获入口，保证像素语义与门禁一致。
    capture_demo_client_png(demo, &path);
}

fn capture_region_change_ratio(
    left: &ClientCapture,
    right: &ClientCapture,
    region: (f64, f64, f64, f64),
) -> f64 {
    assert_eq!(
        (left.width, left.height),
        (right.width, right.height),
        "regional captures must have the same client extent"
    );
    let x0 = region.0.floor().max(0.0) as usize;
    let y0 = region.1.floor().max(0.0) as usize;
    let x1 = (region.0 + region.2).ceil().max(0.0) as usize;
    let y1 = (region.1 + region.3).ceil().max(0.0) as usize;
    let width = left.width.max(1) as usize;
    let height = left.height.max(1) as usize;
    let x1 = x1.min(width);
    let y1 = y1.min(height);
    assert!(x0 < x1 && y0 < y1, "focus region must be visible");

    let mut changed = 0usize;
    let mut total = 0usize;
    for y in y0.min(height)..y1 {
        let row = y * width;
        for x in x0.min(width)..x1 {
            total += 1;
            if (left.pixels[row + x] ^ right.pixels[row + x]) & 0x00FF_FFFF != 0 {
                changed += 1;
            }
        }
    }
    changed as f64 / total.max(1) as f64
}

fn assert_shell_fills_snapshot(snapshot: &Value, label: &str) -> (f64, f64, f64, f64) {
    let nodes = snapshot["nodes"]
        .as_array()
        .expect("semantic snapshot nodes");
    let root = nodes
        .iter()
        .find(|node| node["parent"].is_null())
        .expect("semantic root node");
    let root_bounds = &root["visible_bounds"];
    let root_w = root_bounds["w"].as_f64().expect("root visible width");
    let root_h = root_bounds["h"].as_f64().expect("root visible height");
    assert!(root_w > 0.0 && root_h > 0.0, "{label}: empty root bounds");
    assert_eq!(root_bounds["x"].as_f64(), Some(0.0), "{label}: root x");
    assert_eq!(root_bounds["y"].as_f64(), Some(0.0), "{label}: root y");

    let title_bar = &node_by_automation_id(snapshot, "window-titlebar")["visible_bounds"];
    assert_eq!(title_bar["x"].as_f64(), Some(0.0), "{label}: title x");
    assert_eq!(title_bar["y"].as_f64(), Some(0.0), "{label}: title y");
    assert!(
        (title_bar["w"].as_f64().expect("title width") - root_w).abs() < 0.5,
        "{label}: title bar must span root width"
    );
    assert!(
        title_bar["h"].as_f64().expect("title height") > 0.0,
        "{label}: empty title bar"
    );

    let status = &node_by_automation_id(snapshot, "app-status-bar")["visible_bounds"];
    let status_bottom =
        status["y"].as_f64().expect("status y") + status["h"].as_f64().expect("status height");
    assert!(
        (status_bottom - root_h).abs() < 0.5,
        "{label}: status bar bottom {status_bottom} must meet root bottom {root_h}"
    );

    let page = &node_by_automation_id(snapshot, "page-scroll-0")["visible_bounds"];
    let page_w = page["w"].as_f64().expect("page visible width");
    let page_h = page["h"].as_f64().expect("page visible height");
    assert!(page_w > 0.0 && page_h > 0.0, "{label}: empty page viewport");
    (root_w, root_h, page_w, page_h)
}
