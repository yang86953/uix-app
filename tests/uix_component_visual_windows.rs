// 只在 Windows 同时启用 Agent 与图像编码能力时编译组件真窗验收。
#![cfg(all(windows, feature = "agent-control", feature = "image-codecs"))]
// 允许断言型验收代码使用场景化 expect 诊断。
#![allow(
    clippy::expect_used,
    reason = "组件真窗验收必须在首个缺失窗口、节点或证据文件处给出明确失败"
)]

// 接入唯一拥有验收子进程与 discovery 生命周期的 fixture。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只依赖公开 uix.agent.v1 的协议 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;
// 接入只负责真实 HWND 像素证据的 Windows Adapter。
#[path = "support/uix_lang_graphics_recovery/visual_capture.rs"]
mod visual_capture;

// 引入证据文件路径。
use std::path::{Path, PathBuf};
// 引入折叠动画收敛所需短时等待。
use std::thread;
// 引入有界动画等待时长。
use std::time::Duration;

// 引入 JSON 动作与快照值。
use serde_json::{Value, json};

// 引入独立验收进程 fixture。
use process::DemoProcess;
// 引入公开 Agent 客户端与窗口身份。
use protocol::{AgentClient, AgentWindow};

// 在真实 D3D11 窗口中验收四个受控组件的视觉与正常交互路径。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_registered_components_cover_pointer_keyboard_and_dynamic_visual_states() {
    // 创建本测试唯一稳定证据目录。
    let evidence_root = evidence_root();
    // 清理本测试上次生成的精确证据目录。
    reset_evidence_root(&evidence_root);
    // 保存所有正式截图供最终审阅板拼接。
    let mut images = Vec::new();

    // 验收 Badge 单 child、焦点穿透与动态装饰清除。
    accept_badge(&evidence_root, &mut images);
    // 验收 SelectableList 稳定 id 的指针与键盘写回。
    accept_selectable_list(&evidence_root, &mut images);
    // 验收 Collapse 稳定 key、手风琴与同名面板分离。
    accept_collapse(&evidence_root, &mut images);
    // 验收 Upload 四态队列与含逗号文件删除收缩。
    accept_upload(&evidence_root, &mut images);

    // 生成覆盖全部交互状态的单一视觉审阅板。
    visual_capture::write_contact_sheet(
        // 借用按场景顺序保存的无损截图。
        &images,
        // 固定输出路径便于 AI 与 CI 审阅。
        &evidence_root.join("components-interaction-contact.png"),
        // 每行四张保持各应用仍可辨认。
        4,
    );
    // 输出不含 token 的稳定证据位置。
    println!("component visual evidence: {}", evidence_root.display());
}

// 在真实 D3D11 窗口中验收 Popconfirm 六种位置与双输入动作。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_popconfirm_covers_placements_pointer_keyboard_and_counts() {
    // 创建 Popconfirm 专属稳定证据目录。
    let evidence_root = popconfirm_evidence_root();
    // 清理本测试上次生成的精确证据目录。
    reset_evidence_root(&evidence_root);
    // 保存全部位置与动作截图供最终审阅板拼接。
    let mut images = Vec::new();
    // 启动只拥有 Popconfirm 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("popconfirm-visual");
    // 读取首帧语义快照。
    let initial = client.snapshot(window);
    // 初始确认计数必须为零。
    assert_eq!(
        node_by_automation_id(&initial, "popconfirm-confirm-count")["name"],
        "确认次数：0"
    );
    // 初始取消计数必须为零。
    assert_eq!(
        node_by_automation_id(&initial, "popconfirm-cancel-count")["name"],
        "取消次数：0"
    );
    // 保存六个长短自定义 trigger 的初始视觉证据。
    capture(
        &demo,
        &evidence_root,
        "popconfirm-01-initial.png",
        &mut images,
    );

    // 定义六种作者 placement 与稳定 trigger 的验收顺序。
    let placements = [
        // 覆盖顶部左对齐。
        (
            "popconfirm-top-left",
            "popconfirm-trigger-top-left",
            "popconfirm-02-top-left.png",
        ),
        // 覆盖顶部居中。
        (
            "popconfirm-top",
            "popconfirm-trigger-top",
            "popconfirm-03-top.png",
        ),
        // 覆盖顶部右对齐。
        (
            "popconfirm-top-right",
            "popconfirm-trigger-top-right",
            "popconfirm-04-top-right.png",
        ),
        // 覆盖底部左对齐。
        (
            "popconfirm-bottom-left",
            "popconfirm-trigger-bottom-left",
            "popconfirm-05-bottom-left.png",
        ),
        // 覆盖底部居中与无图标。
        (
            "popconfirm-bottom",
            "popconfirm-trigger-bottom",
            "popconfirm-06-bottom.png",
        ),
        // 覆盖底部右对齐与无箭头。
        (
            "popconfirm-bottom-right",
            "popconfirm-trigger-bottom-right",
            "popconfirm-07-bottom-right.png",
        ),
    ];
    // 逐个通过真实指针打开，再用 Escape 进入统一取消路径。
    for (index, (owner_id, trigger_id, image_name)) in placements.iter().enumerate() {
        // 读取当前 trigger 的真实可见边界。
        let snapshot = client.snapshot(window);
        // 取得当前 placement 的真实 trigger 矩形。
        let (x, y, width, height) = visible_bounds(node_by_automation_id(&snapshot, trigger_id));
        // 在 trigger 中心执行真实指针点击。
        click_at(&mut client, window, x + width * 0.5, y + height * 0.5);
        // 打开态必须公开展开状态与默认确认动作。
        let opened = client.snapshot(window);
        // 当前 owner 必须处于稳定打开态。
        assert_eq!(
            node_by_automation_id(&opened, owner_id)["state"]["expanded"],
            true
        );
        // 默认虚拟动作必须指向确认按钮。
        assert_eq!(
            node_by_automation_id(&opened, owner_id)["state"]["value_text"],
            "OK"
        );
        // 保存当前 placement 的真实浮层像素。
        capture(&demo, &evidence_root, image_name, &mut images);
        // Escape 必须进入统一用户取消动作。
        press_key(&mut client, window, "escape");
        // 读取取消后的语义与计数终态。
        let closed = client.snapshot(window);
        // 当前 owner 必须离开稳定打开态。
        assert_eq!(
            node_by_automation_id(&closed, owner_id)["state"]["expanded"],
            false
        );
        // 焦点必须归还真实 trigger 子树。
        assert_eq!(node_by_automation_id(&closed, trigger_id)["focused"], true);
        // 每轮 Escape 只能增加一次取消计数。
        assert_eq!(
            node_by_automation_id(&closed, "popconfirm-cancel-count")["name"],
            format!("取消次数：{}", index + 1)
        );
        // 等待离场动画释放全局指针捕获，再切换到下一独立 placement。
        thread::sleep(Duration::from_millis(250));
    }

    // 通过公开焦点动作选择顶部左侧真实 trigger。
    focus_automation(&mut client, window, "popconfirm-trigger-top-left");
    // Enter 必须从真实 trigger 键盘路径打开确认框。
    press_key(&mut client, window, "enter");
    // Right 必须选择取消虚拟动作。
    press_key(&mut client, window, "right");
    // 读取方向键选择后的语义状态。
    let cancel_selected = client.snapshot(window);
    // 取消动作必须同时公开名称与一基序号二。
    assert_eq!(
        node_by_automation_id(&cancel_selected, "popconfirm-top-left")["state"]["value_text"],
        "Cancel"
    );
    // Left 必须恢复确认虚拟动作。
    press_key(&mut client, window, "left");
    // Enter 必须提交唯一确认回调并完成呈现。
    press_key(&mut client, window, "enter");
    // 读取键盘确认后的计数与关闭状态。
    let keyboard_confirmed = client.snapshot(window);
    // 键盘确认只能增加一次确认计数。
    assert_eq!(
        node_by_automation_id(&keyboard_confirmed, "popconfirm-confirm-count")["name"],
        "确认次数：1"
    );
    // 确认后 owner 必须关闭。
    assert_eq!(
        node_by_automation_id(&keyboard_confirmed, "popconfirm-top-left")["state"]["expanded"],
        false
    );
    // 等待确认离场动画结束，使截图明确显示 trigger 焦点环与稳定计数。
    thread::sleep(Duration::from_millis(250));
    // 保存键盘确认后的焦点与计数证据。
    capture(
        &demo,
        &evidence_root,
        "popconfirm-08-keyboard-confirm.png",
        &mut images,
    );

    // 读取底部左侧 trigger 作为鼠标动作几何锚点。
    let pointer_snapshot = client.snapshot(window);
    // 取得底部浮层不会翻转时的稳定 trigger 边界。
    let (x, y, width, height) = visible_bounds(node_by_automation_id(
        &pointer_snapshot,
        "popconfirm-trigger-bottom-left",
    ));
    // 鼠标点击 trigger 打开底部左侧确认框。
    click_at(&mut client, window, x + width * 0.5, y + height * 0.5);
    // 点击标准 200x110 浮层内确认按钮中心。
    click_at(&mut client, window, x + 54.0, y + height + 97.0);
    // 读取鼠标确认后的终态。
    let pointer_confirmed = client.snapshot(window);
    // 鼠标确认必须只增加一次确认计数。
    assert_eq!(
        node_by_automation_id(&pointer_confirmed, "popconfirm-confirm-count")["name"],
        "确认次数：2"
    );
    // 再次打开同一底部左侧确认框。
    click_at(&mut client, window, x + width * 0.5, y + height * 0.5);
    // 点击标准浮层内取消按钮中心。
    click_at(&mut client, window, x + 146.0, y + height + 97.0);
    // 读取鼠标取消后的终态。
    let pointer_cancelled = client.snapshot(window);
    // 鼠标取消必须在六次 Escape 后只再增加一次。
    assert_eq!(
        node_by_automation_id(&pointer_cancelled, "popconfirm-cancel-count")["name"],
        "取消次数：7"
    );
    // 第三次打开用于外部点击取消验收。
    click_at(&mut client, window, x + width * 0.5, y + height * 0.5);
    // 点击页面右下空白区域触发外部取消。
    click_at(&mut client, window, 1000.0, 480.0);
    // 读取外部点击后的终态。
    let outside_cancelled = client.snapshot(window);
    // 外部点击必须再且仅再增加一次取消计数。
    assert_eq!(
        node_by_automation_id(&outside_cancelled, "popconfirm-cancel-count")["name"],
        "取消次数：8"
    );
    // 等待外部取消离场动画结束，使最终证据不混入残留浮层。
    thread::sleep(Duration::from_millis(250));
    // 保存鼠标确认、取消与外部点击后的最终证据。
    capture(
        &demo,
        &evidence_root,
        "popconfirm-09-pointer-final.png",
        &mut images,
    );

    // 拼接单一视觉审阅板覆盖初始、六位置和双输入终态。
    visual_capture::write_contact_sheet(
        &images,
        &evidence_root.join("popconfirm-interaction-contact.png"),
        3,
    );
    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 Popconfirm 验收窗口并收集进程输出。
    let _ = demo.stop_and_collect();
    // 输出不含 token 的稳定证据位置。
    println!("popconfirm visual evidence: {}", evidence_root.display());
}

// 在真实 D3D11 窗口中验收 overlay backdrop blur 的背景/前景分离视觉。
#[test]
#[ignore = "requires an interactive Windows desktop and writes PNG visual evidence"]
fn real_overlay_backdrop_blur_keeps_foreground_sharp() {
    // 创建本测试唯一稳定证据目录。
    let evidence_root = backdrop_evidence_root();
    // 清理本测试上次生成的精确证据目录。
    reset_evidence_root(&evidence_root);
    // 启动只拥有 backdrop blur 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("backdrop-visual");
    // 读取首帧派生语义树，确保前景 Modal 真实呈现。
    let snapshot = client.snapshot(window);
    // Modal 标题必须通过公开语义树可观察。
    assert_eq!(
        // 按精确可访问名称定位前景。
        node_by_name(&snapshot, "Overlay Backdrop Blur 已启用")["role"],
        // Modal 使用 dialog 语义。
        "dialog"
    );
    // 等待 Modal 入场动画与 effect 事务收敛后再捕获稳定像素。
    thread::sleep(Duration::from_millis(500));
    // 捕获真实 HWND 无损像素证据。
    let evidence = evidence_root.join("backdrop-blur-d3d11.png");
    // 由 Win32 Adapter 捕获客户区，不读取 renderer 私有状态。
    visual_capture::capture_png(&demo, &evidence);
    // 固定条纹边界必须保留两侧颜色，并在边界两边形成多像素混合。
    visual_capture::assert_backdrop_blur_transition(&evidence);
    // 输出稳定证据位置供 AI 视觉审阅。
    println!("overlay backdrop visual evidence: {}", evidence.display());
    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 backdrop 验收窗口。
    let _ = demo.stop_and_collect();
}

// 验收 Badge 的布局锚点、指针、Tab 顺序与动态显隐残影。
fn accept_badge(evidence_root: &Path, images: &mut Vec<PathBuf>) {
    // 启动只拥有 Badge 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("badge-visual");
    // 读取首帧派生语义树。
    let initial = client.snapshot(window);
    // 初始数字 Badge 必须公开真实计数。
    assert_eq!(node_by_name(&initial, "5")["role"], "status");
    // 文字胶囊必须公开独立状态语义。
    assert_eq!(node_by_name(&initial, "NEW")["role"], "status");
    // 保存初始全部 Badge 形态。
    capture(&demo, evidence_root, "badge-01-initial.png", images);

    // 通过真实客户区坐标点击 Badge 内唯一真实按钮。
    click_named(&mut client, window, &initial, "点击增加计数：5");
    // 读取状态写回并重新协调后的语义树。
    let counted = client.snapshot(window);
    // 子按钮事件必须穿过透明装饰器并更新业务状态。
    assert_eq!(node_by_name(&counted, "点击增加计数：6")["role"], "button");
    // Badge 自身必须同步公开新数字。
    assert_eq!(node_by_name(&counted, "6")["state"]["value_now"], 6.0);
    // 保存点击后的数字与按钮状态。
    capture(&demo, evidence_root, "badge-02-counted.png", images);

    // 显式聚焦首个真实子按钮以建立确定 Tab 起点。
    focus_named(&mut client, window, &counted, "点击增加计数：6");
    // Tab 必须越过不可聚焦 Badge 与 Avatar 进入圆点切换按钮。
    press_key(&mut client, window, "tab");
    // 读取第一轮 Tab 后的焦点事实。
    let second_focus = client.snapshot(window);
    // 焦点只能落在真实子按钮。
    assert_eq!(focused_name(&second_focus), "切换圆点：true");
    // 下一次 Tab 必须落到 NEW Badge 内的真实按钮。
    press_key(&mut client, window, "tab");
    // 读取第二轮 Tab 后的焦点事实。
    let third_focus = client.snapshot(window);
    // 装饰节点和零 child Badge 都不得进入 Tab 顺序。
    assert_eq!(focused_name(&third_focus), "查看更新");

    // 通过真实指针路径切换 Avatar 右上角圆点。
    click_named(&mut client, window, &third_focus, "切换圆点：true");
    // 读取切换后的声明状态投影。
    let hidden = client.snapshot(window);
    // 动态按钮文本证明 show_dot 已由 true 写回 false。
    assert_eq!(node_by_name(&hidden, "切换圆点：false")["role"], "button");
    // 给声明协调后的旧装饰损伤一段有界提交时间。
    thread::sleep(Duration::from_millis(150));
    // 保存圆点隐藏后的清理证据。
    let hidden_path = capture(&demo, evidence_root, "badge-03-dot-hidden.png", images);
    // 取得动态切换前的截图路径。
    let counted_path = evidence_root.join("badge-02-counted.png");
    // 圆点锚点局部必须发生像素变化，排除状态写回但未重绘。
    let dot_delta = visual_capture::changed_pixel_ratio_in_logical_region(
        // 使用切换前截图。
        &counted_path,
        // 使用切换后截图。
        &hidden_path,
        // Badge 验收根逻辑宽度。
        980.0,
        // Badge 验收根逻辑高度。
        480.0,
        // 只观察 Avatar 右上角圆点，不把按钮焦点变化计入。
        (276.0, 145.0, 20.0, 20.0),
    );
    // 至少一个可辨认圆点面积必须被清理。
    assert!(
        dot_delta >= 0.02,
        "Badge dot region did not change: {:.2}%",
        // 把零到一比例转换为人类可读百分数。
        dot_delta * 100.0,
    );

    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 Badge 验收窗口。
    let _ = demo.stop_and_collect();
}

// 验收 SelectableList 的空初值、指针选择和稳定键盘导航。
fn accept_selectable_list(evidence_root: &Path, images: &mut Vec<PathBuf>) {
    // 启动只拥有 SelectableList 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("selectable-list-visual");
    // 读取首帧派生语义树。
    let initial = client.snapshot(window);
    // 取得稳定自动化目标。
    let initial_list = node_by_automation_id(&initial, "selectable-list");
    // 空受控状态不得伪造活动文本或序号。
    assert!(initial_list["state"]["value_text"].is_null());
    // 空受控状态不得产生溢出序号。
    assert!(initial_list["state"]["value_now"].is_null());
    // 保存没有活动高亮的初始证据。
    capture(&demo, evidence_root, "selectable-01-empty.png", images);

    // 读取组件真实可见边界。
    let (x, y, width, height) = visible_bounds(initial_list);
    // 四行等高，使用第二行中心触发真实指针选择。
    click_at(
        // 使用同一公开 Agent 客户端。
        &mut client,
        // 保持当前窗口身份。
        window,
        // 避开图标并命中第二行文本区域。
        x + width.min(160.0) * 0.5,
        // 以四分之一行高计算第二行中心。
        y + height * 0.375,
    );
    // 读取指针选择后的受控状态。
    let selected = client.snapshot(window);
    // 稳定 id 写回后必须投影对应展示文本。
    assert_eq!(
        node_by_automation_id(&selected, "selectable-list")["state"]["value_text"],
        "任务队列"
    );
    // 第二行必须公开一基序号二。
    assert_eq!(
        node_by_automation_id(&selected, "selectable-list")["state"]["value_now"],
        2.0
    );
    // 保存指针选择后的唯一活动高亮。
    capture(&demo, evidence_root, "selectable-02-pointer.png", images);

    // 使用公开语义动作建立键盘焦点。
    focus_automation(&mut client, window, "selectable-list");
    // End 必须移动到末尾稳定条目。
    press_key(&mut client, window, "end");
    // 读取 End 写回后的状态。
    let ended = client.snapshot(window);
    // 末项展示文本必须与稳定 help id 对应。
    assert_eq!(
        node_by_automation_id(&ended, "selectable-list")["state"]["value_text"],
        "帮助与支持"
    );
    // 保存键盘末项高亮与完整焦点环。
    capture(&demo, evidence_root, "selectable-03-end.png", images);
    // Home 必须回到首项。
    press_key(&mut client, window, "home");
    // Down 必须从首项推进到第二项。
    press_key(&mut client, window, "down");
    // 读取组合键盘路径后的状态。
    let keyboard = client.snapshot(window);
    // 组合导航必须重新得到同一稳定任务条目。
    assert_eq!(
        node_by_automation_id(&keyboard, "selectable-list")["state"]["value_text"],
        "任务队列"
    );

    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 SelectableList 验收窗口。
    let _ = demo.stop_and_collect();
}

// 验收 Collapse 的稳定 key、同名标题和手风琴键盘路径。
fn accept_collapse(evidence_root: &Path, images: &mut Vec<PathBuf>) {
    // 启动只拥有 Collapse 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("collapse-visual");
    // 读取首帧派生语义树。
    let initial = client.snapshot(window);
    // 初始焦点标题必须是首项且保持折叠。
    let initial_collapse = node_by_automation_id(&initial, "collapse");
    // 首项稳定标题必须公开。
    assert_eq!(initial_collapse["name"], "基础设置");
    // 初始受控空 key 集合不得展开任何面板。
    assert_eq!(initial_collapse["state"]["expanded"], false);
    // 保存三项全部折叠的初始证据。
    let initial_path = capture(&demo, evidence_root, "collapse-01-closed.png", images);

    // 聚焦 Collapse 真实组件。
    focus_automation(&mut client, window, "collapse");
    // Enter 必须展开当前基础设置面板。
    press_key(&mut client, window, "enter");
    // 等待有界动画进入可审阅终态。
    thread::sleep(Duration::from_millis(250));
    // 读取首项展开后的语义树。
    let basic_open = client.snapshot(window);
    // 当前稳定 key 必须公开展开事实。
    assert_eq!(
        node_by_automation_id(&basic_open, "collapse")["state"]["expanded"],
        true
    );
    // 读取当前 Collapse 不透明节点身份。
    let basic_node_id = &node_by_automation_id(&basic_open, "collapse")["node_id"];
    // 首项展开后必须恰有一个直接内容 Canvas 可见。
    let basic_content = only_visible_direct_child_bounds(&basic_open, basic_node_id);
    // 保存首项展开与内容布局证据。
    let basic_path = capture(&demo, evidence_root, "collapse-02-basic.png", images);
    // 展开必须改变真实客户区像素而非只更新语义状态。
    assert!(
        visual_capture::changed_pixel_ratio(&initial_path, &basic_path) >= 0.001,
        "Collapse basic expansion did not repaint"
    );

    // End 必须把焦点移到第二个同名高级面板。
    press_key(&mut client, window, "end");
    // Space 必须按第三项稳定 key 切换面板。
    press_key(&mut client, window, "space");
    // 等待手风琴关闭旧项并展开新项的动画终态。
    thread::sleep(Duration::from_millis(250));
    // 读取同名第三项展开后的语义树。
    let secondary_open = client.snapshot(window);
    // 当前标题相同但展开事实属于第三项稳定 key。
    let secondary = node_by_automation_id(&secondary_open, "collapse");
    // 同名标题仍必须公开可读名称。
    assert_eq!(secondary["name"], "高级设置");
    // 第三项必须成为唯一展开面板。
    assert_eq!(secondary["state"]["expanded"], true);
    // 第三项展开后仍必须恰有一个直接内容 Canvas 可见。
    let secondary_content = only_visible_direct_child_bounds(
        // 使用协调后的完整语义快照。
        &secondary_open,
        // 不假设 generation 或节点序号稳定。
        &secondary["node_id"],
    );
    // 同名第三项内容必须移动到第三个标题下方，排除复用首项旧 frame。
    assert!(
        secondary_content.1 > basic_content.1 + 40.0,
        "Collapse secondary content kept stale frame: basic={basic_content:?}, secondary={secondary_content:?}"
    );
    // 保存同名第三项展开与旧内容清除证据。
    let secondary_path = capture(&demo, evidence_root, "collapse-03-secondary.png", images);
    // 同名第三项内容必须形成不同于首项内容的真实像素结果。
    assert!(
        visual_capture::changed_pixel_ratio(&basic_path, &secondary_path) >= 0.001,
        "Collapse stable-key panels produced identical visuals"
    );

    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 Collapse 验收窗口。
    let _ = demo.stop_and_collect();
}

// 验收 Upload 的四态绘制、含逗号文件身份与删除收缩。
fn accept_upload(evidence_root: &Path, images: &mut Vec<PathBuf>) {
    // 启动只拥有 Upload 验收页面的独立进程。
    let (mut demo, mut client, window) = start_visual("upload-visual");
    // 读取首帧派生语义树。
    let initial = client.snapshot(window);
    // 取得上传队列稳定目标。
    let initial_upload = node_by_automation_id(&initial, "upload");
    // 队列语义必须同时公开四种状态与精确文件名。
    assert_eq!(
        initial_upload["state"]["value_text"],
        "产品原型.png: pending; 交互说明,最终版.jpg: uploading 58%; 验收记录.png: done; 失败样例.jpg: error"
    );
    // 保存拖放区、四态图标、进度条和含逗号文件初值。
    let initial_path = capture(&demo, evidence_root, "upload-01-four-states.png", images);

    // 读取 Upload 真实可见边界。
    let (x, y, width, _) = visible_bounds(initial_upload);
    // 点击第二行最右侧删除图标，验证逗号不会被当作身份分隔符。
    click_at(
        // 使用同一公开 Agent 客户端。
        &mut client,
        // 保持当前窗口身份。
        window,
        // 命中组件局部最后二十八像素删除热区。
        x + width - 14.0,
        // 命中一百零四像素头部之后的第二个三十二像素文件行。
        y + 104.0 + 32.0 + 16.0,
    );
    // 读取受控队列原子写回后的语义树。
    let removed = client.snapshot(window);
    // 取得删除后的完整队列文本。
    let removed_text = node_by_automation_id(&removed, "upload")["state"]["value_text"]
        // 队列仍有三项，必须保留字符串状态。
        .as_str()
        // 缺失文本表示状态写回失败。
        .expect("Upload queue value text after removal");
    // 含逗号的第二项必须按稳定身份完整删除。
    assert!(!removed_text.contains("交互说明,最终版.jpg"));
    // 其余三项必须保持原顺序与状态。
    assert_eq!(
        removed_text,
        "产品原型.png: pending; 验收记录.png: done; 失败样例.jpg: error"
    );
    // 保存删除后列表收缩与下方清理证据。
    let removed_path = capture(&demo, evidence_root, "upload-02-removed.png", images);
    // 删除必须在真实客户区产生可辨认像素变化。
    let changed = visual_capture::changed_pixel_ratio(&initial_path, &removed_path);
    // 文件行上移和末行清除应超过全窗千分之一。
    assert!(
        changed >= 0.001,
        "Upload removal did not repaint: {:.2}%",
        // 把零到一比例转换为人类可读百分数。
        changed * 100.0,
    );

    // 关闭协议连接让进程回收不等待客户端。
    drop(client);
    // 回收 Upload 验收窗口。
    let _ = demo.stop_and_collect();
}

// 启动一个强制 D3D11 且 Agent discovery 隔离的专用验收程序。
fn start_visual(binary: &str) -> (DemoProcess, AgentClient, AgentWindow) {
    // 启动调用方指定的唯一独立验收二进制。
    let mut demo = DemoProcess::spawn_visual(binary);
    // 等待同用户本机 descriptor。
    let endpoint = demo.wait_for_endpoint();
    // 连接唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 每个独立验收程序必须只公开一个窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(window, 1);
    // 返回完整进程、协议和窗口身份所有权。
    (demo, client, window)
}

// 通过稳定自动化标识聚焦语义目标。
fn focus_automation(client: &mut AgentClient, window: AgentWindow, automation_id: &str) {
    // 执行公开 focus 动作并取得新 revision。
    let revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 使用公开自动化标识定位组件。
        Some(json!({ "automation_id": automation_id })),
        // 请求正常焦点路径。
        json!({ "kind": "focus" }),
    );
    // 等待焦点视觉完成真实 present。
    client.wait_for_presented(window, revision);
}

// 通过可访问名称聚焦唯一真实节点。
fn focus_named(client: &mut AgentClient, window: AgentWindow, snapshot: &Value, name: &str) {
    // 读取目标不透明节点身份。
    let node_id = node_by_name(snapshot, name)["node_id"].clone();
    // 执行公开 focus 动作并取得新 revision。
    let revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 使用当前 generation 的不透明节点身份。
        Some(json!({ "node_id": node_id })),
        // 请求正常焦点路径。
        json!({ "kind": "focus" }),
    );
    // 等待焦点视觉完成真实 present。
    client.wait_for_presented(window, revision);
}

// 通过真实客户区坐标点击一个可访问命名节点。
fn click_named(client: &mut AgentClient, window: AgentWindow, snapshot: &Value, name: &str) {
    // 读取命名节点真实可见边界。
    let (x, y, width, height) = visible_bounds(node_by_name(snapshot, name));
    // 在可见边界中心执行正常指针路径。
    click_at(client, window, x + width * 0.5, y + height * 0.5);
}

// 通过公开窗口动作执行一次完整指针点击。
fn click_at(client: &mut AgentClient, window: AgentWindow, x: f64, y: f64) {
    // 执行公开 click_at 动作并取得新 revision。
    let revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 窗口级指针动作不需要语义目标。
        None,
        // 使用真实客户区逻辑坐标。
        json!({ "kind": "click_at", "x": x, "y": y }),
    );
    // 等待指针状态与业务状态完成真实 present。
    client.wait_for_presented(window, revision);
}

// 通过公开窗口动作发送一个普通键盘按键。
fn press_key(client: &mut AgentClient, window: AgentWindow, key: &str) {
    // 执行公开 press_key 动作并取得新 revision。
    let revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 窗口级键盘动作不需要语义目标。
        None,
        // 使用协议登记的稳定按键名称。
        json!({ "kind": "press_key", "key": key }),
    );
    // 等待焦点、状态与绘制完成真实 present。
    client.wait_for_presented(window, revision);
}

// 捕获一张真实 HWND 无损证据并登记到审阅板顺序。
fn capture(
    // 借用当前独立验收进程。
    demo: &DemoProcess,
    // 借用本测试稳定证据目录。
    evidence_root: &Path,
    // 接收确定文件名。
    name: &str,
    // 借用最终审阅板路径集合。
    images: &mut Vec<PathBuf>,
) -> PathBuf {
    // 组合当前场景证据路径。
    let path = evidence_root.join(name);
    // 从真实 DWM 客户区捕获无损像素。
    visual_capture::capture_png(demo, &path);
    // 保存独立路径供最终审阅板使用。
    images.push(path.clone());
    // 返回路径供局部像素比较。
    path
}

// 按稳定 automation ID 查找语义节点。
fn node_by_automation_id<'a>(snapshot: &'a Value, automation_id: &str) -> &'a Value {
    // 遍历完整派生语义节点数组。
    snapshot["nodes"]
        // 节点字段必须是数组。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 查找精确稳定自动化标识。
        .iter()
        // 只接受完全相同的字符串值。
        .find(|node| node["automation_id"] == automation_id)
        // 缺失时报告具体标识。
        .unwrap_or_else(|| panic!("missing automation target {automation_id}"))
}

// 按精确可访问名称查找唯一语义节点。
fn node_by_name<'a>(snapshot: &'a Value, name: &str) -> &'a Value {
    // 遍历完整派生语义节点数组。
    snapshot["nodes"]
        // 节点字段必须是数组。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 查找精确可访问名称。
        .iter()
        // 只接受完全相同的字符串值。
        .find(|node| node["name"] == name)
        // 缺失时报告具体名称。
        .unwrap_or_else(|| panic!("missing named target {name}"))
}

// 返回当前唯一拥有焦点的可访问名称。
fn focused_name(snapshot: &Value) -> &str {
    // 遍历完整派生语义节点数组。
    snapshot["nodes"]
        // 节点字段必须是数组。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 查找公开 focused 事实为 true 的节点。
        .iter()
        // 逻辑焦点在同一窗口内必须唯一。
        .find(|node| node["focused"] == true)
        // 焦点动作后缺失焦点属于失败。
        .expect("focused semantic node")
        // 焦点节点必须具有可读名称。
        .get("name")
        // 提取 JSON 值中的字符串。
        .and_then(Value::as_str)
        // 无名焦点不能证明 Tab 归属。
        .expect("focused semantic name")
}

// 读取语义节点裁剪后的真实可见边界。
fn visible_bounds(node: &Value) -> (f64, f64, f64, f64) {
    // 可交互目标必须拥有非空可见边界对象。
    let bounds = &node["visible_bounds"];
    // 读取逻辑 x 坐标。
    let x = bounds["x"].as_f64().expect("visible bounds x");
    // 读取逻辑 y 坐标。
    let y = bounds["y"].as_f64().expect("visible bounds y");
    // 读取正宽度。
    let width = bounds["w"].as_f64().expect("visible bounds width");
    // 读取正高度。
    let height = bounds["h"].as_f64().expect("visible bounds height");
    // 拒绝不可点击的零面积目标。
    assert!(width > 0.0 && height > 0.0, "empty visible bounds: {node}");
    // 返回逻辑客户区矩形。
    (x, y, width, height)
}

// 返回指定父节点下唯一可见直接子节点的真实边界。
fn only_visible_direct_child_bounds(
    // 借用包含派生语义树的快照。
    snapshot: &Value,
    // 借用父节点不透明身份。
    parent_node_id: &Value,
) -> (f64, f64, f64, f64) {
    // 收集全部直接子节点供失败时报告真实 frame 与可见门禁。
    let direct_children = snapshot["nodes"]
        // 节点字段必须是数组。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
        // 遍历全部派生语义节点。
        .iter()
        // 只保留当前 Collapse 的直接子节点。
        .filter(|node| &node["parent"] == parent_node_id)
        // 保留原始 JSON 证据用于断言诊断。
        .collect::<Vec<_>>();
    // 收集父节点下通过裁剪门禁的直接内容节点。
    let visible_children = direct_children
        .iter()
        // 只保留拥有真实可见边界的内容 Canvas。
        .filter(|node| !node["visible_bounds"].is_null())
        // 提取可比较的逻辑客户区边界。
        .map(|node| visible_bounds(node))
        // 保存结果用于唯一性诊断。
        .collect::<Vec<_>>();
    // 每轮手风琴终态必须只展示一个内容 Canvas。
    assert_eq!(
        visible_children.len(),
        1,
        "expected one visible Collapse content child: visible={visible_children:?}, direct={direct_children:?}"
    );
    // 返回唯一可见内容边界。
    visible_children[0]
}

// 返回仓库 target 下不进入提交的组件视觉证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离本组组件交互证据。
        .join("uix-component-visual")
}

// 返回仓库 target 下不进入提交的 Popconfirm 视觉证据目录。
fn popconfirm_evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离 Popconfirm 真窗交互证据。
        .join("uix-popconfirm-visual")
}

// 返回仓库 target 下不进入提交的 backdrop blur 视觉证据目录。
fn backdrop_evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离 backdrop blur 证据。
        .join("uix-backdrop-visual")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &Path) {
    // 存在时只删除本测试固定拥有的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous component visual evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create component visual evidence root");
}
