//! 外部可移植模块工作台；独立于 Scheme text_workbench 示例。
//!
//! cargo run --locked --offline --features uix-dynamic,agent-control \
//!   --example module_workbench -- --module-source /明确授权的合成包目录
//!
//! 两侧分别显式装载 main.uix，替换前由操作者更新同一授权目录的源码。
//! 不扫描、不监听、不自动重试提交；领域文档仅存于本进程内存。
//! 每侧一个有界管理线程和模块 worker；共享领域服务有一个异步读取线程。
//! 正常退出和 --quit-after 均等待真实关闭，不用 process::exit 冒充回收。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uix_app::app::agent_workspace::AgentWorkspace;
use uix_app::app::modules::Effect as ModuleEffect;
use uix_app::app::modules::*;
use uix_app::prelude::*;

const WAIT: Duration = Duration::from_secs(3);
const TEXT_LIMIT: usize = 16 * 1024;
type Wake = Arc<dyn Fn() + Send + Sync>;
type Gate = Arc<Mutex<bool>>;

fn log(event: &str, detail: serde_json::Value) {
    println!(
        "{}",
        serde_json::json!({"event":event,"pid":std::process::id(),"detail":detail})
    );
}
fn failure(kind: ErrorKind, message: &str) -> RuntimeError {
    RuntimeError::new(kind, message)
}
fn record(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Record(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn document_type() -> Type {
    Type::Record(BTreeMap::from([
        ("content".into(), Type::String),
        ("version".into(), Type::Int),
    ]))
}
fn read_signature() -> Signature {
    Signature {
        name: "documents_read".into(),
        parameters: vec![],
        returns: document_type(),
        effect: ModuleEffect::Query,
        asynchronous: true,
    }
}
fn commit_signature() -> Signature {
    Signature {
        name: "documents_commit".into(),
        parameters: vec![
            ("expected".into(), Type::Int),
            ("content".into(), Type::String),
        ],
        returns: Type::Record(BTreeMap::from([
            ("accepted".into(), Type::Bool),
            ("version".into(), Type::Int),
            ("status".into(), Type::String),
        ])),
        effect: ModuleEffect::Command,
        asynchronous: false,
    }
}

struct Document {
    content: String,
    version: i64,
}
impl Document {
    fn snapshot(&self) -> Value {
        record([
            ("content", Value::String(self.content.clone())),
            ("version", Value::Int(self.version)),
        ])
    }
}
enum ReadMessage {
    Read(AsyncCall),
    Stop,
}
struct Domain {
    document: Arc<Mutex<Document>>,
    reads: mpsc::SyncSender<ReadMessage>,
    done: mpsc::Receiver<()>,
    thread: JoinHandle<()>,
}
impl Domain {
    fn new() -> std::io::Result<Self> {
        let document = Arc::new(Mutex::new(Document {
            content: "  Shared   synthetic document  ".into(),
            version: 1,
        }));
        let (reads, receive) = mpsc::sync_channel(8);
        let (done_send, done) = mpsc::sync_channel(1);
        let store = document.clone();
        let thread = thread::Builder::new().name("workbench-domain".into()).spawn(move || {
            while let Ok(ReadMessage::Read(call)) = receive.recv() {
                // 合成服务的固定 50ms 工作延迟；实际在另一个执行线程完成，模块已挂起。
                log("read_started", serde_json::json!({"instance":call.operation.instance,"request":call.operation.request}));
                thread::sleep(Duration::from_millis(50));
                let result = if call.cancellation.is_cancelled() {
                    Err(failure(ErrorKind::Cancelled, "合成读取已取消"))
                } else { Ok(store.lock().expect("领域存储锁").snapshot()) };
                if let Err(error) = call.completion.complete(result) {
                    log("read_completion_rejected", serde_json::json!({"error":error.to_string()}));
                } else {
                    log("read_completed", serde_json::json!({"request":call.operation.request}));
                }
            }
            let _ = done_send.send(());
        })?;
        Ok(Self {
            document,
            reads,
            done,
            thread,
        })
    }
    fn close(self) -> Result<(), String> {
        self.reads
            .send(ReadMessage::Stop)
            .map_err(|_| "领域读取服务提前退出")?;
        self.done
            .recv_timeout(WAIT)
            .map_err(|_| "领域读取服务关闭超时")?;
        self.thread.join().map_err(|_| "领域读取服务 panic")?;
        log("domain_closed", serde_json::json!({"joined":true}));
        Ok(())
    }
}

fn ports(
    document: Arc<Mutex<Document>>,
    reads: mpsc::SyncSender<ReadMessage>,
    gate: Gate,
    side: &'static str,
) -> (HostPorts, AsyncHostPorts) {
    let read = AsyncHostPort {
        signature: read_signature(),
        start: Arc::new(move |call| {
            reads
                .try_send(ReadMessage::Read(call))
                .map_err(|error| match error {
                    mpsc::TrySendError::Full(_) => failure(ErrorKind::Quota, "合成读取队列已满"),
                    mpsc::TrySendError::Disconnected(_) => {
                        failure(ErrorKind::Closed, "合成读取已关闭")
                    }
                })
        }),
    };
    let commit = HostPort {
        signature: commit_signature(),
        callback: Arc::new(move |arguments| {
            let [expected, text] = arguments else {
                return Err(failure(ErrorKind::Argument, "提交参数数量错误"));
            };
            let expected = expected.as_int()?;
            let text = text.as_str()?;
            if text.len() > TEXT_LIMIT {
                return Err(failure(ErrorKind::Quota, "合成文档超过 16 KiB"));
            }
            // 授权锁覆盖检查和提交；撤权返回后不再有旧授权的在途写入。
            let allowed = gate.lock().expect("授权锁");
            let mut doc = document.lock().expect("领域存储锁");
            let accepted = *allowed && expected == doc.version;
            let status = if !*allowed {
                "拒绝：写权已撤销".to_owned()
            } else if expected != doc.version {
                format!("版本冲突：预期 v{expected}，实际 v{}；未覆盖", doc.version)
            } else {
                doc.version = doc
                    .version
                    .checked_add(1)
                    .ok_or_else(|| failure(ErrorKind::Quota, "版本已耗尽"))?;
                doc.content = text.into();
                format!("已提交 v{}", doc.version)
            };
            log(
                "domain_commit",
                serde_json::json!({"side":side,"accepted":accepted,"expected":expected,"version":doc.version,"content":doc.content,"status":status}),
            );
            Ok(record([
                ("accepted", Value::Bool(accepted)),
                ("version", Value::Int(doc.version)),
                ("status", Value::String(status)),
            ]))
        }),
    };
    (
        BTreeMap::from([("documents_commit".into(), commit)]),
        BTreeMap::from([("documents_read".into(), read)]),
    )
}

#[derive(Clone, Copy, Debug)]
enum Manage {
    Load,
    Replace,
    Status,
    Revoke,
    Deactivate,
    Stale,
    Close,
}
#[derive(Clone)]
struct Side {
    name: &'static str,
    commands: mpsc::SyncSender<Manage>,
    status: State<String>,
    changed: State<u64>,
    view: Arc<Mutex<Option<ModuleView>>>,
    wake: Arc<Mutex<Option<Wake>>>,
}
impl Side {
    fn notify(&self) {
        self.changed.update(|value| *value += 1);
        let wake = self.wake.lock().expect("所属面唤醒锁").clone();
        if let Some(wake) = wake {
            wake();
        }
    }
    fn report(&self, text: String) {
        self.status.set(text.clone());
        log(
            "management",
            serde_json::json!({"side":self.name,"status":text}),
        );
        self.notify();
    }
    fn submit(&self, action: Manage) {
        if let Err(error) = self.commands.try_send(action) {
            self.report(format!("管理操作未接受：{error}"));
        }
    }
    fn root(&self) -> ViewNode {
        self.changed.get();
        let mount = self.view.lock().expect("挂载锁").clone();
        let facts = mount
            .as_ref()
            .map(|view| {
                let s = view.handle().snapshot();
                format!(
                    "模块 {} · 代 {} · 修订 {} · 在途 {}",
                    s.name,
                    s.generation,
                    s.revision,
                    s.pending_sequences.len()
                )
            })
            .unwrap_or_else(|| "未装载模块".into());
        let panel = match mount {
            Some(view) => view
                .project()
                .unwrap_or_else(|error| label(error.to_string())),
            None => label("显式装载后显示外部模块界面"),
        };
        let button_for = |title: &str, id: &str, action| {
            let side = self.clone();
            button(title)
                .on_click_fn(move || side.submit(action))
                .automation_id(id)
                .build()
        };
        column_fit((
            label(format!("{} · 可移植模块工作台", self.name)).font_size(20.0),
            label("仅合成数据 · 前后台私有草稿 · 不自动重试提交").font_size(13.0),
            row((
                button_for("装载", "manage-load", Manage::Load),
                button_for("替换源码", "manage-replace", Manage::Replace),
                button_for("状态", "manage-status-button", Manage::Status),
            ))
            .gap(8.0),
            row((
                button_for("撤权", "manage-revoke", Manage::Revoke),
                button_for("停用", "manage-deactivate", Manage::Deactivate),
                button_for("提交旧代动作", "manage-stale", Manage::Stale),
            ))
            .gap(8.0),
            label(self.status.get()).automation_id("manage-status"),
            label(facts).automation_id("module-facts"),
            panel,
        ))
        .padding(20.0)
        .gap(12.0)
    }
}
struct SideOwner {
    side: Side,
    done: mpsc::Receiver<Result<(), String>>,
    thread: JoinHandle<()>,
}
impl SideOwner {
    fn new(name: &'static str, source: PathBuf, domain: &Domain) -> std::io::Result<Self> {
        let (commands, receive) = mpsc::sync_channel(4);
        let (done_send, done) = mpsc::sync_channel(1);
        let side = Side {
            name,
            commands,
            status: State::new("未装载；来源由启动参数显式授权".into()),
            changed: State::new(0),
            view: Arc::new(Mutex::new(None)),
            wake: Arc::new(Mutex::new(None)),
        };
        let manager = side.clone();
        let document = domain.document.clone();
        let reads = domain.reads.clone();
        let thread = thread::Builder::new().name(format!("workbench-{name}")).spawn(move || {
            let gate = Arc::new(Mutex::new(false));
            let (sync, asynchronous) = ports(document.clone(), reads, gate.clone(), name);
            let mut worker: Option<ModuleWorker> = None;
            let mut saved_event: Option<(ModuleHandle, u64)> = None;
            let mut close_result = Ok(());
            while let Ok(action) = receive.recv() {
                let started = Instant::now();
                let result: Result<String, String> = (|| match action {
                    Manage::Load | Manage::Replace => {
                        if matches!(action, Manage::Load) && worker.is_some() { return Err("已有实例；请替换或先停用".into()); }
                        if matches!(action, Manage::Replace) && worker.is_none() { return Err("未装载实例".into()); }
                        let path = source.join("main.uix").canonicalize().map_err(|e| e.to_string())?;
                        if path.parent() != Some(source.as_path()) { return Err("main.uix 越过授权包目录".into()); }
                        let module = load_module_file(&path).map_err(|e| format!("{} {}:{} {}（旧代保留）", e.code, e.source_name, e.line, e.message))?;
                        let version = module.version.clone();
                        if let Some(owner) = &worker {
                            let handle = owner.handle();
                            let generation = handle.snapshot().generation;
                            handle.replace(module, generation).and_then(|r| r.wait(WAIT)).map_err(|e| e.to_string())?;
                        } else {
                            let instance = Instance::new_with_async_ports(module, sync.clone(), asynchronous.clone(), Limits::default()).map_err(|e| e.to_string())?;
                            let signal = manager.clone();
                            let (owner, view) = ModuleView::spawn(instance, 8, move || signal.notify()).map_err(|e| e.to_string())?;
                            saved_event = Some((owner.handle(), owner.handle().snapshot().generation));
                            *gate.lock().expect("授权锁") = true;
                            *manager.view.lock().expect("挂载锁") = Some(view);
                            worker = Some(owner);
                        }
                        Ok(format!("{action:?} 成功：外部版本 {version}；代 {}", worker.as_ref().unwrap().handle().snapshot().generation))
                    }
                    Manage::Status => {
                        let allowed = gate.lock().expect("授权锁");
                        let doc = document.lock().expect("领域存储锁");
                        Ok(format!("写权 {} · 合成文档 v{} · {}", *allowed, doc.version, doc.content))
                    }
                    Manage::Revoke => {
                        *gate.lock().expect("授权锁") = false;
                        if let Some(owner) = &worker { owner.handle().revoke(); }
                        Ok("本侧已撤权；新调用拒绝，停用后可显式重新装载".into())
                    }
                    Manage::Stale => {
                        let (handle, generation) = saved_event.as_ref().ok_or("没有已保存的旧动作")?;
                        if handle.snapshot().generation == *generation && handle.snapshot().active {
                            return Err("尚未替换；没有旧代动作，本次未发送提交".into());
                        }
                        match handle.event("commit", *generation, vec![]).and_then(|r| r.wait(WAIT)) {
                            Err(error) => Ok(format!("旧代动作已拒绝：{:?} {}", error.kind, error.message)),
                            Ok(value) => Ok(format!("动作仍属当前代：{value:?}；请先替换后演示旧代拒绝")),
                        }
                    }
                    Manage::Deactivate | Manage::Close => {
                        *gate.lock().expect("授权锁") = false;
                        *manager.view.lock().expect("挂载锁") = None;
                        if let Some(owner) = worker.take() { owner.close(WAIT).map_err(|e| e.to_string())?; }
                        saved_event = None;
                        Ok("本侧 worker 已关闭并 join；挂载已清除".into())
                    }
                })();
                log("management_timing", serde_json::json!({"side":name,"action":format!("{action:?}"),"micros":started.elapsed().as_micros(),"ok":result.is_ok()}));
                manager.report(match &result { Ok(value) => value.clone(), Err(error) => format!("{action:?} 失败：{error}") });
                if matches!(action, Manage::Close) { close_result = result.map(|_| ()); break; }
            }
            let _ = done_send.send(close_result);
        })?;
        Ok(Self { side, done, thread })
    }
    fn close(self) -> Result<(), String> {
        self.side
            .commands
            .send(Manage::Close)
            .map_err(|_| "管理线程提前退出")?;
        self.done
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| "管理线程关闭超时")??;
        self.thread.join().map_err(|_| "管理线程 panic")?;
        log(
            "side_closed",
            serde_json::json!({"side":self.side.name,"joined":true}),
        );
        Ok(())
    }
}

fn option(name: &str) -> Option<String> {
    let args: Vec<_> = std::env::args().collect();
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(option("--module-source").ok_or("需要 --module-source 合成包目录")?)
        .canonicalize()?;
    if !source.is_dir() {
        return Err("授权来源不是目录".into());
    }
    let probe = option("--native-probe").map(PathBuf::from);
    #[cfg(not(all(
        target_os = "linux",
        feature = "test-harness",
        feature = "image-codecs"
    )))]
    if probe.is_some() {
        return Err("原生诊断需要 Linux 与 test-harness,image-codecs".into());
    }
    let quit_after = option("--quit-after")
        .map(|n| n.parse::<u64>())
        .transpose()?;
    let domain = Domain::new()?;
    let front = SideOwner::new("front", source.clone(), &domain)?;
    let back = SideOwner::new("back", source.clone(), &domain)?;
    let back_root = back.side.clone();
    let workspace = AgentWorkspace::new(800, 660, move || back_root.root())
        .title("UIX Portable Module Workbench")
        .spawn()?;
    let poster = workspace.poster();
    *back.side.wake.lock().expect("后台唤醒锁") = Some(Arc::new(move || poster.wake()));
    let front_root = front.side.clone();
    // 仅显式截图时失效独立诊断画布，保证产生待回读帧；不重建业务根或触碰草稿/焦点。
    let native_diagnostic = probe.is_some();
    let capture_tick = State::new(0u64);
    let root_capture_tick = capture_tick.clone();
    let front_start = front.side.clone();
    let stop = Arc::new(AtomicBool::new(false));
    #[cfg(all(
        target_os = "linux",
        feature = "test-harness",
        feature = "image-codecs"
    ))]
    let stop_start = stop.clone();
    let diagnostic = Arc::new(Mutex::new(None::<JoinHandle<Result<(), String>>>));
    #[cfg(all(
        target_os = "linux",
        feature = "test-harness",
        feature = "image-codecs"
    ))]
    let diagnostic_start = diagnostic.clone();
    log(
        "host_ready",
        serde_json::json!({"source":source,"preloaded":false,"document_version":1}),
    );
    let code = App::new()
        .title("UIX Portable Workbench — front")
        .size(800, 660)
        .root(move || {
            column_fit((
                window_control_named(
                    WindowControl::Close,
                    "关闭工作台",
                    if native_diagnostic {
                        let tick = root_capture_tick.clone();
                        canvas(280.0, 28.0, move |bounds, ctx| {
                            let number = tick.get();
                            ctx.fill_rect(bounds, Color::WHITE, None);
                            ctx.draw_text(
                                &format!("关闭工作台 · 原生诊断帧 {number}"),
                                Point::new(bounds.x + 4.0, bounds.y + 7.0),
                                Color::BLACK,
                                14.0,
                            );
                        })
                    } else {
                        label("关闭工作台")
                    },
                )
                .height(28.0)
                .automation_id("host-close"),
                front_root.root(),
            ))
        })
        .on_start(move |handle| {
            let wake_handle = handle.clone();
            *front_start.wake.lock().expect("前台唤醒锁") =
                Some(Arc::new(move || wake_handle.post_to_ui(|| {})));
            // 普通运行没有固定 tick；只有显式截止时间才安排一次原生关闭动作。
            if let Some(seconds) = quit_after {
                let close_handle = handle.clone();
                handle
                    .run_after(Duration::from_secs(seconds), move || {
                        if let Err(error) = close_handle
                            .perform_automation_action("host-close", SemanticAction::Invoke)
                        {
                            eprintln!("定时关闭请求失败：{error}");
                        }
                    })
                    .detach();
            }
            #[cfg(all(
                target_os = "linux",
                feature = "test-harness",
                feature = "image-codecs"
            ))]
            if let Some(directory) = probe {
                let flag = stop_start.clone();
                let tick = capture_tick.clone();
                *diagnostic_start.lock().expect("诊断线程锁") = Some(thread::spawn(move || {
                    native_probe(handle, tick, directory, flag)
                }));
            }
        })
        .run();
    stop.store(true, Ordering::Release);
    let mut errors = Vec::new();
    if let Some(thread) = diagnostic.lock().expect("诊断线程锁").take() {
        match thread.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(e),
            Err(_) => errors.push("诊断线程 panic".into()),
        }
    }
    // 先关闭后台面，阻止继续接单；随后回收两侧模块/管理线程和领域服务。
    if let Err(error) = workspace.close() {
        errors.push(error.to_string());
    } else {
        log("workspace_closed", serde_json::json!({"joined":true}));
    }
    for result in [front.close(), back.close(), domain.close()] {
        if let Err(error) = result {
            errors.push(error);
        }
    }
    log(
        "host_closed",
        serde_json::json!({"native_exit":code,"errors":errors}),
    );
    if code != 0 || !errors.is_empty() {
        return Err("工作台关闭存在失败，见 host_closed".into());
    }
    Ok(())
}

// 仅显式诊断模式接受 stdin 指令；独立于 Agent 后台根，绝不把前台句柄交给后台。
#[cfg(all(
    target_os = "linux",
    feature = "test-harness",
    feature = "image-codecs"
))]
fn native_probe(
    handle: AppHandle,
    tick: State<u64>,
    directory: PathBuf,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut pending = Vec::new();
    let mut bytes = [0u8; 4096];
    while !stop.load(Ordering::Acquire) {
        let mut poll = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
        if ready < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if ready == 0 {
            continue;
        }
        let read = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
        if read < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&bytes[..read as usize]);
        if pending.len() > 65536 {
            return Err("原生诊断指令超过 64 KiB".into());
        }
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let command: serde_json::Value =
                serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            let result = (|| -> Result<(), String> {
                if command.get("quit") == Some(&serde_json::Value::Bool(true)) {
                    handle
                        .perform_automation_action("host-close", SemanticAction::Invoke)
                        .map_err(|e| e.to_string())?
                        .recv_timeout(WAIT)
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?;
                    stop.store(true, Ordering::Release);
                    return Ok(());
                }
                if let Some(name) = command.get("capture").and_then(|v| v.as_str()) {
                    if name.is_empty()
                        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        return Err("无效截图名称".into());
                    }
                    let (send, receive) = mpsc::sync_channel(1);
                    let capture_handle = handle.clone();
                    let tick = tick.clone();
                    handle.post_to_ui(move || {
                        tick.update(|n| *n += 1);
                        let ticket = capture_handle.request_surface_readback_for_test();
                        let _ = send.send(ticket);
                    });
                    let surface = receive
                        .recv_timeout(WAIT)
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?
                        .recv_timeout(Duration::from_secs(10))
                        .map_err(|e| e.to_string())?;
                    let rgba: Vec<u8> = surface
                        .pixels
                        .into_iter()
                        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8])
                        .collect();
                    image::save_buffer(
                        directory.join(format!("{name}.png")),
                        &rgba,
                        surface.width as u32,
                        surface.height as u32,
                        image::ColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())?;
                    let (send, receive) = mpsc::sync_channel(1);
                    handle.post_to_ui(move || {
                        let _ = send.send(());
                    });
                    receive.recv_timeout(WAIT).map_err(|e| e.to_string())?;
                    let export = PathBuf::from(
                        std::env::var_os("UIX_AUTOMATION_DIR")
                            .ok_or("诊断需 UIX_AUTOMATION_DIR")?,
                    )
                    .join(format!(
                        "uix-{}-window-{}.json",
                        std::process::id(),
                        handle.window_id().raw()
                    ));
                    std::fs::copy(export, directory.join(format!("{name}.snapshot.json")))
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                let target = command
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or("诊断动作缺少 target")?;
                let action = match command.get("action").and_then(|v| v.as_str()) {
                    Some("invoke") => SemanticAction::Invoke,
                    Some("focus") => SemanticAction::Focus,
                    Some("set_value") => SemanticAction::SetValue(
                        command
                            .get("value")
                            .and_then(|v| v.as_str())
                            .ok_or("缺少 value")?
                            .into(),
                    ),
                    _ => return Err("诊断只支持 invoke/focus/set_value/capture/quit".into()),
                };
                handle
                    .perform_automation_action(target, action)
                    .map_err(|e| e.to_string())?
                    .recv_timeout(WAIT)
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                Ok(())
            })();
            log(
                "native_probe",
                serde_json::json!({"command":command,"ok":result.is_ok(),"error":result.err()}),
            );
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("module_workbench: {error}");
        std::process::exit(1);
    }
}
