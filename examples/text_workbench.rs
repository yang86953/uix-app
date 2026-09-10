//! 文本处理工作台：外部扩展包交付 × 显式生命周期管理 × Agent 独立后台操作面。
//!
//! 运行：
//! ```sh
//! cargo run --release --features extensions,agent-control --example text_workbench \
//!   -- --extension-source extensions/text-bench/v1 [--quit-after 秒]
//! ```
//!
//! - 宿主启动时不装载任何扩展：外部扩展包（`manifest.scm` + `.scm` 源文件）
//!   只在显式管理操作时从 `--extension-source` 指定的目录读取并冻结为
//!   不可变快照；宿主不扫描、不监听目录，来源由本参数显式授权。
//! - 前台真窗与后台操作面各自拥有扩展 worker、投影器、草稿与交互状态，
//!   只共享领域端口（查询 / 受控提交）；两侧原生管理按钮执行同一套
//!   装载 / 替换 / 状态 / 撤权 / 停用操作，结果按侧分别报告到管理状态。
//!   AI 经 `scripts/agent_client.py` 在后台操作面驱动同一受控入口，不控制
//!   前台按钮；面板内的业务操作（读取 → 输入 → 处理 → 提交）由扩展自身
//!   实现，合法提交按版本与授权规则对两侧可见。
//! - 收尾责任统一：正常关闭与 `--quit-after` 复用同一收尾函数，逐步取得
//!   扩展 worker 与后台操作面的真实终态；失败与超时如实诊断并以非零码
//!   退出，不把强制进程退出冒充资源已优雅释放。
//!
//! 边界见 docs/使用/能力与边界/软件动态扩展.md 与 Agent后台操作面.md。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uix_app::app::agent_workspace::{AgentWorkspace, AgentWorkspaceHandle};
use uix_app::app::extensions::{
    ExtensionHost, ExtensionPackage, ExtensionPort, ExtensionUiHandle, ExtensionValue, UiNode,
    UiProjector, UiUpdate,
};
use uix_app::prelude::*;

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

// ---------- 侧装配：worker 句柄、投影器、声明与修订 ----------

/// 一侧扩展装配（前台 / 后台同构；差异只在修订推进的唤醒通道）。
struct SideAssembly {
    name: &'static str,
    handle_slot: Arc<Mutex<Option<ExtensionUiHandle>>>,
    generation: Arc<Mutex<u64>>,
    current: Arc<Mutex<Option<UiNode>>>,
    projector: Arc<Mutex<Option<UiProjector>>>,
    /// 在该侧 owner 推进一次修订并唤醒重建（前台 post_to_ui；后台 wake）。
    notify: Arc<dyn Fn() + Send + Sync>,
}

impl SideAssembly {
    /// 激活后安装投影器并补帧（初始声明可能早于投影器安装）。
    fn install_projector(&self, handle: &ExtensionUiHandle, extension_id: &str, generation: u64) {
        *self.projector.lock().expect("投影器锁") = Some(UiProjector::new(
            extension_id,
            generation,
            handle.event_sender(),
        ));
        (self.notify)();
    }

    /// 停用终态后清空挂载子树（应用负责子树退出）。
    fn clear_mount(&self) {
        *self.current.lock().expect("声明锁") = None;
        (self.notify)();
    }

    fn report(&self, text: String) -> String {
        format!("{}：{text}", self.name)
    }
}

// ---------- 显式管理操作 ----------

#[derive(Clone, Copy, PartialEq)]
enum ManageAction {
    /// 读取授权来源并装载（要求该侧尚无活动实例）。
    Load,
    /// 重读授权来源并热替换（预期当前活动代；失败侧旧代保留）。
    Replace,
    /// 查看两侧活动实例状态（handle.list 真实回读）。
    Status,
    /// 撤权两侧（新调用与事件拒绝，实例保留至停用）。
    Revoke,
    /// 停用两侧（排空释放并清空挂载子树）。
    Deactivate,
}

impl ManageAction {
    fn label(self) -> &'static str {
        match self {
            ManageAction::Load => "装载",
            ManageAction::Replace => "替换",
            ManageAction::Status => "状态",
            ManageAction::Revoke => "撤权",
            ManageAction::Deactivate => "停用",
        }
    }
}

/// 管理操作上下文：授权来源、共享管理状态与两侧装配。
#[derive(Clone)]
struct ManageContext {
    source: Option<PathBuf>,
    status: State<String>,
    user: Arc<SideAssembly>,
    agent: Arc<SideAssembly>,
}

/// 读取授权来源并冻结（阶段 1：包检查）。失败写入管理状态并返回 `None`。
fn read_authorized_source(context: &ManageContext) -> Option<(ExtensionPackage, PathBuf)> {
    let Some(path) = context.source.clone() else {
        context
            .status
            .set("装载/替换失败：未配置 --extension-source，宿主未授权任何包来源".to_string());
        return None;
    };
    match ExtensionPackage::read_from_directory(&path) {
        Ok(package) => Some((package, path)),
        Err(error) => {
            context
                .status
                .set(format!("包检查失败（来源 {}）：{error}", path.display()));
            None
        }
    }
}

/// 在独立线程执行管理操作（prepare / activate / replace 阻塞求值，
/// 不占用任何 UI 线程）；结果按侧分别写入管理状态。
fn run_manage(action: ManageAction, context: ManageContext) {
    thread::spawn(move || match action {
        ManageAction::Status => {
            let mut reports = Vec::new();
            for side in [context.user.as_ref(), context.agent.as_ref()] {
                reports.push(match side_status(&side.handle_slot) {
                    Ok(text) => side.report(text),
                    Err(error) => side.report(format!("状态读取失败：{error}")),
                });
            }
            context
                .status
                .set(format!("状态回读 · {}", reports.join(" · ")));
        }
        ManageAction::Revoke => {
            let mut reports = Vec::new();
            for side in [context.user.as_ref(), context.agent.as_ref()] {
                let Some(handle) = side.handle_slot.lock().expect("扩展句柄锁").clone() else {
                    reports.push(side.report("worker 不存在".to_string()));
                    continue;
                };
                let outcome = match handle.list() {
                    Ok(instances) if instances.is_empty() => Ok("无活动实例".to_string()),
                    _ => handle
                        .revoke("text-bench")
                        .map(|_| "已撤权（新调用与面板事件被拒，实例保留至停用）".to_string()),
                };
                reports.push(match outcome {
                    Ok(text) => side.report(text),
                    Err(error) => side.report(format!("撤权失败：{error}")),
                });
            }
            context
                .status
                .set(format!("撤权 · {}", reports.join(" · ")));
        }
        ManageAction::Deactivate => {
            let mut reports = Vec::new();
            for side in [context.user.as_ref(), context.agent.as_ref()] {
                let Some(handle) = side.handle_slot.lock().expect("扩展句柄锁").clone() else {
                    reports.push(side.report("worker 不存在".to_string()));
                    continue;
                };
                match handle.deactivate("text-bench") {
                    Ok(receipt) => {
                        // 终态真实返回后按合同清空该侧挂载子树。
                        side.clear_mount();
                        reports.push(side.report(format!(
                            "已停用（generation {}），挂载子树已退出",
                            receipt.generation
                        )));
                    }
                    Err(error) => reports.push(side.report(format!("停用失败：{error}"))),
                }
            }
            context
                .status
                .set(format!("停用 · {}", reports.join(" · ")));
        }
        ManageAction::Load | ManageAction::Replace => {
            let action_label = action.label();
            // 阶段 1：包检查（读取并冻结为不可变快照）。
            let Some((package, path)) = read_authorized_source(&context) else {
                return;
            };
            let header = match package.manifest() {
                Ok(manifest) => format!(
                    "{action_label} · 包检查通过：{} v{}（{} 文件，来源 {}）",
                    manifest.id,
                    manifest.version,
                    package.source_paths().count() + 1,
                    path.display()
                ),
                Err(error) => {
                    context.status.set(format!(
                        "{action_label} · 清单解析失败（来源 {}）：{error}",
                        path.display()
                    ));
                    return;
                }
            };
            // 阶段 2：候选准备；阶段 3：活动代切换（装载 / 热替换）。
            let mut reports = Vec::new();
            for side in [context.user.as_ref(), context.agent.as_ref()] {
                let Some(handle) = side.handle_slot.lock().expect("扩展句柄锁").clone() else {
                    reports.push(side.report("worker 不存在".to_string()));
                    continue;
                };
                let expected = *side.generation.lock().expect("扩展代际锁");
                let outcome = match handle.prepare(&package) {
                    Err(error) => Err(error),
                    Ok(candidate) => match action {
                        ManageAction::Load => handle.activate(candidate).map(|receipt| {
                            (receipt.extension_id, receipt.generation, receipt.commands)
                        }),
                        ManageAction::Replace => {
                            handle.replace(candidate, expected).map(|receipt| {
                                (receipt.extension_id, receipt.generation, receipt.commands)
                            })
                        }
                        _ => unreachable!("装载与替换之外的分支不携带候选"),
                    },
                };
                match outcome {
                    Ok((extension_id, generation, commands)) => {
                        *side.generation.lock().expect("扩展代际锁") = generation;
                        if action == ManageAction::Load {
                            side.install_projector(&handle, &extension_id, generation);
                        } else {
                            // 热替换：投影器沿用，代际由声明回执切换。
                            (side.notify)();
                        }
                        reports.push(side.report(format!(
                            "{} g{}（命令 {}），界面应用中",
                            if action == ManageAction::Load {
                                "已激活"
                            } else {
                                "已切换"
                            },
                            generation,
                            commands.join("/")
                        )));
                    }
                    Err(error) => {
                        let hint = if action == ManageAction::Replace {
                            "（旧代保留并继续服务）"
                        } else {
                            ""
                        };
                        reports.push(side.report(format!("{action_label}失败：{error}{hint}")));
                    }
                }
            }
            // 阶段结果按侧分别报告；呈现由后台视口 presented_revision 观察。
            context
                .status
                .set(format!("{header} · {}", reports.join(" · ")));
        }
    });
}

/// 单侧活动实例摘要（handle.list 真实回读）。
fn side_status(
    handle_slot: &Arc<Mutex<Option<ExtensionUiHandle>>>,
) -> Result<String, uix_app::app::extensions::ExtensionError> {
    let Some(handle) = handle_slot.lock().expect("扩展句柄锁").clone() else {
        return Ok("worker 不存在".to_string());
    };
    let instances = handle.list()?;
    if instances.is_empty() {
        return Ok("无活动实例".to_string());
    }
    Ok(instances
        .iter()
        .map(|instance| {
            format!(
                "{} v{} g{}{}（命令 {}）",
                instance.extension_id,
                instance.version,
                instance.generation,
                if instance.revoked { " 已撤权" } else { "" },
                instance.commands.join("/")
            )
        })
        .collect::<Vec<_>>()
        .join("；"))
}

/// 构建管理按钮（宿主原生 UI；点击在独立线程执行管理操作）。
/// `context_slot` 允许后台根在装配完成前先渲染按钮（点击时读取当前值）。
fn manage_button(
    text: &str,
    automation_id: &str,
    action: ManageAction,
    context_slot: &Arc<Mutex<Option<ManageContext>>>,
) -> ViewNode {
    button(text)
        .on_click(&State::new(false), {
            let context_slot = Arc::clone(context_slot);
            move |_: &State<bool>| {
                if let Some(context) = context_slot.lock().expect("管理上下文锁").clone() {
                    run_manage(action, context);
                }
            }
        })
        .build()
        .automation_id(automation_id)
}

/// 取 `--flag value` 形式的命令行参数值。
fn argument_value(flag: &str) -> Option<String> {
    let arguments: Vec<String> = std::env::args().collect();
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}

fn main() {
    let source: Option<PathBuf> = argument_value("--extension-source").map(PathBuf::from);
    let quit_after = argument_value("--quit-after").and_then(|value| value.parse::<u64>().ok());

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

    // ---- 共享管理状态（两个根读同一 State，操作结果两侧同显）----
    let mgmt_status =
        State::new("未装载扩展：等待显式装载（装载 / 替换 / 状态 / 撤权 / 停用）".to_string());
    // 后台根先于管理上下文装配：按钮经槽在点击时读取当前上下文。
    let manage_slot: Arc<Mutex<Option<ManageContext>>> = Arc::new(Mutex::new(None));

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
        let root_status = mgmt_status.clone();
        let root_manage_slot = Arc::clone(&manage_slot);
        let workspace = AgentWorkspace::new(800, 660, move || {
            root_revision.get();
            let status = root_status.get();
            let node = root_current.lock().expect("后台声明锁").clone();
            let mut projector = root_projector.lock().expect("后台投影器锁");
            let panel = match (node, projector.as_mut()) {
                (Some(node), Some(projector)) => projector.project("panel", &node),
                _ => column(Vec::<ViewNode>::new()),
            };
            // 宿主管理入口：原生 UI，不属于扩展能力；AI 经 Agent 通道驱动。
            let load = manage_button(
                "装载扩展",
                "manage-load",
                ManageAction::Load,
                &root_manage_slot,
            );
            let replace = manage_button(
                "热替换扩展",
                "manage-replace",
                ManageAction::Replace,
                &root_manage_slot,
            );
            let status_btn = manage_button(
                "扩展状态",
                "manage-status-btn",
                ManageAction::Status,
                &root_manage_slot,
            );
            let revoke = manage_button(
                "撤权两侧",
                "manage-revoke",
                ManageAction::Revoke,
                &root_manage_slot,
            );
            let deactivate = manage_button(
                "停用两侧",
                "manage-deactivate",
                ManageAction::Deactivate,
                &root_manage_slot,
            );
            column_fit((
                label("AI 独立后台操作面 · 文本处理").font_size(15.0),
                label(status.as_str())
                    .font_size(11.0)
                    .automation_id("manage-status"),
                row((load, replace, status_btn)).gap(8.0),
                row((revoke, deactivate)).gap(8.0),
                label("扩展面板（外部包装载后出现）").font_size(11.0),
                panel,
            ))
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
        .with_mount("panel")
        .with_port("documents-query", Arc::clone(&shared_query))
        .with_port("documents-commit", Arc::clone(&shared_commit))
        .with_ui_sink(Arc::new({
            let current = Arc::clone(&bg_current);
            let revision = bg_revision.clone();
            let projector = Arc::clone(&bg_projector);
            let poster = workspace.poster();
            move |update| match update {
                UiUpdate::Applied {
                    mut node,
                    generation,
                    ..
                } => {
                    if let Some(projector) = projector.lock().expect("后台投影器锁").as_mut()
                    {
                        projector.update_generation(generation);
                        // reset 是本次 Applied 的一次性指令：执行后消耗标记，
                        // 后续重投影不得再次覆盖本地编辑。
                        projector.consume_declaration_resets(&mut node);
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
    *bg_handle_slot.lock().expect("后台句柄锁") = Some(bg_handle);
    let agent_side = Arc::new(SideAssembly {
        name: "agent",
        handle_slot: Arc::clone(&bg_handle_slot),
        generation: Arc::clone(&bg_generation),
        current: Arc::clone(&bg_current),
        projector: Arc::clone(&bg_projector),
        notify: Arc::new({
            let revision = bg_revision.clone();
            let poster = workspace.poster();
            move || {
                revision.update(|value| *value += 1);
                poster.wake();
            }
        }),
    });

    println!(
        "[workbench] 宿主已启动：pid={}，二进制 {:?}",
        std::process::id(),
        std::env::current_exe().unwrap_or_default()
    );
    println!(
        "[workbench] 授权包来源：{}",
        source
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "（未配置 --extension-source）".to_string())
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
    // 前台窗口句柄槽：管理线程经 post_to_ui 把修订推进投回窗口 owner。
    let fg_app_slot: Arc<Mutex<Option<AppHandle>>> = Arc::new(Mutex::new(None));

    let fg_host = ExtensionHost::new()
        .with_mount("panel")
        .with_port("documents-query", shared_query)
        .with_port("documents-commit", shared_commit)
        .with_ui_sink(Arc::new(move |update| {
            let _ = fg_updates_tx.send(update);
        }));
    let fg_handle = fg_host.spawn_worker();
    *fg_handle_slot.lock().expect("前台句柄锁") = Some(fg_handle);
    let user_side = Arc::new(SideAssembly {
        name: "user",
        handle_slot: Arc::clone(&fg_handle_slot),
        generation: Arc::clone(&fg_generation),
        current: Arc::clone(&fg_current),
        projector: Arc::clone(&fg_projector),
        notify: Arc::new({
            let revision = fg_revision.clone();
            let app_slot = Arc::clone(&fg_app_slot);
            move || {
                let revision = revision.clone();
                match app_slot.lock().expect("app 句柄锁").clone() {
                    Some(app) => app.post_to_ui(move || {
                        revision.update(|value| *value += 1);
                    }),
                    // 窗口尚未启动时无消费者，直接推进无害。
                    None => revision.update(|value| *value += 1),
                }
            }
        }),
    });

    *manage_slot.lock().expect("管理上下文锁") = Some(ManageContext {
        source: source.clone(),
        status: mgmt_status.clone(),
        user: Arc::clone(&user_side),
        agent: Arc::clone(&agent_side),
    });

    let app = App::new()
        .title("UIX 文本处理工作台")
        .size(620, 720)
        .on_start({
            let fg_current = Arc::clone(&fg_current);
            let fg_revision = fg_revision.clone();
            let fg_projector = Arc::clone(&fg_projector);
            let app_capture = Arc::clone(&fg_app_slot);
            let doc_summary = doc_summary.clone();
            move |app_handle| {
                *app_capture.lock().expect("app 句柄锁") = Some(app_handle.clone());
                // 声明交付桥：扩展线程 sink → 通道 → 本线程 → 窗口 owner
                // thread 应用（UI 线程不执行任何 Lisp）。
                let deliver = {
                    let current = Arc::clone(&fg_current);
                    let revision = fg_revision.clone();
                    let projector = Arc::clone(&fg_projector);
                    let app_handle = app_handle.clone();
                    move |update: UiUpdate| match update {
                        UiUpdate::Applied {
                            mut node,
                            generation,
                            ..
                        } => {
                            let current = Arc::clone(&current);
                            let revision = revision.clone();
                            let projector = Arc::clone(&projector);
                            app_handle.post_to_ui(move || {
                                if let Some(projector) =
                                    projector.lock().expect("前台投影器锁").as_mut()
                                {
                                    projector.update_generation(generation);
                                    // 同后台：reset 在 Applied 处一次性执行并消耗。
                                    projector.consume_declaration_resets(&mut node);
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
                            summary_state.set(format!(
                                "共享文档 {key} @v{version}（后台或前台提交后更新）"
                            ));
                        });
                    }
                });
            }
        })
        .root({
            let root_revision = fg_revision.clone();
            let root_current = Arc::clone(&fg_current);
            let root_projector = Arc::clone(&fg_projector);
            let root_summary = doc_summary.clone();
            let root_status = mgmt_status.clone();
            let root_manage_slot = Arc::clone(&manage_slot);
            move || {
                root_revision.get();
                let summary = root_summary.get();
                let status = root_status.get();
                let node = root_current.lock().expect("前台声明锁").clone();
                let mut projector = root_projector.lock().expect("前台投影器锁");
                let panel = match (node, projector.as_mut()) {
                    (Some(node), Some(projector)) => projector.project("panel", &node),
                    _ => column(Vec::<ViewNode>::new()),
                };
                // 宿主管理区：原生 UI，每次重建时构建（ViewNode 不可克隆）。
                let load = manage_button(
                    "装载扩展",
                    "manage-load",
                    ManageAction::Load,
                    &root_manage_slot,
                );
                let replace = manage_button(
                    "热替换扩展",
                    "manage-replace",
                    ManageAction::Replace,
                    &root_manage_slot,
                );
                let status_btn = manage_button(
                    "扩展状态",
                    "manage-status-btn",
                    ManageAction::Status,
                    &root_manage_slot,
                );
                let revoke = manage_button(
                    "撤权两侧",
                    "manage-revoke",
                    ManageAction::Revoke,
                    &root_manage_slot,
                );
                let deactivate = manage_button(
                    "停用两侧",
                    "manage-deactivate",
                    ManageAction::Deactivate,
                    &root_manage_slot,
                );
                column_fit((
                    label("用户前台 · 文本处理工作台").font_size(15.0),
                    label(summary.as_str())
                        .font_size(12.0)
                        .automation_id("doc-summary"),
                    label(status.as_str())
                        .font_size(11.0)
                        .automation_id("manage-status"),
                    row((load, replace, status_btn)).gap(8.0),
                    row((revoke, deactivate)).gap(8.0),
                    label("扩展面板（外部包装载后出现）").font_size(11.0),
                    panel,
                ))
            }
        });

    // 关闭权共享槽：定时退出与主线程收尾复用同一收尾责任（先到先得）。
    let workspace_slot: Arc<Mutex<Option<AgentWorkspaceHandle>>> =
        Arc::new(Mutex::new(Some(workspace)));
    // 自动退出：到点先执行统一收尾（真实排空 worker、回收操作面并记录
    // 终态），再结束进程；前台窗口随进程退出，收尾结果决定退出码。
    if let Some(seconds) = quit_after {
        let fg_slot = Arc::clone(&fg_handle_slot);
        let bg_slot = Arc::clone(&bg_handle_slot);
        let quit_workspace_slot = Arc::clone(&workspace_slot);
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_secs(seconds));
            eprintln!("[workbench] --quit-after 到期，执行统一收尾");
            let code = shutdown_all(&fg_slot, &bg_slot, &quit_workspace_slot);
            std::process::exit(code);
        });
    }

    let exit = app.run();
    // 正常关闭：与自动退出共用同一收尾函数；逐步记录真实终态。
    let close_code = shutdown_all(&fg_handle_slot, &bg_handle_slot, &workspace_slot);
    let final_code = if exit != 0 { exit } else { close_code };
    if close_code != 0 {
        eprintln!("[workbench] 收尾存在失败项，以退出码 {close_code} 如实报告");
    }
    std::process::exit(final_code);
}

/// 统一收尾：前台 worker → 后台 worker → 后台操作面，逐步取得真实终态。
///
/// 成功与失败都打印明确诊断；任何失败或超时返回非零码，不把强制进程
/// 退出冒充资源已优雅释放。前台窗口的生命周期归 `App::run`（随进程
/// 结束）；本函数只声称它真正释放的资源。
fn shutdown_all(
    fg_slot: &Arc<Mutex<Option<ExtensionUiHandle>>>,
    bg_slot: &Arc<Mutex<Option<ExtensionUiHandle>>>,
    workspace_slot: &Arc<Mutex<Option<AgentWorkspaceHandle>>>,
) -> i32 {
    let mut exit_code = 0;
    for (name, slot) in [("前台", fg_slot), ("后台", bg_slot)] {
        if let Some(handle) = slot.lock().expect("扩展句柄锁").take() {
            match handle.shutdown() {
                Ok(()) => {
                    println!("[workbench] {name}扩展 worker 已排空释放（实例与引擎堆回收）")
                }
                Err(error) => {
                    eprintln!("[workbench] {name}扩展 worker 关闭未完成：{error}");
                    exit_code = 1;
                }
            }
        }
    }
    if let Some(workspace) = workspace_slot.lock().expect("操作面锁").take() {
        match workspace.close() {
            Ok(()) => println!("[workbench] 后台操作面已回收（视口、端点与像素资源释放）"),
            Err(error) => {
                eprintln!("[workbench] 后台操作面关闭失败（含两秒等待超时）：{error:?}");
                exit_code = 1;
            }
        }
    }
    exit_code
}
