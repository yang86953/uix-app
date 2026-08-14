// 只在 Windows 启用 Agent 控制能力时编译真窗无障碍语义验收。
#![cfg(all(windows, feature = "agent-control"))]
// 允许断言型验收代码在协议、语义树或证据缺失处定向失败。
#![allow(
    clippy::expect_used,
    reason = "无障碍验收必须在首个缺失角色、敏感值门禁或正常动作路径处给出明确失败"
)]

// 接入唯一拥有主演示子进程与 discovery 生命周期的 fixture。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只依赖公开 uix.agent.v1 的协议 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;

// 导入语义证据目录和文件路径。
use std::path::{Path, PathBuf};

// 导入确定性节点身份集合。
use std::collections::BTreeSet;
// 导入公开协议 JSON 构造与值类型。
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

// 保存必须具有稳定可访问投影的主演示组件目标。
const STABLE_COMPONENT_TARGETS: [&str; 9] = [
    // 密码输入验证敏感值清除。
    "demo-password-input",
    // 受控可选择列表。
    "demo-selectable-list",
    // 折叠面板。
    "demo-collapse",
    // 状态徽标。
    "demo-badge",
    // 消息反馈。
    "demo-message",
    // 通知反馈。
    "demo-notification",
    // 确认气泡。
    "demo-popconfirm",
    // 确认气泡触发器。
    "demo-popconfirm-trigger",
    // 受控上传区。
    "demo-upload",
];

// 在真实 D3D11 主演示中验收十二页派生语义树、焦点、动作与敏感值门禁。
#[test]
#[ignore = "requires an interactive Windows desktop and writes accessibility JSON evidence"]
fn real_uix_lang_demo_captures_accessibility_semantics_and_normal_actions() {
    // 启动显式 Agent 与强制 D3D11 的普通主演示。
    let mut demo = DemoProcess::spawn_main();
    // 等待同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接 fixture 唯一命名管道。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成公开协议握手。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 无障碍矩阵必须只观察一个主演示窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present。
    client.wait_for_presented(window, 1);

    // 创建当前仓库 target 下的稳定证据目录。
    let evidence_root = evidence_root();
    // 精确清理本测试上次生成的证据目录。
    reset_evidence_root(&evidence_root);

    // 先验证公开 focus 动作重新进入正常窗口焦点路径。
    verify_focus_and_invoke(&mut client, window);
    // 保存跨十二页已经观察到的稳定组件标识。
    let mut observed_targets = BTreeSet::new();
    // 保存全部页面语义节点总数供完成摘要。
    let mut semantic_node_total = 0_usize;
    // 按公开侧边栏顺序逐页导航、验证并写入证据。
    for (index, slug, title) in PAGES {
        // 通过公开 invoke 进入目标页面。
        navigate(&mut client, window, index);
        // 读取当前页面完整派生语义树。
        let snapshot = client.snapshot(window);
        // 验证节点身份、父子关系、角色、状态与动作基本不变量。
        validate_semantic_tree(&snapshot, title);
        // 当前快照节点数必须是非零真实树。
        semantic_node_total = semantic_node_total.saturating_add(
            snapshot["nodes"]
                // 节点数组必须由协议提供。
                .as_array()
                // 缺失节点数组属于协议错误。
                .expect("snapshot nodes")
                // 读取当前页节点数。
                .len(),
        );
        // 记录当前页命中的稳定组件目标。
        collect_stable_targets(&snapshot, &mut observed_targets);
        // 输入页额外验证密码状态与值清除门禁。
        if index == 5 {
            // 敏感输入不得通过 Agent 泄漏值文本。
            validate_password_redaction(&snapshot);
        }
        // 构造当前页语义证据文件路径。
        let path = evidence_root.join(format!("{index:02}-{slug}.json"));
        // 写入只包含公开协议字段的格式化 JSON。
        std::fs::write(
            &path,
            // 序列化失败属于证据 Adapter 缺陷。
            serde_json::to_string_pretty(&page_evidence(index, slug, title, &snapshot))
                // 证据必须可由外部工具读取。
                .expect("serialize accessibility page evidence"),
        )
        // 文件写入失败必须阻止验收通过。
        .expect("write accessibility page evidence");
    }

    // 全部已登记稳定组件都必须在其页面语义树中出现。
    let missing = STABLE_COMPONENT_TARGETS
        // 按清单顺序遍历预期目标。
        .iter()
        // 只保留未观察到的目标。
        .filter(|target| !observed_targets.contains(**target))
        // 复制字符串借用供诊断。
        .copied()
        // 收集稳定缺失列表。
        .collect::<Vec<_>>();
    // 缺少任何稳定目标都表示页面语义覆盖不完整。
    assert!(
        missing.is_empty(),
        "missing stable accessibility targets: {missing:?}"
    );
    // 十二页应产生足够大的真实语义矩阵而不是空壳证据。
    assert!(
        semantic_node_total >= 300,
        "accessibility matrix is unexpectedly small: {semantic_node_total} nodes"
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
        "accessibility demo did not select D3D11 recipe: {output}"
    );
    // 验收不得静默退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "accessibility demo unexpectedly used software fallback: {output}"
    );
    // 输出不含 token 的证据位置摘要。
    println!(
        "uix-lang accessibility evidence: {}; pages={}; semantic_nodes={semantic_node_total}; stable_targets={}",
        evidence_root.display(),
        PAGES.len(),
        observed_targets.len()
    );
}

// 返回仓库 target 下不进入提交的无障碍证据目录。
fn evidence_root() -> PathBuf {
    // 从根 crate 清单目录构造确定路径。
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // 进入构建输出目录。
        .join("target")
        // 进入人工与 AI 可读捕获目录。
        .join("debug-captures")
        // 隔离主演示无障碍证据。
        .join("uix-lang-accessibility")
}

// 精确重建本测试拥有的证据目录。
fn reset_evidence_root(root: &Path) {
    // 存在时只删除本测试的精确子目录。
    if root.is_dir() {
        // 目标由 evidence_root 固定在仓库 target/debug-captures 下。
        std::fs::remove_dir_all(root).expect("remove previous accessibility evidence");
    }
    // 创建空证据目录。
    std::fs::create_dir_all(root).expect("create accessibility evidence root");
}

// 验证公开 focus 与 invoke 动作走同一语义事件路径。
fn verify_focus_and_invoke(client: &mut AgentClient, window: AgentWindow) {
    // 确保测试从首页开始。
    navigate(client, window, 0);
    // 读取动作前快照。
    let before = client.snapshot(window);
    // 首页计数按钮必须公开 focus 与 invoke 两项动作。
    assert_node_action(&before, "home-count-increment", "focus");
    // 首页计数按钮必须能够正常调用。
    assert_node_action(&before, "home-count-increment", "invoke");
    // 通过稳定标识请求正常焦点动作。
    let focus_revision = client.perform(
        // 保持当前窗口身份。
        window,
        // 使用跨启动稳定 automation ID。
        Some(json!({ "automation_id": "home-count-increment" })),
        // 使用公开 focus 动作。
        json!({ "kind": "focus" }),
    );
    // 等待焦点状态完成真实 present。
    client.wait_for_presented(window, focus_revision);
    // 读取焦点后的派生语义树。
    let focused = client.snapshot(window);
    // 目标节点必须成为逻辑焦点唯一事实。
    assert_eq!(
        node_by_automation_id(&focused, "home-count-increment")["focused"],
        true
    );
    // 通过同一稳定目标执行正常 invoke 动作。
    let invoke_revision = client.invoke(window, "home-count-increment");
    // 等待响应式计数与绘制完成。
    client.wait_for_presented(window, invoke_revision);
    // 动作后的语义树必须反映业务 State 更新。
    let invoked = client.snapshot(window);
    // 至少一个文本节点必须公开递增后的真实计数。
    assert!(
        invoked["nodes"]
            // 节点数组必须存在。
            .as_array()
            // 遍历全部语义节点。
            .into_iter()
            .flatten()
            // 匹配更新后的可访问文本。
            .any(|node| node["name"] == "计数: 1"),
        "invoke action did not update normal reactive state"
    );
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

// 验证当前页面派生语义树的结构与字段不变量。
fn validate_semantic_tree(snapshot: &Value, expected_title: &str) {
    // 读取扁平语义节点数组。
    let nodes = snapshot["nodes"].as_array().expect("snapshot nodes");
    // 当前页面不得产生空语义树。
    assert!(!nodes.is_empty(), "semantic tree must not be empty");
    // 保存已经出现的 opaque node ID。
    let mut ids = BTreeSet::new();
    // 逐节点验证公开字段形状。
    for node in nodes {
        // node_id 必须是非空协议字符串。
        let node_id = node["node_id"]
            // 读取 opaque 身份文本。
            .as_str()
            // 缺失身份属于协议错误。
            .expect("semantic node id");
        // 同一 generation 内身份必须唯一。
        assert!(ids.insert(node_id), "duplicate semantic node id {node_id}");
        // 已公开节点不得保留被过滤的 none 角色。
        assert_ne!(node["role"], "none", "none role leaked into semantic tree");
        // 角色必须是非空字符串。
        assert!(
            node["role"].as_str().is_some_and(|role| !role.is_empty()),
            "semantic node missing role: {node}"
        );
        // 状态必须使用公开对象形状。
        assert!(node["state"].is_object(), "semantic node missing state");
        // 动作必须使用公开数组形状。
        assert!(node["actions"].is_array(), "semantic node missing actions");
    }
    // 每个非根节点的 parent 都必须指向同一快照内身份。
    for node in nodes {
        // 根节点允许空 parent。
        if let Some(parent) = node["parent"].as_str() {
            // 非根 parent 必须已经出现在同一节点集合。
            assert!(
                ids.contains(parent),
                "semantic node parent {parent} is missing"
            );
        }
    }
    // 当前页标题必须以可见 heading 或 text 语义存在。
    assert!(
        nodes.iter().any(|node| {
            // 匹配精确名称。
            node["name"] == expected_title
                // 标题必须具有真实可见边界。
                && !node["visible_bounds"].is_null()
                // 标题角色必须保持可朗读文本语义。
                && matches!(node["role"].as_str(), Some("heading" | "text"))
        }),
        "missing visible accessible page title {expected_title}"
    );
}

// 记录当前页面命中的稳定组件自动化标识。
fn collect_stable_targets(snapshot: &Value, observed: &mut BTreeSet<String>) {
    // 遍历全部当前语义节点。
    for node in snapshot["nodes"]
        // 节点数组必须存在。
        .as_array()
        // 缺失数组属于协议错误。
        .expect("snapshot nodes")
    {
        // 缺少自动化标识的普通节点无需登记。
        let Some(automation_id) = node["automation_id"].as_str() else {
            // 继续检查下一节点。
            continue;
        };
        // 只记录当前验收清单中的稳定组件。
        if STABLE_COMPONENT_TARGETS.contains(&automation_id) {
            // 保存拥有所有权的标识供跨页面汇总。
            observed.insert(automation_id.to_string());
        }
    }
}

// 验证密码输入状态公开但值文本始终清除。
fn validate_password_redaction(snapshot: &Value) {
    // 按稳定自动化标识取得密码输入节点。
    let password = node_by_automation_id(snapshot, "demo-password-input");
    // 密码输入必须保持文本框角色。
    assert_eq!(password["role"], "text_box");
    // 密码状态必须明确为 true。
    assert_eq!(password["state"]["password"], true);
    // 敏感值文本必须为 null，不能通过 Agent 响应泄漏。
    assert!(
        password["state"]["value_text"].is_null(),
        "password value leaked through semantic snapshot: {password}"
    );
}

// 构造可独立审计的单页语义证据对象。
fn page_evidence(index: usize, slug: &str, title: &str, snapshot: &Value) -> Value {
    // 只投影公开协议已经提供的派生语义字段。
    json!({
        // 固定证据 schema 供后续工具识别。
        "schema": "uix.accessibility.page.v1",
        // 保存公开页面索引。
        "page_index": index,
        // 保存稳定页面文件名。
        "page_slug": slug,
        // 保存可访问标题。
        "page_title": title,
        // 保存当前窗口 generation。
        "generation": snapshot["generation"],
        // 保存捕获时语义 revision。
        "revision": snapshot["revision"],
        // 保存捕获时已呈现 revision。
        "presented_revision": snapshot["presented_revision"],
        // 保存完整公开节点数组以供外部审计。
        "nodes": snapshot["nodes"],
    })
}

// 验证稳定目标公开指定语义动作。
fn assert_node_action(snapshot: &Value, automation_id: &str, expected: &str) {
    // 取得稳定目标节点。
    let node = node_by_automation_id(snapshot, automation_id);
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
