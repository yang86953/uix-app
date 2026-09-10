//! 《外部扩展包 × Agent 独立后台操作面》集成场景的公开消费者。
//!
//! 以文本处理工作台为场景：宿主持有版本化文档领域数据并显式授权
//! 查询 / 提交端口；Scheme 扩展（仓库交付的 `extensions/text-bench`
//! 外部包）实现真实文本统计算法与动态面板；AI 只经 `AgentWorkspace`
//! 后台视口操作，与用户前台（`TestApp` 输入侧）互不干预局部交互状态。
//! 本文件只使用已文档化的公开 API；无效包输入在临时目录构造。
#![cfg(all(
    feature = "agent-control",
    feature = "test-harness",
    feature = "extensions"
))]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use uix_app::app::agent_client::AgentBridgeClient;
use uix_app::app::agent_workspace::AgentWorkspace;
use uix_app::app::extensions::{
    ExtensionHost, ExtensionPackage, ExtensionPort, ExtensionUiHandle, ExtensionValue, UiNode,
    UiProjector, UiUpdate,
};
use uix_app::prelude::*;
use uix_app::ui::test_harness::{AutomationNode, TestApp};

// 文档规定每进程一个后台操作面；消费者串行持有该公开租约。
static WORKSPACE: Mutex<()> = Mutex::new(());

// ---------- 交付包与无效输入 ----------

/// 读取仓库交付的外部扩展包（示例与测试共用同一交付物）。
fn delivered_package(major: u32) -> ExtensionPackage {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("extensions")
        .join("text-bench")
        .join(format!("v{major}"));
    ExtensionPackage::read_from_directory(&directory)
        .unwrap_or_else(|error| panic!("交付包 v{major} 读取失败（{directory:?}）：{error}"))
}

/// 独立的临时包目录（无效输入构造用；调用方负责清理）。
fn temp_package_dir(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("uix-text-bench-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("临时包目录创建");
    directory
}

// ---------- 宿主领域数据与共享端口 ----------

#[derive(Clone)]
struct Document {
    content: String,
    version: u64,
}

type SharedDocuments = Arc<Mutex<std::collections::BTreeMap<String, Document>>>;

fn initial_documents() -> SharedDocuments {
    Arc::new(Mutex::new(
        [(
            "intro".to_string(),
            Document {
                content: "  UIX 文本处理工作台   示例文档\n第二行内容  ".to_string(),
                version: 1,
            },
        )]
        .into_iter()
        .collect(),
    ))
}

fn query_port(documents: SharedDocuments) -> ExtensionPort {
    Arc::new(move |arguments: &[ExtensionValue]| match arguments {
        [ExtensionValue::Text(key)] => {
            let store = documents.lock().expect("文档存储锁");
            store
                .get(key)
                .map(|document| {
                    ExtensionValue::List(vec![
                        ExtensionValue::Text(document.content.clone()),
                        ExtensionValue::Int(document.version as i64),
                    ])
                })
                .ok_or_else(|| format!("未知文档 {key}"))
        }
        _ => Err("需要 (文档键) 一个字符串参数".to_string()),
    })
}

/// 受控提交：版本一致才写入；冲突返回 `(conflict 当前版本)`，不覆盖。
fn commit_port(documents: SharedDocuments) -> ExtensionPort {
    Arc::new(move |arguments: &[ExtensionValue]| match arguments {
        [
            ExtensionValue::Text(key),
            ExtensionValue::Int(expected),
            ExtensionValue::Text(text),
        ] => {
            let mut store = documents.lock().expect("文档存储锁");
            let Some(document) = store.get_mut(key) else {
                return Err(format!("未知文档 {key}"));
            };
            if document.version as i64 != *expected {
                return Ok(ExtensionValue::List(vec![
                    ExtensionValue::Symbol("conflict".to_string()),
                    ExtensionValue::Int(document.version as i64),
                ]));
            }
            document.version += 1;
            document.content = text.clone();
            Ok(ExtensionValue::List(vec![
                ExtensionValue::Symbol("ok".to_string()),
                ExtensionValue::Int(document.version as i64),
            ]))
        }
        _ => Err("需要 (文档键 预期版本 新文本) 三个参数".to_string()),
    })
}

// ---------- 后台操作面装配 ----------

/// 一次后台装配：workspace（根 + 投影器 + 修订 State）与扩展 worker。
struct BackgroundWorkbench {
    workspace: uix_app::app::agent_workspace::AgentWorkspaceHandle,
    client: AgentBridgeClient,
    handle: ExtensionUiHandle,
    documents: SharedDocuments,
    generation: u64,
    /// 停用终态后清空挂载子树并唤醒后台 owner（应用负责子树退出）。
    clear_mount: Arc<dyn Fn() + Send + Sync>,
}

fn spawn_background_workbench() -> BackgroundWorkbench {
    spawn_background_workbench_with(delivered_package(1))
}

/// 以指定交付包装配后台（装载代 v1 / 候选 v2 复用同一装配）。
fn spawn_background_workbench_with(package: ExtensionPackage) -> BackgroundWorkbench {
    let documents = initial_documents();
    let revision = State::new(0u64);
    let current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
    let projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));

    let workspace = {
        let root_revision = revision.clone();
        let root_current = Arc::clone(&current);
        let root_projector = Arc::clone(&projector);
        AgentWorkspace::new(760, 600, move || {
            root_revision.get();
            let node = root_current.lock().expect("后台声明锁").clone();
            let mut projector = root_projector.lock().expect("后台投影器锁");
            let panel = match (node, projector.as_mut()) {
                (Some(node), Some(projector)) => projector.project("panel", &node),
                _ => column(Vec::<ViewNode>::new()),
            };
            column_fit((
                label("AI 独立后台操作面 · 文本处理")
                    .font_size(15.0)
                    .automation_id("workspace-root"),
                panel,
            ))
        })
        .spawn()
        .expect("后台操作面启动")
    };
    // 扩展线程的声明出口：sink 直接应用 owned 声明并唤醒后台 owner。
    let host = ExtensionHost::new()
        .with_mount("panel")
        .with_port("documents-query", query_port(Arc::clone(&documents)))
        .with_port("documents-commit", commit_port(Arc::clone(&documents)))
        .with_ui_sink(Arc::new({
            let sink_current = Arc::clone(&current);
            let sink_revision = revision.clone();
            let sink_projector = Arc::clone(&projector);
            let poster = workspace.poster();
            move |update| match update {
                UiUpdate::Applied {
                    mut node,
                    generation,
                    ..
                } => {
                    if let Some(projector) = sink_projector.lock().expect("后台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
                        // reset 是本次 Applied 的一次性指令：执行后消耗标记，
                        // 后续重投影不得再次覆盖本地编辑。
                        projector.consume_declaration_resets(&mut node);
                    }
                    *sink_current.lock().expect("后台声明锁") = Some(node);
                    sink_revision.update(|value| *value += 1);
                    poster.wake();
                }
                UiUpdate::Rejected { mount, reason, .. } => {
                    panic!("后台声明不应被拒绝（{mount}）：{reason}")
                }
            }
        }));
    let handle = host.spawn_worker();
    let prepared = handle.prepare(&package).expect("准备");
    let receipt = handle.activate(prepared).expect("激活");
    *projector.lock().expect("后台投影器锁") = Some(UiProjector::new(
        &receipt.extension_id,
        receipt.generation,
        handle.event_sender(),
    ));
    // 装配补帧：初始声明的重帧可能早于投影器安装（渲染出空面板），
    // 安装后主动推进修订并唤醒，保证一次带投影的完整重建。
    revision.update(|value| *value += 1);
    workspace.poster().wake();
    let clear_mount: Arc<dyn Fn() + Send + Sync> = Arc::new({
        let current = Arc::clone(&current);
        let revision = revision.clone();
        let poster = workspace.poster();
        move || {
            *current.lock().expect("后台声明锁") = None;
            revision.update(|value| *value += 1);
            poster.wake();
        }
    });
    let client = workspace.client().expect("后台客户端");
    BackgroundWorkbench {
        workspace,
        client,
        handle,
        documents,
        generation: receipt.generation,
        clear_mount,
    }
}

// ---------- 快照工具 ----------

fn node_field(snapshot: &Value, automation_id: &str, field: &str) -> Option<String> {
    snapshot["nodes"].as_array()?.iter().find_map(|node| {
        (node["automation_id"].as_str() == Some(automation_id))
            .then(|| {
                node[field]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| node["state"]["value_text"].as_str().map(str::to_string))
            })
            .flatten()
    })
}

/// 文本节点内容（label 的 name）。
fn node_text(snapshot: &Value, automation_id: &str) -> String {
    node_field(snapshot, automation_id, "name")
        .unwrap_or_else(|| panic!("快照缺少 {automation_id}：{snapshot}"))
}

/// 输入框当前值（State 的 value_text）。
fn node_value(snapshot: &Value, automation_id: &str) -> String {
    node_field(snapshot, automation_id, "value")
        .unwrap_or_else(|| panic!("快照缺少输入值 {automation_id}：{snapshot}"))
}

const DRAFT: &str = "text-bench/panel/root/draft";
const PROCESS: &str = "text-bench/panel/root/controls/process";
const LOAD: &str = "text-bench/panel/root/controls/load";
const COMMIT: &str = "text-bench/panel/root/controls/commit";
const STATS: &str = "text-bench/panel/root/stats";
const TITLE: &str = "text-bench/panel/root/title";
const STATUS: &str = "text-bench/panel/root/status";
const CJK: &str = "text-bench/panel/root/cjk";
const WORKSPACE_ROOT: &str = "workspace-root";

const TIMEOUT: Duration = Duration::from_secs(8);

// ---------- 场景 ----------

/// v1 词数口径（文档化 `stats` 命令）：空串、纯空白、无尾空格单词、
/// 多词与首尾空白等价输入。修复进入单词与字符串结束的重复计数。
#[test]
fn word_count_v1_stats_covers_boundary_inputs() {
    let documents = initial_documents();
    let mut host = ExtensionHost::new()
        .with_mount("panel")
        .with_port("documents-query", query_port(Arc::clone(&documents)))
        .with_port("documents-commit", commit_port(Arc::clone(&documents)));
    let prepared = host.prepare(&delivered_package(1)).expect("准备");
    let receipt = host.activate(prepared).expect("激活");
    assert_eq!(receipt.extension_id, "text-bench");

    let mut stats = |text: &str| {
        host.call_command(
            "text-bench",
            "stats",
            &[ExtensionValue::Text(text.to_string())],
        )
        .expect("stats 命令")
    };
    assert_eq!(
        stats(""),
        ExtensionValue::Text("字符 0 · 非空白 0 · 行 1 · 词 0".to_string()),
        "空串无单词"
    );
    assert_eq!(
        stats("   "),
        ExtensionValue::Text("字符 3 · 非空白 0 · 行 1 · 词 0".to_string()),
        "纯空白无单词"
    );
    assert_eq!(
        stats("hello"),
        ExtensionValue::Text("字符 5 · 非空白 5 · 行 1 · 词 1".to_string()),
        "无尾空格单词只计一次"
    );
    assert_eq!(
        stats("hello world foo"),
        ExtensionValue::Text("字符 15 · 非空白 13 · 行 1 · 词 3".to_string()),
        "多词逐段计数"
    );
    // 首尾空白与无空白版本等价（空白只分段，不产生单词）。
    assert_eq!(
        stats("  hello world  "),
        ExtensionValue::Text("字符 15 · 非空白 10 · 行 1 · 词 2".to_string()),
    );
    assert_eq!(
        stats("hello world"),
        ExtensionValue::Text("字符 11 · 非空白 10 · 行 1 · 词 2".to_string()),
    );

    host.deactivate("text-bench").expect("停用");
}

/// AI 在后台完成读取 → 等待读取已应用 → 编辑为不同内容 → 处理 → 提交的
/// 完整编辑闭环；最终提交的是编辑后的内容，而不是读取时的旧内容。
#[test]
fn background_agent_edits_after_load_and_commits_edited_content() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    let windows = bench
        .client
        .request(serde_json::json!({"type": "list_windows"}))
        .expect("枚举");
    assert_eq!(windows["windows"][0]["visible"], false, "后台视口不可见");
    assert_eq!(
        bench.client.capabilities()["background_control"]["isolated_workspace"],
        true
    );

    // 初始声明经 worker → sink → 后台 owner 真实到达语义树。
    let initial = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");
    assert_eq!(node_text(&initial, TITLE), "文本处理工作台 v1");

    // 未读取文档就提交（携带初始版本 0）：明确冲突，共享数据不变。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("提交动作");
    let premature = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("版本冲突"))
        })
        .expect("未读取提交的冲突未回显");
    assert!(
        node_text(&premature, STATUS).contains("已是 v1"),
        "{premature:?}"
    );

    // 读取宿主文档（显式授权的只读端口），等待读取结果已应用。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("读取文档");
    let loaded = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value")
                .is_some_and(|value| value.starts_with("  UIX") && value.contains("第二行内容"))
        })
        .expect("读取结果未应用到草稿");
    assert!(
        node_text(&loaded, STATUS).contains("已读取 v1"),
        "{loaded:?}"
    );

    // 编辑为不同内容（AI 私有交互状态；事件经 worker 执行脚本）。
    let edited = "  AI 编辑后的   新内容  ";
    bench
        .client
        .perform_set_value(view.window_id, view.generation, DRAFT, edited)
        .expect("编辑草稿");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some(edited)
        })
        .expect("编辑未覆盖读取内容");

    // 处理并确认编辑后内容的 v1 统计（字符 17 · 非空白 9 · 词 3）。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("执行处理");
    let processed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .expect("处理结果未回显");
    assert_eq!(
        node_text(&processed, STATS),
        "统计: 字符 17 · 非空白 9 · 行 1 · 词 3"
    );
    // 等待完成呈现：后台语义已应用，离屏帧已绘制（呈现回执）。
    let revision = processed["revision"].as_u64().expect("修订号");
    bench
        .client
        .wait_until_presented(view.window_id, view.generation, revision, TIMEOUT)
        .expect("离屏帧呈现");

    // 业务提交：共享文档更新为编辑后内容的规范化文本（版本递增）。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("业务提交");
    let committed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已提交"))
        })
        .expect("提交结果未回显");
    assert!(
        node_text(&committed, STATUS).contains("v2"),
        "{committed:?}"
    );
    {
        let store = bench.documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 2, "只有授权提交才递增版本");
        assert_eq!(
            document.content, "AI 编辑后的 新内容",
            "提交的是编辑后内容，不是读取时的旧内容"
        );
    }

    // 后台语义状态与真实离屏 PNG 对应：截图包含已呈现的处理结果帧。
    let screenshot = bench
        .client
        .request(serde_json::json!({
            "type": "screenshot", "window_id": view.window_id
        }))
        .expect("截图");
    assert_eq!(screenshot["ok"], true);
    assert!(
        screenshot["data_base64"]
            .as_str()
            .expect("像素数据")
            .starts_with("iVBORw0KGgo"),
        "必须是真实 PNG"
    );

    // 无窗口逻辑部分：命令调用与面板共用同一实例与算法。
    let normalized = bench
        .handle
        .call_command(
            "text-bench",
            "normalize",
            &[ExtensionValue::Text("  a   b  ".to_string())],
        )
        .expect("命令调用");
    assert_eq!(normalized, ExtensionValue::Text("a b".to_string()));

    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
}

/// 陈旧版本的提交被明确拒绝：共享数据不被覆盖，冲突版本可见。
#[test]
fn stale_commit_conflicts_without_silent_overwrite() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");

    // AI 先输入并处理自己的草稿（提交内容 = 规范化后的草稿）。
    bench
        .client
        .perform_set_value(
            view.window_id,
            view.generation,
            DRAFT,
            "  AI 的   迟到提交 ",
        )
        .expect("输入");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some("  AI 的   迟到提交 ")
        })
        .expect("草稿未更新");

    // AI 读取文档（记录版本 1）——读取会以文档内容重建草稿。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("读取文档");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已读取 v1"))
        })
        .expect("读取结果未回显");

    // 用户在前台先提交（经同一授权端口），版本推进到 2。
    let user_commit = commit_port(Arc::clone(&bench.documents));
    let outcome = user_commit(&[
        ExtensionValue::Text("intro".to_string()),
        ExtensionValue::Int(1),
        ExtensionValue::Text("用户先提交的新内容".to_string()),
    ])
    .expect("前台提交");
    assert_eq!(
        outcome,
        ExtensionValue::List(vec![
            ExtensionValue::Symbol("ok".to_string()),
            ExtensionValue::Int(2),
        ])
    );

    // AI 携带陈旧版本提交：冲突拒绝，用户修改保留。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("提交动作");
    let conflicted = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("版本冲突"))
        })
        .expect("冲突未回显");
    assert!(
        node_text(&conflicted, STATUS).contains("v2"),
        "{conflicted:?}"
    );
    {
        let store = bench.documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 2);
        assert_eq!(
            document.content, "用户先提交的新内容",
            "用户的新修改不被覆盖"
        );
    }

    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
}

/// 后台执行扩展业务时，前台（用户输入侧）局部交互状态保持独立；
/// 合法提交产生的共享数据更新按业务契约对前台可见。
#[test]
fn foreground_local_state_independent_from_background_activity() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let documents = initial_documents();
    let (fg_updates_tx, fg_updates_rx) = std::sync::mpsc::channel::<UiUpdate>();

    // 前台用户 worker（挂载位 panel）：声明经通道由测试线程应用。
    let fg_host = ExtensionHost::new()
        .with_mount("panel")
        .with_port("documents-query", query_port(Arc::clone(&documents)))
        .with_port("documents-commit", commit_port(Arc::clone(&documents)))
        .with_ui_sink(Arc::new(move |update| {
            let _ = fg_updates_tx.send(update);
        }));
    let fg_handle = fg_host.spawn_worker();
    let fg_current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
    let fg_revision = State::new(0u64);
    let fg_projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));
    let fg_receipt = fg_handle
        .prepare(&delivered_package(1))
        .and_then(|prepared| fg_handle.activate(prepared))
        .expect("前台装载");
    *fg_projector.lock().expect("前台投影器锁") = Some(UiProjector::new(
        &fg_receipt.extension_id,
        fg_receipt.generation,
        fg_handle.event_sender(),
    ));
    let mut apply_foreground = {
        let current = Arc::clone(&fg_current);
        let revision = fg_revision.clone();
        let projector = Arc::clone(&fg_projector);
        move || {
            let update = fg_updates_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("前台声明超时");
            match update {
                UiUpdate::Applied {
                    mut node,
                    generation,
                    ..
                } => {
                    if let Some(projector) = projector.lock().expect("前台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
                        projector.consume_declaration_resets(&mut node);
                    }
                    *current.lock().expect("前台声明锁") = Some(node);
                    revision.update(|value| *value += 1);
                }
                UiUpdate::Rejected { mount, reason, .. } => {
                    panic!("前台声明不应被拒绝（{mount}）：{reason}")
                }
            }
        }
    };
    apply_foreground();
    let build_current = Arc::clone(&fg_current);
    let build_revision = fg_revision.clone();
    let build_projector = Arc::clone(&fg_projector);
    let mut user = TestApp::new((560.0, 640.0), move || {
        build_revision.get();
        let node = build_current.lock().expect("前台声明锁").clone();
        match (node, build_projector.lock().expect("前台投影器锁").as_mut()) {
            (Some(node), Some(projector)) => projector.project("panel", &node),
            _ => column(Vec::<ViewNode>::new()),
        }
    });
    user.settle().expect("前台初始帧");
    let user_before = user.snapshot().nodes;
    let fg_draft = "text-bench/panel/root/draft";

    // 用户在前台输入自己的草稿。
    user.set_value(fg_draft, "用户前台草稿 内容")
        .expect("前台输入");
    apply_foreground();
    user.settle().expect("前台帧");
    assert_eq!(user.text(fg_draft).expect("前台草稿"), "用户前台草稿 内容");

    // 后台操作面与 AI worker 并行执行业务。
    let mut bench = {
        // 复用同一领域端口构造后台（与前台共享 documents）。
        let revision = State::new(0u64);
        let current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
        let projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));
        let workspace = {
            let root_revision = revision.clone();
            let root_current = Arc::clone(&current);
            let root_projector = Arc::clone(&projector);
            AgentWorkspace::new(760, 600, move || {
                root_revision.get();
                let node = root_current.lock().expect("后台声明锁").clone();
                let mut projector = root_projector.lock().expect("后台投影器锁");
                let panel = match (node, projector.as_mut()) {
                    (Some(node), Some(projector)) => projector.project("panel", &node),
                    _ => column(Vec::<ViewNode>::new()),
                };
                column_fit((label("AI 后台").font_size(15.0), panel))
            })
            .spawn()
            .expect("后台操作面启动")
        };
        let host = ExtensionHost::new()
            .with_mount("panel")
            .with_port("documents-query", query_port(Arc::clone(&documents)))
            .with_port("documents-commit", commit_port(Arc::clone(&documents)))
            .with_ui_sink(Arc::new({
                let sink_current = Arc::clone(&current);
                let sink_revision = revision.clone();
                let sink_projector = Arc::clone(&projector);
                let poster = workspace.poster();
                move |update| match update {
                    UiUpdate::Applied {
                        mut node,
                        generation,
                        ..
                    } => {
                        if let Some(projector) =
                            sink_projector.lock().expect("后台投影器锁").as_mut()
                        {
                            projector.update_generation(generation);
                            projector.consume_declaration_resets(&mut node);
                        }
                        *sink_current.lock().expect("后台声明锁") = Some(node);
                        sink_revision.update(|value| *value += 1);
                        poster.wake();
                    }
                    UiUpdate::Rejected { mount, reason, .. } => {
                        panic!("后台声明不应被拒绝（{mount}）：{reason}")
                    }
                }
            }));
        let handle = host.spawn_worker();
        let receipt = handle
            .prepare(&delivered_package(1))
            .and_then(|prepared| handle.activate(prepared))
            .expect("后台装载");
        *projector.lock().expect("后台投影器锁") = Some(UiProjector::new(
            &receipt.extension_id,
            receipt.generation,
            handle.event_sender(),
        ));
        revision.update(|value| *value += 1);
        workspace.poster().wake();
        BackgroundWorkbench {
            client: workspace.client().expect("客户端"),
            workspace,
            handle,
            documents: Arc::clone(&documents),
            generation: receipt.generation,
            clear_mount: Arc::new(|| {}),
        }
    };
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("后台初始声明未到达");

    // AI 在后台输入、读取版本后提交（业务闭环）。
    bench
        .client
        .perform_set_value(
            view.window_id,
            view.generation,
            DRAFT,
            "  AI 后台提交   内容 ",
        )
        .expect("后台输入");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some("  AI 后台提交   内容 ")
        })
        .expect("后台草稿未更新");
    // 读取会把后台草稿重置为文档内容；随后按读取版本提交。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("后台读取");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已读取 v1"))
        })
        .expect("后台读取未回显");
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("后台提交");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已提交"))
        })
        .expect("后台提交未回显");
    {
        let store = documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 2, "授权提交更新共享数据");
        assert_eq!(
            document.content, "UIX 文本处理工作台 示例文档 第二行内容",
            "提交的是读取后规范化文本"
        );
    }

    // 前台用户树未被后台触碰：焦点、草稿与节点结构保持（值可能来自
    // 用户自身输入，节点身份集合必须一致）。
    assert_eq!(
        user.text(fg_draft).expect("前台草稿保持"),
        "用户前台草稿 内容"
    );
    let automation_ids = |nodes: &Vec<AutomationNode>| {
        let mut ids: Vec<String> = nodes
            .iter()
            .filter_map(|node| node.automation_id.clone())
            .collect();
        ids.sort();
        ids
    };
    assert_eq!(
        automation_ids(&user_before),
        automation_ids(&user.snapshot().nodes),
        "后台活动不改变前台节点结构"
    );

    // 共享数据更新按业务契约对前台可见：前台重新读取即得新版本。
    user.invoke("text-bench/panel/root/controls/load")
        .expect("前台读取");
    apply_foreground();
    user.settle().expect("前台帧");
    let fg_status = user.text("text-bench/panel/root/status").expect("状态");
    assert!(
        fg_status.contains("已读取 v2"),
        "合法提交对前台可见：{fg_status}"
    );
    assert_eq!(
        user.text(fg_draft).expect("前台草稿被文档读取显式重置"),
        "UIX 文本处理工作台 示例文档 第二行内容"
    );

    bench.handle.shutdown().expect("后台 worker 关停");
    let _ = fg_handle.shutdown();
    bench.workspace.close().expect("操作面关闭");
}

/// 热替换同时升级后台算法与界面，兼容状态（含读取版本）迁移保留；
/// 替换后继续编辑并完成业务提交；无效候选与迁移失败保留旧代。
#[test]
fn hot_replace_keeps_editing_and_committing_after_upgrade() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");

    // v1 处理结果作为算法基线（单行输入把换行规范化为空格）。
    bench
        .client
        .perform_set_value(
            view.window_id,
            view.generation,
            DRAFT,
            "  hello   世界  abc  ",
        )
        .expect("输入");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some("  hello   世界  abc  ")
        })
        .expect("草稿未更新");
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("处理");
    let v1_stats = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .expect("v1 结果未回显");
    assert_eq!(
        node_text(&v1_stats, STATS),
        "统计: 字符 19 · 非空白 10 · 行 1 · 词 3"
    );

    // 注入无效候选（外部包文件：状态 schema 不一致）：替换被拒，旧代继续服务。
    let bad_dir = temp_package_dir("schema-mismatch");
    std::fs::write(
        bad_dir.join("manifest.scm"),
        "(uix-extension (schema-version 1) (id \"text-bench\") (version \"0.3.0\") \
         (language r7rs-small) (entry \"main.scm\") \
         (capabilities documents-query documents-commit mount-panel) \
         (state-schema-version 2))",
    )
    .expect("写无效清单");
    std::fs::write(bad_dir.join("main.scm"), "(define x 1)").expect("写无效源码");
    let bad = ExtensionPackage::read_from_directory(&bad_dir).expect("无效包可读取");
    let bad_prepared = bench.handle.prepare(&bad).expect("无效包可准备");
    let rejected = bench
        .handle
        .replace(bad_prepared, bench.generation)
        .expect_err("schema 不一致应拒绝");
    assert!(matches!(
        rejected,
        uix_app::app::extensions::ExtensionError::Incompatible(_)
    ));
    let _ = std::fs::remove_dir_all(&bad_dir);
    let still_v1 = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some_and(|text| text == "文本处理工作台 v1")
        })
        .expect("旧代界面保留");
    assert_eq!(
        node_text(&still_v1, STATS),
        "统计: 字符 19 · 非空白 10 · 行 1 · 词 3"
    );

    // 读取文档（记录版本 1）：为替换后的提交准备兼容状态。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("读取文档");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已读取 v1"))
        })
        .expect("读取未回显");

    // 正常替换 v2（外部包文件升级：算法与界面共同升级）。
    let candidate = bench
        .handle
        .prepare(&delivered_package(2))
        .expect("候选准备");
    let replacement = bench
        .handle
        .replace(candidate, bench.generation)
        .expect("替换");
    assert!(replacement.migrated, "同 schema 状态迁移应执行");
    bench.generation = replacement.generation;
    let upgraded = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some_and(|text| text == "文本处理工作台 v2")
        })
        .expect("新代界面未到达");
    // 迁移保留兼容状态：读取版本与草稿按导入路径重建。
    assert!(
        node_text(&upgraded, STATUS).contains("已读取 v1"),
        "读取版本跨代保留：{upgraded:?}"
    );
    // 新界面节点实际生效：CJK 行按迁移草稿（读取的文档内容，16 个
    // CJK 字）计算，证明新代算法与新界面同时生效。
    assert_eq!(node_text(&upgraded, CJK), "CJK 字符: 16");

    // 热替换后继续编辑：新内容 → 新代算法处理 → 按迁移的版本提交。
    let edited = "  替换后编辑   beta  ";
    bench
        .client
        .perform_set_value(view.window_id, view.generation, DRAFT, edited)
        .expect("替换后编辑");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some(edited)
        })
        .expect("替换后编辑未生效");
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("新代处理");
    let reprocessed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text.contains("词 6"))
        })
        .expect("新算法未生效");
    // 字符 16 · 非空白 9 · 行 1 · 词（CJK 5 + 拉丁 1 = 6）· CJK 5。
    assert_eq!(
        node_text(&reprocessed, STATS),
        "统计: 字符 16 · 非空白 9 · 行 1 · 词 6 · CJK 5"
    );
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("替换后提交");
    let committed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已提交"))
        })
        .expect("替换后提交未回显");
    assert!(
        node_text(&committed, STATUS).contains("v2"),
        "迁移的读取版本支撑提交：{committed:?}"
    );
    {
        let store = bench.documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 2);
        assert_eq!(
            document.content, "替换后编辑 beta",
            "替换后提交的是编辑后内容"
        );
    }
    // 命令入口同步升级（无窗口逻辑与面板同代）。
    let version = bench
        .handle
        .call_command("text-bench", "engine-version", &[])
        .expect("版本命令");
    assert_eq!(version, ExtensionValue::Text("0.2.0".to_string()));

    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
}

/// 撤权、停用与关闭返回真实终态：能力退出、扩展节点退出语义树、
/// 宿主与后台操作面保持存活；不把关闭操作面当作挂载清理证明。
#[test]
fn revocation_deactivation_clears_mount_and_keeps_host_alive() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    let ready = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");
    assert_eq!(node_text(&ready, TITLE), "文本处理工作台 v1");

    // 撤权：新调用立即被拒（状态回读可见 revoked）；已呈现界面保留。
    bench.handle.revoke("text-bench").expect("撤权");
    let statuses = bench.handle.list().expect("状态回读");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].extension_id, "text-bench");
    assert!(statuses[0].revoked, "撤权状态可观察");
    let denied = bench
        .handle
        .call_command("text-bench", "engine-version", &[])
        .expect_err("撤权后命令应被拒");
    assert!(matches!(
        denied,
        uix_app::app::extensions::ExtensionError::CapabilityDenied(_)
    ));
    // 面板事件被丢弃：处理后统计冻结（有界负等待，非固定延时替代流程）。
    let _ = bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS);
    let frozen = bench
        .client
        .wait_for_snapshot(view.window_id, Duration::from_millis(600), |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .is_err();
    assert!(frozen, "撤权后统计不应变化");

    // 停用：真实终态后由宿主清空挂载位（示例合同：应用负责子树退出）。
    let teardown = bench.handle.deactivate("text-bench").expect("停用终态");
    assert_eq!(teardown.extension_id, "text-bench");
    assert_eq!(teardown.generation, bench.generation);
    (bench.clear_mount)();
    let cleared = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").is_none()
                && node_field(snapshot, TITLE, "name").is_none()
                && node_field(snapshot, WORKSPACE_ROOT, "name").is_some()
        })
        .expect("挂载子树未退出");
    // 扩展节点全部退出语义树；宿主根（操作面）仍存活。
    let extension_nodes = cleared["nodes"]
        .as_array()
        .expect("节点表")
        .iter()
        .filter(|node| {
            node["automation_id"]
                .as_str()
                .is_some_and(|id| id.starts_with("text-bench/"))
        })
        .count();
    assert_eq!(extension_nodes, 0, "扩展节点退出语义树");
    // 旧目标不可再操作：对已退出节点的动作明确失败。
    let stale_action = bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS);
    assert!(stale_action.is_err(), "旧目标不可再操作");
    // 宿主其他功能仍可用：操作面枚举、快照与截图照常工作。
    let windows = bench.client.list_windows().expect("操作面存活");
    assert_eq!(windows.len(), 1);
    let screenshot = bench
        .client
        .request(serde_json::json!({
            "type": "screenshot", "window_id": view.window_id
        }))
        .expect("停用后截图");
    assert_eq!(screenshot["ok"], true);
    // 重复停用返回真实失败，不伪报完成。
    let repeated = bench
        .handle
        .deactivate("text-bench")
        .expect_err("重复停用应失败");
    assert!(matches!(
        repeated,
        uix_app::app::extensions::ExtensionError::UnknownExtension(_)
    ));

    // 关闭：worker 排空释放，操作面回收端点；客户端随后观察到关闭事实。
    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
    let closed = bench.client.snapshot(view.window_id);
    assert!(closed.is_err(), "操作面关闭后端点不可再用");
}

/// 停用后同一宿主进程重新装载：新实例完成一次业务操作，旧代事件
/// 不污染新实例（陈旧代事件被稳定丢弃）。
#[test]
fn reload_after_teardown_completes_business_without_stale_pollution() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");

    // 首代停用并清空挂载。
    let old_generation = bench.generation;
    bench.handle.deactivate("text-bench").expect("停用");
    (bench.clear_mount)();
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_none()
        })
        .expect("挂载子树未退出");

    // 同一 worker 重新装载：新代身份递增，初始声明到达。
    let prepared = bench
        .handle
        .prepare(&delivered_package(1))
        .expect("重新准备");
    let receipt = bench.handle.activate(prepared).expect("重新激活");
    assert!(receipt.generation > old_generation, "新代身份不复用旧代");
    bench.generation = receipt.generation;
    let reloaded = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some_and(|text| text == "文本处理工作台 v1")
        })
        .expect("重装初始声明未到达");
    // 新实例状态干净：统计回到初始值，未被首代状态污染。
    assert_eq!(node_text(&reloaded, STATS), "统计: 尚未处理");

    // 旧代事件不污染新实例：投递首代 generation 的事件，稳定丢弃。
    bench
        .handle
        .event_sender()
        .send(uix_app::app::extensions::UiEvent {
            extension: "text-bench".to_string(),
            generation: old_generation,
            handler: "on-process".to_string(),
            payload: uix_app::app::extensions::UiEventPayload::Click,
        })
        .expect("投递陈旧事件");
    let untouched = bench
        .client
        .wait_for_snapshot(view.window_id, Duration::from_millis(600), |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .is_err();
    assert!(untouched, "陈旧代事件不得驱动新实例");

    // 新实例完成一次业务操作：读取 → 编辑 → 处理 → 提交。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("读取");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value")
                .is_some_and(|value| value.starts_with("  UIX") && value.contains("第二行内容"))
        })
        .expect("读取未应用");
    bench
        .client
        .perform_set_value(view.window_id, view.generation, DRAFT, " 重装后编辑 ")
        .expect("编辑");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some(" 重装后编辑 ")
        })
        .expect("编辑未应用");
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("处理");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .expect("处理未回显");
    bench
        .client
        .perform_invoke(view.window_id, view.generation, COMMIT)
        .expect("提交");
    let committed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATUS, "name").is_some_and(|text| text.contains("已提交"))
        })
        .expect("提交未回显");
    assert!(
        node_text(&committed, STATUS).contains("v2"),
        "{committed:?}"
    );
    {
        let store = bench.documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 2);
        assert_eq!(document.content, "重装后编辑", "新实例提交编辑后内容");
    }

    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
}
