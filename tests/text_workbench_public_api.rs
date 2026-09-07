//! 《软件动态扩展 × Agent 独立后台操作面》集成场景的公开消费者。
//!
//! 以文本处理工作台为场景：宿主持有版本化文档领域数据并显式授权
//! 查询 / 提交端口；Scheme 扩展提供真实文本统计算法与动态面板；AI 只经
//! `AgentWorkspace` 后台视口操作，与用户前台（`TestApp` 输入侧）互不
//! 干预局部交互状态。本文件只使用已文档化的公开 API。
#![cfg(all(
    feature = "agent-control",
    feature = "test-harness",
    feature = "extensions"
))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use uix::app::agent_client::AgentBridgeClient;
use uix::app::agent_workspace::AgentWorkspace;
use uix::app::extensions::{
    ExtensionHost, ExtensionPackage, ExtensionPort, ExtensionUiHandle, ExtensionValue, UiNode,
    UiProjector, UiUpdate,
};
use uix::prelude::*;
use uix::ui::test_harness::{AutomationNode, TestApp};

// 文档规定每进程一个后台操作面；消费者串行持有该公开租约。
static WORKSPACE: Mutex<()> = Mutex::new(());

// ---------- 宿主领域数据与共享端口 ----------

#[derive(Clone)]
struct Document {
    content: String,
    version: u64,
}

type SharedDocuments = Arc<Mutex<BTreeMap<String, Document>>>;

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

// ---------- 扩展包（与 examples/text_workbench.rs 同一业务算法）----------

fn workbench_package(mount: &str, version: &str, major: u32) -> ExtensionPackage {
    let manifest = format!(
        "(uix-extension (schema-version 1) (id \"text-bench\") (version \"{version}\") \
         (language r7rs-small) (entry \"main.scm\") \
         (capabilities documents-query documents-commit mount-{mount}) \
         (state-schema-version 1))"
    );
    let title = format!("文本处理工作台 · {mount} v{major}");
    let cjk_definition = if major >= 2 {
        r#"(define (cjk-count s)
  (count-if (lambda (c)
    (let ((code (char->integer c)))
      (and (>= code 19968) (<= code 40959)))) s))
(define (latin-char? c)
  (let ((code (char->integer c)))
    (and (or (char-alphabetic? c) (char-numeric? c))
         (not (and (>= code 19968) (<= code 40959))))))
(define (latin-word-count s)
  (define (walk i prev n)
    (if (= i (string-length s))
        n
        (let* ((c (string-ref s i))
               (word-char (latin-char? c)))
          (walk (+ i 1) word-char
                (if (and word-char (not prev)) (+ n 1) n)))))
  (walk 0 #f 0))
(define (word-count s) (+ (latin-word-count s) (cjk-count s)))"#
    } else {
        r#"(define (word-count s)
  (define (walk i in-word n)
    (if (= i (string-length s))
        (if in-word (+ n 1) n)
        (let ((space (char-whitespace? (string-ref s i))))
          (cond ((and (not space) (not in-word)) (walk (+ i 1) #t (+ n 1)))
                ((and space in-word) (walk (+ i 1) #f n))
                (else (walk (+ i 1) in-word n))))))
  (walk 0 #f 0))"#
    };
    let stats_tail = if major >= 2 {
        r#"   (string-append " · CJK " (number->string (cjk-count s)))"#
    } else {
        ""
    };
    let cjk_line = if major >= 2 {
        r#"        (list 'text (list 'key "cjk")
              (string-append "CJK 字符: " (number->string (cjk-count draft))))
"#
    } else {
        ""
    };
    let source = format!(
        r#"
(define draft "")
(define result "尚未处理")
(define target "intro")
(define base-version 0)
(define status "就绪")
(define (count-if pred s)
  (define (walk i n)
    (if (= i (string-length s))
        n
        (walk (+ i 1) (if (pred (string-ref s i)) (+ n 1) n))))
  (walk 0 0))
(define (non-space-count s)
  (count-if (lambda (c) (not (char-whitespace? c))) s))
(define (line-count s)
  (+ 1 (count-if (lambda (c) (char=? c #\newline)) s)))
{cjk_definition}
(define (collapse-chars chars acc)
  (cond ((null? chars) (list->string (reverse acc)))
        ((char-whitespace? (car chars))
         (if (or (null? acc) (char=? (car acc) #\space))
             (collapse-chars (cdr chars) acc)
             (collapse-chars (cdr chars) (cons #\space acc))))
        (else (collapse-chars (cdr chars) (cons (car chars) acc)))))
(define (trim-tail s)
  (define (drop i)
    (if (and (> i 0) (char-whitespace? (string-ref s (- i 1))))
        (drop (- i 1))
        i))
  (substring s 0 (drop (string-length s))))
(define (normalize-text s)
  (trim-tail (collapse-chars (string->list s) '())))
(define (stats->text s)
  (string-append
   "字符 " (number->string (string-length s))
   " · 非空白 " (number->string (non-space-count s))
   " · 行 " (number->string (line-count s))
   " · 词 " (number->string (word-count s))
{stats_tail}))
(define (draft-input reset?)
  (if reset?
      (list 'input (list 'key "draft")
            (list 'placeholder "输入或读取要处理的文本")
            (list 'on-change 'on-draft-change)
            (list 'reset #t)
            draft)
      (list 'input (list 'key "draft")
            (list 'placeholder "输入或读取要处理的文本")
            (list 'on-change 'on-draft-change)
            draft)))
(define (declaration reset?)
  (list 'column (list 'key "root") (list 'pad 12.0) (list 'gap 8.0)
        (list 'text (list 'key "title") (list 'size 14.0) "{title}")
        (draft-input reset?)
        (list 'row (list 'key "controls") (list 'gap 8.0)
              (list 'button (list 'key "process") (list 'on-click 'on-process) "处理文本")
              (list 'button (list 'key "load") (list 'on-click 'on-load) "读取文档")
              (list 'button (list 'key "commit") (list 'on-click 'on-commit) "提交到文档"))
        (list 'text (list 'key "stats") (string-append "统计: " result))
{cjk_line}        (list 'text (list 'key "status") (string-append "状态: " status))
        (list 'text (list 'key "doc")
              (string-append "目标: " target " @v" (number->string base-version)))))
(define (on-draft-change text)
  (set! draft text)
  (submit-ui! '{mount} (declaration #f)))
(define (on-process)
  (set! result (stats->text draft))
  (set! status "已处理")
  (submit-ui! '{mount} (declaration #f)))
(define (on-load)
  (let* ((entry (documents-query target))
         (content (list-ref entry 0))
         (version (list-ref entry 1)))
    (set! base-version version)
    (set! draft content)
    (set! status (string-append "已读取 v" (number->string version)))
    (submit-ui! '{mount} (declaration #t))))
(define (on-commit)
  (let ((outcome (documents-commit target base-version (normalize-text draft))))
    (if (eq? (list-ref outcome 0) 'ok)
        (begin
          (set! base-version (list-ref outcome 1))
          (set! status (string-append "已提交 v" (number->string (list-ref outcome 1)))))
        (set! status (string-append "版本冲突：文档已是 v"
                                    (number->string (list-ref outcome 1)))))
    (submit-ui! '{mount} (declaration #f))))
(register-handler! "on-draft-change" on-draft-change)
(register-handler! "on-process" on-process)
(register-handler! "on-load" on-load)
(register-handler! "on-commit" on-commit)
(register-command! "stats" (lambda (text) (stats->text text)))
(register-command! "normalize" (lambda (text) (normalize-text text)))
(register-command! "engine-version" (lambda () "{version}"))
(register-state-export!
  (lambda () (list draft result target base-version status)))
(register-state-import!
  (lambda (snapshot)
    (set! draft (list-ref snapshot 0))
    (set! result (list-ref snapshot 1))
    (set! target (list-ref snapshot 2))
    (set! base-version (list-ref snapshot 3))
    (set! status (list-ref snapshot 4))
    (submit-ui! '{mount} (declaration #t))))
(submit-ui! '{mount} (declaration #f))
"#
    );
    let sources: BTreeMap<String, String> =
        [("main.scm".to_string(), source)].into_iter().collect();
    ExtensionPackage::from_parts(manifest, sources).expect("包构造")
}

// ---------- 后台操作面装配 ----------

/// 一次后台装配：workspace（根 + 投影器 + 修订 State）与扩展 worker。
struct BackgroundWorkbench {
    workspace: uix::app::agent_workspace::AgentWorkspaceHandle,
    client: AgentBridgeClient,
    handle: ExtensionUiHandle,
    documents: SharedDocuments,
    generation: u64,
}

fn spawn_background_workbench() -> BackgroundWorkbench {
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
                (Some(node), Some(projector)) => projector.project("agent-panel", &node),
                _ => column(Vec::<ViewNode>::new()),
            };
            column_fit((label("AI 独立后台操作面 · 文本处理").font_size(15.0), panel))
        })
        .spawn()
        .expect("后台操作面启动")
    };
    // 扩展线程的声明出口：sink 直接应用 owned 声明并唤醒后台 owner。
    let host = ExtensionHost::new()
        .with_mount("agent-panel")
        .with_port("documents-query", query_port(Arc::clone(&documents)))
        .with_port("documents-commit", commit_port(Arc::clone(&documents)))
        .with_ui_sink(Arc::new({
            let sink_current = Arc::clone(&current);
            let sink_revision = revision.clone();
            let sink_projector = Arc::clone(&projector);
            let poster = workspace.poster();
            move |update| match update {
                UiUpdate::Applied {
                    node, generation, ..
                } => {
                    if let Some(projector) = sink_projector.lock().expect("后台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
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
    let prepared = handle
        .prepare(&workbench_package("agent-panel", "0.1.0", 1))
        .expect("准备");
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
    let client = workspace.client().expect("后台客户端");
    BackgroundWorkbench {
        workspace,
        client,
        handle,
        documents,
        generation: receipt.generation,
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

const DRAFT: &str = "text-bench/agent-panel/root/draft";
const PROCESS: &str = "text-bench/agent-panel/root/controls/process";
const LOAD: &str = "text-bench/agent-panel/root/controls/load";
const COMMIT: &str = "text-bench/agent-panel/root/controls/commit";
const STATS: &str = "text-bench/agent-panel/root/stats";
const TITLE: &str = "text-bench/agent-panel/root/title";
const STATUS: &str = "text-bench/agent-panel/root/status";
const CJK: &str = "text-bench/agent-panel/root/cjk";

const TIMEOUT: Duration = Duration::from_secs(8);

// ---------- 场景 ----------

/// AI 在后台完成读取 → 输入 → 处理 → 查看结果 → 业务提交的完整闭环；
/// 只有经授权的提交才改变共享领域数据，语义状态与离屏 PNG 对应。
#[test]
fn background_agent_roundtrip_and_authorized_commit() {
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
    assert_eq!(
        node_text(&initial, TITLE),
        "文本处理工作台 · agent-panel v1"
    );

    // 输入草稿（AI 私有交互状态；事件经 worker 执行脚本）。
    // 单行输入组件会把换行规范化为空格，这里使用无换行文本。
    bench
        .client
        .perform_set_value(
            view.window_id,
            view.generation,
            DRAFT,
            "  hello   世界  abc  ",
        )
        .expect("输入草稿");
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").as_deref() == Some("  hello   世界  abc  ")
        })
        .expect("草稿未进入后台语义状态");

    // 执行文本处理算法（v1 口径：空白分段词数）。
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
        "统计: 字符 19 · 非空白 10 · 行 1 · 词 3"
    );
    // 等待完成呈现：后台语义已应用，离屏帧已绘制（呈现回执）。
    let revision = processed["revision"].as_u64().expect("修订号");
    bench
        .client
        .wait_until_presented(view.window_id, view.generation, revision, TIMEOUT)
        .expect("离屏帧呈现");

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
    {
        let store = bench.documents.lock().expect("文档存储锁");
        let document = store.get("intro").expect("文档存在");
        assert_eq!(document.version, 1, "冲突提交不改变共享数据");
    }

    // 读取宿主文档（显式授权的只读端口），携带版本提交。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, LOAD)
        .expect("读取文档");
    let loaded = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, DRAFT, "value").is_some_and(|value| value.starts_with("  UIX"))
        })
        .expect("文档内容未进入草稿");
    assert!(
        node_text(&loaded, STATUS).contains("已读取 v1"),
        "{loaded:?}"
    );

    // 业务提交：共享文档按端口契约更新（版本递增、内容规范化）。
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
            document.content, "UIX 文本处理工作台 示例文档 第二行内容",
            "提交的是规范化后的文本"
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

    // 前台用户 worker（挂载位 user-panel）：声明经通道由测试线程应用。
    let fg_host = ExtensionHost::new()
        .with_mount("user-panel")
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
        .prepare(&workbench_package("user-panel", "0.1.0", 1))
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
                    node, generation, ..
                } => {
                    if let Some(projector) = projector.lock().expect("前台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
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
            (Some(node), Some(projector)) => projector.project("user-panel", &node),
            _ => column(Vec::<ViewNode>::new()),
        }
    });
    user.settle().expect("前台初始帧");
    let user_before = user.snapshot().nodes;
    let fg_draft = "text-bench/user-panel/root/draft";

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
                    (Some(node), Some(projector)) => projector.project("agent-panel", &node),
                    _ => column(Vec::<ViewNode>::new()),
                };
                column_fit((label("AI 后台").font_size(15.0), panel))
            })
            .spawn()
            .expect("后台操作面启动")
        };
        let host = ExtensionHost::new()
            .with_mount("agent-panel")
            .with_port("documents-query", query_port(Arc::clone(&documents)))
            .with_port("documents-commit", commit_port(Arc::clone(&documents)))
            .with_ui_sink(Arc::new({
                let sink_current = Arc::clone(&current);
                let sink_revision = revision.clone();
                let sink_projector = Arc::clone(&projector);
                let poster = workspace.poster();
                move |update| match update {
                    UiUpdate::Applied {
                        node, generation, ..
                    } => {
                        if let Some(projector) =
                            sink_projector.lock().expect("后台投影器锁").as_mut()
                        {
                            projector.update_generation(generation);
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
            .prepare(&workbench_package("agent-panel", "0.1.0", 1))
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
        }
    };
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("后台初始声明未到达");

    // AI 在后台输入、处理，读取版本后提交（完整业务闭环）。
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
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("后台处理");
    let ai_stats = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .expect("后台处理结果未回显");
    assert_eq!(
        node_text(&ai_stats, STATS),
        "统计: 字符 15 · 非空白 8 · 行 1 · 词 3"
    );
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
    user.invoke("text-bench/user-panel/root/controls/load")
        .expect("前台读取");
    apply_foreground();
    user.settle().expect("前台帧");
    let fg_status = user
        .text("text-bench/user-panel/root/status")
        .expect("状态");
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

/// 热替换同时升级后台算法与界面，兼容状态与草稿保留；候选失败不破坏旧代。
#[test]
fn hot_replace_upgrades_background_and_keeps_compatible_state() {
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
    bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text != "统计: 尚未处理")
        })
        .expect("v1 结果未回显");

    // 先注入失败候选（状态 schema 不一致）：替换被拒，旧代继续服务。
    let bad_manifest = "(uix-extension (schema-version 1) (id \"text-bench\") (version \"0.3.0\") \
         (language r7rs-small) (entry \"main.scm\") \
         (capabilities documents-query documents-commit mount-agent-panel) \
         (state-schema-version 2))";
    let bad_sources: BTreeMap<String, String> =
        [("main.scm".to_string(), "(define x 1)".to_string())]
            .into_iter()
            .collect();
    let bad =
        ExtensionPackage::from_parts(bad_manifest.to_string(), bad_sources).expect("坏包构造");
    let bad_prepared = bench.handle.prepare(&bad).expect("坏包可准备");
    let rejected = bench
        .handle
        .replace(bad_prepared, bench.generation)
        .expect_err("schema 不一致应拒绝");
    assert!(matches!(
        rejected,
        uix::app::extensions::ExtensionError::Incompatible(_)
    ));
    let still_v1 = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name")
                .is_some_and(|text| text == "文本处理工作台 · agent-panel v1")
        })
        .expect("旧代界面保留");
    assert_eq!(
        node_text(&still_v1, STATS),
        "统计: 字符 19 · 非空白 10 · 行 1 · 词 3"
    );

    // 正常替换 v2：算法（词数口径 + CJK 统计）与界面（新增 CJK 行）共同升级。
    let candidate = bench
        .handle
        .prepare(&workbench_package("agent-panel", "0.2.0", 2))
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
            node_field(snapshot, TITLE, "name")
                .is_some_and(|text| text == "文本处理工作台 · agent-panel v2")
        })
        .expect("新代界面未到达");
    // 迁移保留兼容状态：草稿与状态文本按导入路径重建。
    assert_eq!(
        node_value(&upgraded, DRAFT),
        "  hello   世界  abc  ",
        "草稿保留"
    );
    assert!(
        node_text(&upgraded, STATUS).contains("已处理"),
        "扩展状态保留"
    );
    // 新界面节点实际生效。
    assert_eq!(node_text(&upgraded, CJK), "CJK 字符: 2");

    // 新算法实际生效：同文本重新处理，词数口径从分段改为拉丁词 + CJK 字。
    bench
        .client
        .perform_invoke(view.window_id, view.generation, PROCESS)
        .expect("新代处理");
    let reprocessed = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, STATS, "name").is_some_and(|text| text.contains("词 4"))
        })
        .expect("新算法未生效");
    assert_eq!(
        node_text(&reprocessed, STATS),
        "统计: 字符 19 · 非空白 10 · 行 1 · 词 4 · CJK 2"
    );
    // 命令入口同步升级（无窗口逻辑与面板同代）。
    let version = bench
        .handle
        .call_command("text-bench", "engine-version", &[])
        .expect("版本命令");
    assert_eq!(version, ExtensionValue::Text("0.2.0".to_string()));

    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
}

/// 撤权、停用与关闭返回真实终态：能力退出、挂载资源释放、不伪报完成。
#[test]
fn revocation_deactivation_and_close_return_real_results() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|error| error.into_inner());
    let mut bench = spawn_background_workbench();
    let view = bench.client.list_windows().expect("窗口枚举")[0];
    let ready = bench
        .client
        .wait_for_snapshot(view.window_id, TIMEOUT, |snapshot| {
            node_field(snapshot, TITLE, "name").is_some()
        })
        .expect("初始声明未到达");

    // 撤权：新调用立即被拒；已呈现界面保留为可观察状态。
    bench.handle.revoke("text-bench").expect("撤权");
    let denied = bench
        .handle
        .call_command("text-bench", "engine-version", &[])
        .expect_err("撤权后命令应被拒");
    assert!(matches!(
        denied,
        uix::app::extensions::ExtensionError::CapabilityDenied(_)
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
    assert_eq!(node_text(&ready, TITLE), "文本处理工作台 · agent-panel v1");

    // 停用：真实终态后由宿主清空挂载位（示例合同：应用负责子树退出）。
    let teardown = bench.handle.deactivate("text-bench").expect("停用终态");
    assert_eq!(teardown.extension_id, "text-bench");
    assert_eq!(teardown.generation, bench.generation);
    let gone = bench
        .handle
        .call_command("text-bench", "engine-version", &[])
        .expect_err("停用后实例不可调用");
    assert!(matches!(
        gone,
        uix::app::extensions::ExtensionError::UnknownExtension(_)
    ));
    // 重复停用返回真实失败，不伪报完成。
    let repeated = bench
        .handle
        .deactivate("text-bench")
        .expect_err("重复停用应失败");
    assert!(matches!(
        repeated,
        uix::app::extensions::ExtensionError::UnknownExtension(_)
    ));

    // 关闭：worker 排空释放，操作面回收端点；客户端随后观察到关闭事实。
    bench.handle.shutdown().expect("worker 关停");
    bench.workspace.close().expect("操作面关闭");
    let closed = bench.client.snapshot(view.window_id);
    assert!(closed.is_err(), "操作面关闭后端点不可再用");
}
