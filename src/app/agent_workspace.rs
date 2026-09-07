//! AI 独立后台操作面：拥有专用 UI 线程、组件树、内存视口和本机 Agent 端点。
//!
//! 根工厂必须创建操作面私有的导航、草稿及选择状态；仅显式捕获应用业务服务。
//! 不得捕获前台 AppHandle 或共享前台导航 State。框架隔离 UI 运行时，不沙箱化 Rust 回调。

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::app::agent::agent_policy::AgentPolicy;
use crate::app::agent_client::{AgentBridgeClient, AgentBridgeClientError};
use crate::app::queues::agent_command_queue::{AgentCommandRequest, AgentConfirmationRequest};
use crate::app::session_runtime::AppRuntime;
use crate::core::{Errc, Error, Result, WindowId};
use crate::draw::FontBundle;
use crate::platform::windowing::event::EventLoopWaker;
use crate::ui::{SemanticActionKind, Theme, ViewNode};

static WORKSPACE_ACTIVE: AtomicBool = AtomicBool::new(false);

// 同进程只发布一个后台操作面；租约由实际工作线程持有，关闭超时也不能重用身份。
struct WorkspaceLease;
impl Drop for WorkspaceLease {
    fn drop(&mut self) {
        WORKSPACE_ACTIVE.store(false, Ordering::Release);
    }
}

// 不吞工作线程 panic；栈展开仍须先关闭端点，最后才归还进程租约。
struct WorkspaceShutdown(AppRuntime);
impl Drop for WorkspaceShutdown {
    fn drop(&mut self) {
        self.0.shutdown_all();
    }
}

/// 尚未启动的后台操作面配置；不借用或复制可见窗口的组件树。
pub struct AgentWorkspace {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) root: Arc<dyn Fn() -> ViewNode + Send + Sync>,
    pub(crate) title: String,
    pub(crate) theme: Theme,
    pub(crate) fonts: Option<FontBundle>,
    pub(crate) policy: AgentPolicy,
    pub(crate) confirmation: Option<Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>>,
}

impl AgentWorkspace {
    /// 配置独立根工厂和 logical 视口（DPR 固定为 1）。尺寸在 spawn 前统一校验。
    pub fn new<F>(width: i32, height: i32, root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        Self {
            width,
            height,
            root: Arc::new(root),
            title: "UIX Agent Workspace".into(),
            theme: Theme::antd_light(),
            fonts: None,
            policy: AgentPolicy::default(),
            confirmation: None,
        }
    }
    /// 设置 Hub 枚举中的操作面显示名。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    /// 设置操作面独立的主题快照。
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
    /// 安装确定性字体；未指定时使用内建位图文本后备，不访问桌面字体服务。
    pub fn font_bundle(mut self, fonts: FontBundle) -> Self {
        self.fonts = Some(fonts);
        self
    }
    /// 拒绝全部写动作；快照与离屏截图仍可读取。
    pub fn read_only(mut self) -> Self {
        self.policy = self.policy.read_only();
        self
    }
    /// 保护稳定语义目标。
    pub fn protect(mut self, target: impl Into<String>) -> Self {
        self.policy = self.policy.protect(target);
        self
    }
    /// 禁止指定语义动作类别。
    pub fn deny_action(mut self, kind: SemanticActionKind) -> Self {
        self.policy = self.policy.deny_action(kind);
        self
    }
    /// 要求业务动作经一次性用户确认，不能由后台模式绕过。
    pub fn require_confirm(mut self, target: impl Into<String>) -> Self {
        self.policy = self.policy.require_confirm(target);
        self
    }
    /// 在后台 UI turn 投递确认意图；回调应使用非打断式通知，不得直接弹前台模态框。
    pub fn confirm_with(
        mut self,
        handler: impl Fn(AgentConfirmationRequest) + Send + Sync + 'static,
    ) -> Self {
        self.confirmation = Some(Arc::new(handler));
        self
    }

    /// 启动唯一后台操作面。成功表示首帧已真实离屏绘制且本机端点已就绪。
    pub fn spawn(self) -> Result<AgentWorkspaceHandle> {
        crate::app::window::offscreen_window::validate_extent(self.width, self.height)?;
        WORKSPACE_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "one agent workspace is already active in this process",
                )
            })?;
        let lease = WorkspaceLease;
        let mut runtime = AppRuntime::new();
        runtime.set_agent_policy(self.policy.clone());
        let stop = Arc::new(AtomicBool::new(false));
        let thread_runtime = runtime.clone();
        let thread_stop = stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("uix-agent-workspace".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let _lease = lease;
                let shutdown = WorkspaceShutdown(thread_runtime.clone());
                let result = crate::app::window::background::run(
                    self,
                    &thread_runtime,
                    &thread_stop,
                    ready_tx,
                );
                // 无论初始化、绘制还是回调失败，都先撤销目录和在途工作，不回退到可见窗口。
                drop(shutdown);
                if let Err(mpsc::SendError(Err(error))) = done_tx.send(result) {
                    thread_runtime.diagnostics().report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework(
                            "agent_workspace",
                            "late_shutdown",
                        ),
                    );
                }
            })
            .map_err(|error| {
                Error::new(
                    Errc::IoError,
                    format!("agent workspace thread startup failed: {error}"),
                )
            })?;
        let owner = worker.thread().clone();
        let waker = EventLoopWaker::new(move || owner.unpark());
        runtime.set_event_loop_waker(waker.clone());
        let mut handle = AgentWorkspaceHandle {
            runtime,
            stop,
            waker,
            worker: Some(worker),
            done: done_rx,
            discovery: PathBuf::new(),
            endpoint: String::new(),
        };
        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok((discovery, endpoint))) => {
                handle.discovery = discovery;
                handle.endpoint = endpoint;
                Ok(handle)
            }
            Ok(Err(error)) => Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(Error::new(
                Errc::Timeout,
                "agent workspace did not become ready",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => match handle.stop_and_join() {
                Err(error) => Err(error),
                Ok(()) => Err(Error::new(
                    Errc::InvalidState,
                    "agent workspace stopped before becoming ready",
                )),
            },
        }
    }
}

/// 后台操作面的可克隆转发出口：唤醒后台 owner 或向其投递短任务。
///
/// 能力与 `AgentWorkspaceHandle::wake` / `post_to_ui` 完全一致，不转移租约
/// 与关闭责任；供扩展 worker 线程、业务线程等长期持有方把结果转交后台
/// owner thread（文档场景：后台业务线程更新 State 后唤醒此操作面）。
#[derive(Clone)]
pub struct AgentWorkspacePoster {
    runtime: AppRuntime,
    waker: EventLoopWaker,
}

impl AgentWorkspacePoster {
    /// 唤醒后台 owner 重新组帧；不触碰任何可见窗口。
    pub fn wake(&self) {
        self.waker.wake();
    }
    /// 在后台 owner thread 执行短任务；会话已关闭时安全跳过。
    pub fn post_to_ui(&self, task: impl FnOnce() + Send + 'static) {
        self.runtime.post_to_ui(WindowId::ROOT, task);
    }
}

/// 后台操作面租约。Drop 请求停止并有界等待；不会关闭或激活可见窗口。
pub struct AgentWorkspaceHandle {
    pub(crate) runtime: AppRuntime,
    stop: Arc<AtomicBool>,
    waker: EventLoopWaker,
    worker: Option<JoinHandle<()>>,
    done: mpsc::Receiver<Result<()>>,
    discovery: PathBuf,
    endpoint: String,
}

impl AgentWorkspaceHandle {
    /// 连接此租约的精确端点，不扫描发现目录或自动改绑替代实例。
    pub fn client(&self) -> std::result::Result<AgentBridgeClient, AgentBridgeClientError> {
        if self.stop.load(Ordering::Acquire) || self.runtime.is_shutting_down() {
            return Err(AgentBridgeClientError::Discovery(
                "workspace is closed".into(),
            ));
        }
        let descriptor: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&self.discovery)?)?;
        if descriptor["endpoint"].as_str() != Some(self.endpoint.as_str()) {
            return Err(AgentBridgeClientError::Discovery(
                "workspace identity changed".into(),
            ));
        }
        let token = descriptor["token"].as_str().ok_or_else(|| {
            AgentBridgeClientError::Discovery("workspace token is missing".into())
        })?;
        AgentBridgeClient::connect(&self.endpoint, token)
    }
    /// 业务服务异步更新 State 后唤醒此操作面，不唤醒前台窗口。
    pub fn wake(&self) {
        self.waker.wake();
    }
    /// 在后台 owner thread 执行短任务；不得在回调中阻塞等待 Agent 响应。
    pub fn post_to_ui(&self, task: impl FnOnce() + Send + 'static) {
        self.runtime.post_to_ui(WindowId::ROOT, task);
    }
    /// 克隆转发出口（wake / post_to_ui）；租约与关闭责任仍由本句柄独占。
    pub fn poster(&self) -> AgentWorkspacePoster {
        AgentWorkspacePoster {
            runtime: self.runtime.clone(),
            waker: self.waker.clone(),
        }
    }
    /// 交回用户确认决定；只路由到本租约，不接受其他实例的确认身份。
    pub fn resolve_confirmation(
        &self,
        window_id: WindowId,
        confirm_id: u64,
        allow: bool,
    ) -> Result<()> {
        self.runtime
            .submit_agent_command(
                window_id,
                AgentCommandRequest::ResolveConfirmation { confirm_id, allow },
            )
            .map(|ticket| ticket.detach())
            .map_err(|error| {
                Error::new(
                    Errc::InvalidState,
                    format!("agent confirmation resolve failed: {error:?}"),
                )
            })
    }
    /// 请求停止并最多等待两秒回收 UI worker；超时不声称回调已取消。
    pub fn close(mut self) -> Result<()> {
        self.stop_and_join()
    }

    fn stop_and_join(&mut self) -> Result<()> {
        if self.worker.is_none() {
            return Ok(());
        }
        self.stop.store(true, Ordering::Release);
        self.waker.wake();
        let result = self.done.recv_timeout(Duration::from_secs(2));
        let worker = self.worker.take();
        match result {
            Ok(result) => {
                if let Some(worker) = worker {
                    if worker.join().is_err() {
                        return Err(Error::new(
                            Errc::InvalidState,
                            "agent workspace worker panicked",
                        ));
                    }
                }
                result
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(worker) = worker {
                    let _ = worker.join();
                }
                Err(Error::new(
                    Errc::InvalidState,
                    "agent workspace worker terminated without a result",
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 不能强制杀死 Rust 回调；关闭控制面并保留真实未完成语义，线程仍持有租约。
                self.runtime.shutdown_all();
                Err(Error::new(
                    Errc::Timeout,
                    "agent workspace callback did not stop within two seconds",
                ))
            }
        }
    }
}

impl Drop for AgentWorkspaceHandle {
    fn drop(&mut self) {
        if let Err(error) = self.stop_and_join() {
            self.runtime.diagnostics().report_with_origin(
                error,
                crate::diagnostics::ReportOrigin::framework("agent_workspace", "shutdown"),
            );
        }
    }
}
