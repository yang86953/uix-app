//! 文本处理工作台：软件动态扩展 × Agent 独立后台操作面集成示例。
//!
//! 运行：
//! ```sh
//! cargo run --release --features extensions,agent-control --example text_workbench \
//!   [--hot-replace] [--quit-after 秒]
//! ```
//!
//! - 宿主持有文档领域数据（内容 + 乐观并发版本），显式授权
//!   `documents-query`（只读）与 `documents-commit`（受控写；携带预期
//!   版本，冲突时携带当前版本拒绝，不静默覆盖）。
//! - Scheme 扩展实现真实文本处理算法（字符 / 非空白 / 行 / 词统计、
//!   空白规范化）与动态面板，不是标题或颜色装饰。
//! - 前台真窗与后台操作面各自拥有扩展 worker、投影器与交互草稿库；
//!   只共享领域端口，不 clone 同一投影器或前台 AppHandle。
//! - AI 经 `scripts/agent_client.py` 绑定本进程的后台视口，完成读取 →
//!   输入 → 处理 → 查看结果 → 提交的完整业务闭环；合法提交后的共享
//!   文档更新按业务契约对前台可见。
//! - `--hot-replace` 启动 5 秒后两侧同时升级 v2（统计算法 + 界面共同
//!   升级，兼容状态与草稿按合同保留）；前台按钮亦可手动触发。
//!
//! 前台控制区按钮属于宿主原生 UI，不是扩展能力；AI 只经后台操作面
//! 操作，不控制用户窗口。边界见 docs/使用/能力与边界/Agent后台操作面.md。

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use uix::app::agent_workspace::AgentWorkspace;
use uix::app::extensions::{
    ExtensionHost, ExtensionPackage, ExtensionPort, ExtensionUiHandle, ExtensionValue, UiNode,
    UiProjector, UiUpdate,
};
use uix::prelude::*;

/// 宿主领域数据：文档内容与提交版本。
#[derive(Clone)]
struct Document {
    content: String,
    version: u64,
}

type SharedDocuments = Arc<Mutex<BTreeMap<String, Document>>>;

/// 提交结果通知：`(文档键, 新版本)`，供前台在业务契约内展示共享更新。
type CommitNotice = mpsc::Sender<(String, u64)>;

/// 只读端口：`(documents-query 键)` → `(内容 版本)`。
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

/// 受控写端口：`(documents-commit 键 预期版本 新文本)`。
///
/// 版本一致才写入并递增；不一致返回 `(conflict 当前版本)`，由扩展向
/// 用户呈现，宿主不静默覆盖任何一方的新修改。
fn commit_port(documents: SharedDocuments, notice: CommitNotice) -> ExtensionPort {
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
            let committed = document.version;
            let _ = notice.send((key.clone(), committed));
            Ok(ExtensionValue::List(vec![
                ExtensionValue::Symbol("ok".to_string()),
                ExtensionValue::Int(committed as i64),
            ]))
        }
        _ => Err("需要 (文档键 预期版本 新文本) 三个参数".to_string()),
    })
}

fn main() {
    let hot_replace = std::env::args().any(|argument| argument == "--hot-replace");
    let quit_after = std::env::args()
        .position(|argument| argument == "--quit-after")
        .and_then(|index| std::env::args().nth(index + 1))
        .and_then(|value| value.parse::<u64>().ok());

    // ---- 宿主领域数据与共享端口（前台 / 后台 worker 共同授权面）----
    let documents: SharedDocuments = Arc::new(Mutex::new(
        [(
            "intro".to_string(),
            Document {
                content: "  UIX 文本处理工作台   示例文档\n第二行内容  ".to_string(),
                version: 1,
            },
        )]
        .into_iter()
        .collect(),
    ));
    let (committed_tx, committed_rx) = mpsc::channel::<(String, u64)>();
    let shared_query = query_port(Arc::clone(&documents));
    let shared_commit = commit_port(Arc::clone(&documents), committed_tx);

    // ---- 后台操作面：独立根、投影器与交互状态（AI 专用）----
    let bg_revision = State::new(0u64);
    let bg_current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
    let bg_projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));
    let bg_generation: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let bg_handle_slot: Arc<Mutex<Option<ExtensionUiHandle>>> = Arc::new(Mutex::new(None));

    let workspace = {
        let root_revision = bg_revision.clone();
        let root_current = Arc::clone(&bg_current);
        let root_projector = Arc::clone(&bg_projector);
        let workspace = AgentWorkspace::new(760, 600, move || {
            root_revision.get();
            let node = root_current.lock().expect("后台声明锁").clone();
            let mut projector = root_projector.lock().expect("后台投影器锁");
            let panel = match (node, projector.as_mut()) {
                (Some(node), Some(projector)) => projector.project("agent-panel", &node),
                _ => column(Vec::<ViewNode>::new()),
            };
            column_fit((label("AI 独立后台操作面 · 文本处理").font_size(15.0), panel))
        })
        .title("UIX text workbench (agent workspace)");
        match workspace.spawn() {
            Ok(workspace) => workspace,
            Err(error) => {
                eprintln!("[workbench] 后台操作面启动失败：{error:?}");
                std::process::exit(1);
            }
        }
    };
    // 后台 worker：引擎在独立执行线程；声明经 sink 直接应用并唤醒后台
    // owner（poster 能力与 handle 一致，只是允许扩展线程持有）。
    let bg_host = ExtensionHost::new()
        .with_mount("agent-panel")
        .with_port("documents-query", Arc::clone(&shared_query))
        .with_port("documents-commit", Arc::clone(&shared_commit))
        .with_ui_sink(Arc::new({
            let current = Arc::clone(&bg_current);
            let revision = bg_revision.clone();
            let projector = Arc::clone(&bg_projector);
            let poster = workspace.poster();
            move |update| match update {
                UiUpdate::Applied {
                    node, generation, ..
                } => {
                    if let Some(projector) = projector.lock().expect("后台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
                    }
                    *current.lock().expect("后台声明锁") = Some(node);
                    revision.update(|value| *value += 1);
                    poster.wake();
                }
                UiUpdate::Rejected { mount, reason, .. } => {
                    eprintln!("[workbench] 后台声明被拒绝（{mount}）：{reason}")
                }
            }
        }));
    let bg_handle = bg_host.spawn_worker();
    let bg_receipt = match bg_handle
        .prepare(&workbench_package("agent-panel", "0.1.0", 1))
        .and_then(|prepared| bg_handle.activate(prepared))
    {
        Ok(receipt) => receipt,
        Err(error) => {
            eprintln!("[workbench] 后台扩展装载失败：{error}");
            let _ = bg_handle.shutdown();
            let _ = workspace.close();
            std::process::exit(1);
        }
    };
    *bg_projector.lock().expect("后台投影器锁") = Some(UiProjector::new(
        &bg_receipt.extension_id,
        bg_receipt.generation,
        bg_handle.event_sender(),
    ));
    // 装配补帧：初始声明的重帧可能早于投影器安装（渲染出空面板），
    // 安装后主动推进修订并唤醒，保证一次带投影的完整重建。
    bg_revision.update(|value| *value += 1);
    workspace.poster().wake();
    *bg_generation.lock().expect("后台代际锁") = bg_receipt.generation;
    *bg_handle_slot.lock().expect("后台句柄锁") = Some(bg_handle.clone());
    // 关闭权共享槽：定时退出与主线程收尾都会真实释放后台资源。
    let bg_poster = workspace.poster();
    let workspace_slot: Arc<Mutex<Option<_>>> = Arc::new(Mutex::new(Some(workspace)));

    println!(
        "[workbench] 文本处理工作台已启动：pid={}（前台用户窗口 + 后台操作面）",
        std::process::id()
    );
    println!(
        "[workbench] 后台接入：python3 scripts/agent_client.py apps 枚举后 --instance 绑定本进程"
    );

    // ---- 前台真窗：用户自己的扩展 worker、投影器与草稿库 ----
    let fg_revision = State::new(0u64);
    let fg_current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
    let fg_projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));
    let fg_generation: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let fg_handle_slot: Arc<Mutex<Option<ExtensionUiHandle>>> = Arc::new(Mutex::new(None));
    let (fg_updates_tx, fg_updates_rx) = mpsc::channel::<UiUpdate>();
    let doc_summary = State::new("共享文档 intro @v1".to_string());

    let fg_host = ExtensionHost::new()
        .with_mount("user-panel")
        .with_port("documents-query", shared_query)
        .with_port("documents-commit", shared_commit)
        .with_ui_sink(Arc::new(move |update| {
            let _ = fg_updates_tx.send(update);
        }));
    let fg_handle = fg_host.spawn_worker();

    let app = App::new()
        .title("UIX 文本处理工作台")
        .size(560, 640)
        .on_start({
            let fg_handle = fg_handle.clone();
            let fg_current = Arc::clone(&fg_current);
            let fg_revision = fg_revision.clone();
            let fg_projector = Arc::clone(&fg_projector);
            let fg_handle_slot = Arc::clone(&fg_handle_slot);
            let fg_generation = Arc::clone(&fg_generation);
            let doc_summary = doc_summary.clone();
            move |app_handle| {
                // 声明交付桥：扩展线程 sink → 通道 → 本线程 → 窗口 owner
                // thread 应用（UI 线程不执行任何 Lisp）。
                let bridge_projector = Arc::clone(&fg_projector);
                let deliver = {
                    let current = Arc::clone(&fg_current);
                    let revision = fg_revision.clone();
                    let projector = Arc::clone(&bridge_projector);
                    let app_handle = app_handle.clone();
                    move |update: UiUpdate| match update {
                        UiUpdate::Applied { node, generation, .. } => {
                            let current = Arc::clone(&current);
                            let revision = revision.clone();
                            let projector = Arc::clone(&projector);
                            app_handle.post_to_ui(move || {
                                if let Some(projector) =
                                    projector.lock().expect("前台投影器锁").as_mut()
                                {
                                    projector.update_generation(generation);
                                }
                                *current.lock().expect("前台声明锁") = Some(node);
                                revision.update(|value| *value += 1);
                            });
                        }
                        UiUpdate::Rejected { mount, reason, .. } => {
                            eprintln!("[workbench] 前台声明被拒绝（{mount}）：{reason}")
                        }
                    }
                };
                thread::spawn(move || {
                    while let Ok(update) = fg_updates_rx.recv() {
                        deliver(update);
                    }
                });
                // 合法业务提交的通知：更新共享文档摘要（业务契约内可见）。
                let summary_state = doc_summary.clone();
                thread::spawn(move || {
                    while let Ok((key, version)) = committed_rx.recv() {
                        let summary_state = summary_state.clone();
                        app_handle.post_to_ui(move || {
                            summary_state.set(format!("共享文档 {key} @v{version}（后台或前台提交后更新）"));
                        });
                    }
                });
                // 前台扩展装载：准备 → 激活（初始声明经桥回投）。
                match fg_handle
                    .prepare(&workbench_package("user-panel", "0.1.0", 1))
                    .and_then(|prepared| fg_handle.activate(prepared))
                {
                    Ok(receipt) => {
                        *fg_projector.lock().expect("前台投影器锁") = Some(UiProjector::new(
                            &receipt.extension_id,
                            receipt.generation,
                            fg_handle.event_sender(),
                        ));
                        *fg_generation.lock().expect("前台代际锁") = receipt.generation;
                        *fg_handle_slot.lock().expect("前台句柄锁") = Some(fg_handle.clone());
                    }
                    Err(error) => {
                        *fg_current.lock().expect("前台声明锁") = Some(load_failed_node(&error.to_string()));
                        fg_revision.update(|value| *value += 1);
                    }
                }
            }
        })
        .root({
            let root_revision = fg_revision.clone();
            let root_current = Arc::clone(&fg_current);
            let root_projector = Arc::clone(&fg_projector);
            let root_summary = doc_summary.clone();
            let upgrade_state = State::new(false);
            let revoke_state = State::new(false);
            let teardown_state = State::new(false);
            let fg_upgrade_slot = Arc::clone(&fg_handle_slot);
            let bg_upgrade_slot = Arc::clone(&bg_handle_slot);
            let fg_upgrade_generation = Arc::clone(&fg_generation);
            let bg_upgrade_generation = Arc::clone(&bg_generation);
            let revoke_slot = Arc::clone(&bg_handle_slot);
            let teardown_slot = Arc::clone(&bg_handle_slot);
            let teardown_current = Arc::clone(&bg_current);
            let teardown_revision = bg_revision.clone();
            let teardown_poster = bg_poster.clone();
            move || {
                root_revision.get();
                let summary = root_summary.get();
                let node = root_current.lock().expect("前台声明锁").clone();
                let mut projector = root_projector.lock().expect("前台投影器锁");
                let panel = match (node, projector.as_mut()) {
                    (Some(node), Some(projector)) => projector.project("user-panel", &node),
                    _ => column(Vec::<ViewNode>::new()),
                };
                // 宿主控制区：原生 UI，每次重建时构建（ViewNode 不可克隆）。
                let upgrade = button("热替换扩展到 v2")
                    .on_click(&upgrade_state, {
                        let fg_handle_slot = Arc::clone(&fg_upgrade_slot);
                        let bg_handle_slot = Arc::clone(&bg_upgrade_slot);
                        let fg_generation = Arc::clone(&fg_upgrade_generation);
                        let bg_generation = Arc::clone(&bg_upgrade_generation);
                        move |_flag: &State<bool>| {
                            spawn_upgrade(
                                Arc::clone(&fg_handle_slot),
                                Arc::clone(&bg_handle_slot),
                                Arc::clone(&fg_generation),
                                Arc::clone(&bg_generation),
                            );
                        }
                    })
                    .build()
                    .automation_id("upgrade-button");
                let revoke = button("撤权后台扩展")
                    .on_click(&revoke_state, {
                        let bg_handle_slot = Arc::clone(&revoke_slot);
                        move |_flag: &State<bool>| {
                            let bg_handle_slot = Arc::clone(&bg_handle_slot);
                            thread::spawn(move || {
                                let Some(bg_handle) = bg_handle_slot
                                    .lock()
                                    .expect("后台句柄锁")
                                    .clone()
                                else {
                                    return;
                                };
                                match bg_handle.revoke("text-bench") {
                                    Ok(()) => eprintln!(
                                        "[workbench] 后台扩展已撤权：新调用与面板事件被拒绝"
                                    ),
                                    Err(error) => {
                                        eprintln!("[workbench] 撤权失败：{error}")
                                    }
                                }
                            });
                        }
                    })
                    .build()
                    .automation_id("revoke-button");
                let teardown = button("停用后台扩展")
                    .on_click(&teardown_state, {
                        let bg_handle_slot = Arc::clone(&teardown_slot);
                        let bg_current = Arc::clone(&teardown_current);
                        let bg_revision = teardown_revision.clone();
                        let poster = teardown_poster.clone();
                        move |_flag: &State<bool>| {
                            let bg_handle_slot = Arc::clone(&bg_handle_slot);
                            let bg_current = Arc::clone(&bg_current);
                            let bg_revision = bg_revision.clone();
                            let poster = poster.clone();
                            thread::spawn(move || {
                                let Some(bg_handle) = bg_handle_slot
                                    .lock()
                                    .expect("后台句柄锁")
                                    .clone()
                                else {
                                    return;
                                };
                                match bg_handle.deactivate("text-bench") {
                                    Ok(receipt) => {
                                        // 终态真实返回后按合同清空挂载子树。
                                        *bg_current.lock().expect("后台声明锁") = None;
                                        bg_revision.update(|value| *value += 1);
                                        poster.wake();
                                        eprintln!(
                                            "[workbench] 后台扩展已停用（generation {}），挂载位已清空",
                                            receipt.generation
                                        );
                                    }
                                    Err(error) => {
                                        eprintln!("[workbench] 停用失败：{error}")
                                    }
                                }
                            });
                        }
                    })
                    .build()
                    .automation_id("teardown-button");
                column_fit((
                    label("用户前台 · 文本处理工作台").font_size(15.0),
                    label(summary.as_str())
                        .font_size(12.0)
                        .automation_id("doc-summary"),
                    panel,
                    label("宿主控制（原生 UI，不属于扩展能力）")
                        .font_size(11.0),
                    row((upgrade, revoke, teardown)).gap(8.0),
                ))
            }
        });

    // 演示热替换：--hot-replace 启动 5 秒后两侧共同升级 v2。
    if hot_replace {
        let fg_handle_slot = Arc::clone(&fg_handle_slot);
        let bg_handle_slot = Arc::clone(&bg_handle_slot);
        let fg_generation = Arc::clone(&fg_generation);
        let bg_generation = Arc::clone(&bg_generation);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(5));
            spawn_upgrade(fg_handle_slot, bg_handle_slot, fg_generation, bg_generation);
        });
    }
    // 自动退出：到点先真实排空两个扩展 worker 并回收后台操作面，再结束
    // 进程（前台窗口随进程退出；扩展实例的释放在上方有真实终态）。
    if let Some(seconds) = quit_after {
        let workspace_slot = Arc::clone(&workspace_slot);
        let fg_handle_slot = Arc::clone(&fg_handle_slot);
        let bg_handle_slot = Arc::clone(&bg_handle_slot);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(seconds));
            eprintln!("[workbench] --quit-after 到期，释放资源并退出");
            if let Some(handle) = fg_handle_slot.lock().expect("前台句柄锁").take() {
                let _ = handle.shutdown();
            }
            if let Some(handle) = bg_handle_slot.lock().expect("后台句柄锁").take() {
                let _ = handle.shutdown();
            }
            if let Some(workspace) = workspace_slot.lock().expect("操作面锁").take() {
                let _ = workspace.close();
            }
            std::process::exit(0);
        });
    }

    let exit = app.run();
    // 优雅关闭：两个 worker 各自排空释放，后台操作面回收视口与端点。
    let _ = fg_handle.shutdown();
    let _ = bg_handle.shutdown();
    if let Some(workspace) = workspace_slot.lock().expect("操作面锁").take() {
        let _ = workspace.close();
    }
    std::process::exit(exit);
}

/// 双 worker 热替换：候选在独立线程准备与提交，UI 线程不等待求值。
fn spawn_upgrade(
    fg_handle_slot: Arc<Mutex<Option<ExtensionUiHandle>>>,
    bg_handle_slot: Arc<Mutex<Option<ExtensionUiHandle>>>,
    fg_generation: Arc<Mutex<u64>>,
    bg_generation: Arc<Mutex<u64>>,
) {
    thread::spawn(move || {
        for (handle_slot, generation_slot, mount) in [
            (fg_handle_slot, fg_generation, "user-panel"),
            (bg_handle_slot, bg_generation, "agent-panel"),
        ] {
            let Some(handle) = handle_slot.lock().expect("扩展句柄锁").clone() else {
                continue;
            };
            let expected = *generation_slot.lock().expect("扩展代际锁");
            let upgraded = workbench_package(mount, "0.2.0", 2);
            match handle
                .prepare(&upgraded)
                .and_then(|candidate| handle.replace(candidate, expected))
            {
                Ok(receipt) => {
                    *generation_slot.lock().expect("扩展代际锁") = receipt.generation;
                    eprintln!(
                        "[workbench] {mount} 已升级 v{}（generation {}，migrated={}）",
                        receipt.version, receipt.generation, receipt.migrated
                    );
                }
                Err(error) => eprintln!("[workbench] {mount} 升级失败：{error}"),
            }
        }
    });
}

fn load_failed_node(message: &str) -> UiNode {
    UiNode::Column {
        key: "root".to_string(),
        padding: Some(14.0),
        gap: None,
        background: None,
        children: vec![UiNode::Text {
            key: "error".to_string(),
            content: format!("扩展装载失败：{message}"),
            color: None,
            size: Some(13.0),
        }],
    }
}

/// 构造文本处理工作台扩展包（v1：分段词数；v2：拉丁词 + CJK 字口径，
/// 并在界面增加 CJK 统计行——算法与界面共同升级）。
fn workbench_package(mount: &str, version: &str, major: u32) -> ExtensionPackage {
    let manifest = format!(
        "(uix-extension (schema-version 1) (id \"text-bench\") (version \"{version}\") \
         (language r7rs-small) (entry \"main.scm\") \
         (capabilities documents-query documents-commit mount-{mount}) \
         (state-schema-version 1))"
    );
    let title = format!("文本处理工作台 · {mount} v{major}");
    // v2 才定义 cjk-count 并在统计与界面中使用；v1 无该节点。
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
;; 文本处理工作台扩展（挂载位 {mount}，算法代 v{major}）
(define draft "")
(define result "尚未处理")
(define target "intro")
(define base-version 0)
(define status "就绪")

;; ---- 文本统计算法 ----
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

;; ---- 空白规范化：压缩连续空白为单空格并去首尾 ----
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

;; ---- 动态面板 ----
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

;; ---- 面板事件 ----
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

;; ---- 无窗口命令（逻辑部分与面板共用同一实例）----
(register-command! "stats" (lambda (text) (stats->text text)))
(register-command! "normalize" (lambda (text) (normalize-text text)))
(register-command! "engine-version" (lambda () "{version}"))

;; ---- 状态迁移（schema 1：草稿与摘要跨代保留）----
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
    ExtensionPackage::from_parts(manifest, sources).expect("示例包构造")
}
