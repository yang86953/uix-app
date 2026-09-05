use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};
use std::time::Duration;

use crate::app::application::di::Container;
use crate::app::queues::main_thread_queue::MainThreadContext;
use crate::ui::semantic_action::SemanticAction;
// 连接 Application Module 私有逐窗 owner；无 feedback 时为零尺寸哨兵。
use crate::app::application::feedback_state::AppFeedbackState;
use crate::app::queues::app_timer::TimerHandle;
use crate::app::session_runtime::AppRuntime;
use crate::app::window::window_config::WindowConfig;
pub(crate) use crate::core::WindowId;
use crate::core::{Errc, Error, Result};
// 反馈 capability 启用时才构造反馈浮层根组件。
#[cfg(feature = "feedback")]
use crate::core::{Constraints, Rect, Size, WidgetId};
use crate::diagnostics::Diagnostics;
// test-harness 只把一次性规范像素票据暴露给应用。
#[cfg(feature = "test-harness")]
use crate::draw::SurfaceReadbackTicket;
use crate::ui::adapter::ViewAdapter;
// 反馈 capability 启用时才实现稳定快照的浮层根组件契约。
#[cfg(feature = "feedback")]
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities, WidgetLayout};
// 引入应用根默认背景所需的主题样式值。
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
// 引入 UI 反馈 Module 定义的窄声明租约契约。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::declaration::{
    FeedbackKind, MessageDeclaration, NotificationDeclaration,
};
// 反馈 capability 启用时才连接应用逐窗反馈状态与 Message Host。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::message::{Message, MessageItem};
// Notification Host 与条目继续使用反馈 Module 公开的窄句柄契约。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::notification::{Notification, NotificationItem};
// 引入应用状态、主题与布局背景语义角色。
use crate::ui::{AppState, NeutralRole, Theme};

#[cfg(feature = "feedback")]
#[derive(Default)]
pub(crate) struct AppOverlayRoot;

#[cfg(feature = "feedback")]
impl Widget for AppOverlayRoot {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn snapshot_fields(&self) -> crate::ui::SnapshotFields {
        // 零状态应用壳在任意声明轮次都保持配置等价。
        crate::ui::SnapshotFields::Custom {
            widget: "AppOverlayRoot",
            fields: Vec::new(),
        }
    }

    fn capabilities(&self) -> WidgetCapabilities {
        let mut capabilities = WidgetCapabilities::new();
        capabilities.insert(WidgetCapabilities::LAYOUT);
        capabilities
    }

    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        Some(self)
    }
}

#[cfg(feature = "feedback")]
impl WidgetLayout for AppOverlayRoot {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.definite.unwrap_or_default()
    }

    fn measure_children_into(
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &crate::ui::WidgetTree,
        output: &mut Vec<crate::ui::LayoutChild>,
    ) {
        output.clear();
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| crate::ui::LayoutChild::new(id, Size::zero())),
        );
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &crate::ui::WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        children.iter().map(|child| (child.id, frame)).collect()
    }

    fn layout_children_into(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &crate::ui::WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        output.extend(children.iter().map(|child| (child.id, frame)));
    }
}

// 在建树前递归为反馈声明节点注入目标窗口窄端口。
#[cfg(feature = "feedback")]
fn bind_feedback_declarations(
    node: &mut ViewNode,
    feedback: &AppFeedbackState,
    window_id: WindowId,
) {
    // Message 声明只能获取 Message 类型端口。
    if let Some(declaration) = node
        .widget
        .as_any_mut()
        .downcast_mut::<MessageDeclaration>()
    {
        // 注入绑定目标窗口的 acquire 能力。
        declaration.bind(feedback.declaration_binding(window_id, FeedbackKind::Message));
    }
    // Notification 声明只能获取 Notification 类型端口。
    if let Some(declaration) = node
        .widget
        .as_any_mut()
        .downcast_mut::<NotificationDeclaration>()
    {
        // 注入绑定目标窗口的 acquire 能力。
        declaration.bind(feedback.declaration_binding(window_id, FeedbackKind::Notification));
    }
    // 递归覆盖任意容器与控制流展开产生的后代。
    for child in &mut node.children {
        // 所有后代沿用同一个 WindowId owner。
        bind_feedback_declarations(child, feedback, window_id);
    }
}

// 统一准备主窗、副窗与运行期替换使用的应用根节点。
pub(crate) fn prepare_app_root(
    root: ViewNode,
    feedback: Option<AppFeedbackState>,
    window_id: WindowId,
) -> ViewNode {
    // 先应用不覆盖用户声明的应用级主题背景默认值。
    let root = apply_default_app_root_background(root);
    // 只有已安装反馈状态时才继续组装逐窗反馈浮层。
    let Some(feedback) = feedback else {
        // 没有反馈状态时返回已经应用默认值的根节点。
        return root;
    };
    // 反馈能力启用时把两个 Host 挂载到应用根节点上层。
    #[cfg(feature = "feedback")]
    {
        // 在生命周期 mount 前把声明节点绑定到目标窗口 owner。
        let mut root = root;
        // 递归处理业务根中全部反馈声明节点。
        bind_feedback_declarations(&mut root, &feedback, window_id);
        // 同一次查找取得同一窗口的 Message 与 Notification 句柄。
        let handles = feedback.handles(window_id);
        // Message Host 只消费当前窗口消息队列。
        let message = Message::from_handle(handles.message);
        // Notification Host 只消费当前窗口通知队列。
        let notification = Notification::from_handle(handles.notification);
        // 两个 Host 都是覆盖业务根的无布局浮层子节点。
        ViewNode::new(
            AppOverlayRoot,
            vec![
                // 业务根保留第一个子节点身份。
                root,
                // Message 自身的 overlay kind 决定最终浮层语义。
                ViewNode::leaf(message).z_index(10_000),
                // Notification 与 Message 使用相邻稳定层级。
                ViewNode::leaf(notification).z_index(10_001),
            ],
        )
    }
    // 关闭反馈能力时根视图直通，且不构造通知组件。
    #[cfg(not(feature = "feedback"))]
    {
        // 无 feedback capability 时只消费通用参数以保持单一路径。
        let _ = (feedback, window_id);
        root
    }
}

// 为没有显式背景的应用根节点补充主题布局背景。
fn apply_default_app_root_background(mut root: ViewNode) -> ViewNode {
    // 用户未声明背景时才写入 App 级默认值。
    if root.style.background.is_none() {
        // 使用语义令牌，使背景继续跟随当前应用主题解析。
        root.style.background = Some(ColorValue::Neutral(NeutralRole::BgLayout));
    }
    // 保留根组件身份、子树以及全部其他声明属性。
    root
}

#[derive(Clone)]
/// 向特定应用窗口调度任务、更新视图和访问应用服务的可克隆句柄。
pub struct AppHandle {
    window_id: WindowId,
    app_state: AppState,
    runtime: AppRuntime,
    container: Container,
    alive: Arc<AtomicBool>,
}

impl AppHandle {
    pub(crate) fn new(
        window_id: WindowId,
        app_state: AppState,
        runtime: AppRuntime,
        container: Container,
        alive: Arc<AtomicBool>,
    ) -> Self {
        Self {
            window_id,
            app_state,
            runtime,
            container,
            alive,
        }
    }

    /// 返回此句柄关联的窗口身份。
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// 返回此句柄关联窗口在该次读取时是否仍有活动会话。
    ///
    /// 结果是瞬时生命周期快照；并发关闭可能在返回后立即发生。调用方仍须处理
    /// 后续窗口请求的 `InvalidState`，不得把该布尔值当作跨线程租约。
    pub fn is_open(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// 请求合成器激活并聚焦此窗口。
    ///
    /// 成功只表示请求已由活动窗口会话接受并投递到其 owner thread，不保证窗口
    /// 管理器最终授予焦点。平台在执行期拒绝请求时由框架记录 typed error；关闭
    /// 后的句柄稳定返回 `InvalidState`，不会重定向到其他窗口。
    pub fn request_activate(&self) -> Result<()> {
        if !self.is_open() {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot activate a closed AppHandle",
            ));
        }
        // 平台在执行期拒绝激活请求是文档声明的策略行为（如 compositor 焦点
        // 管控）：经冷却去重观察，不进报告存储。
        let diagnostics = self.runtime.diagnostics();
        let accepted = self
            .runtime
            .enqueue_with_context(self.window_id, move |context| {
                if let Err(error) = context.request_activate() {
                    diagnostics.observe_transient_error(
                        "window",
                        "activation request rejected by platform", &error);
                }
            });
        if accepted {
            Ok(())
        } else {
            Err(Error::new(
                Errc::InvalidState,
                "cannot activate a closed AppHandle",
            ))
        }
    }

    /// 返回可克隆的应用状态访问句柄。
    pub fn app_state(&self) -> AppState {
        self.app_state.clone()
    }

    /// Returns the runtime-scoped stability, recovery, and reporting handle.
    ///
    /// The handle remains usable after this window closes so final reports and
    /// snapshots are not lost with the window lifecycle.
    pub fn diagnostics(&self) -> Diagnostics {
        self.runtime.diagnostics()
    }

    /// 从应用依赖容器解析并克隆指定服务。
    pub fn resolve<T: 'static + Send + Sync + Clone>(&self) -> Option<T> {
        self.container.resolve_clone::<T>()
    }

    /// 在窗口仍存活时安排一次延迟任务，否则返回非活动计时器句柄。
    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_after(self.window_id, delay, f)
    }

    /// 在窗口仍存活时安排周期任务，否则返回非活动计时器句柄。
    pub fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_interval(self.window_id, interval, f)
    }

    /// 在窗口仍存活时向其 UI 线程投递一次任务。
    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            self.runtime.post_to_ui(self.window_id, f);
        }
    }

    /// 替换应用主题并通知全部存活窗口；仅色板变化只重绘，字号变化同时请求布局。
    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot set the theme from a closed AppHandle",
            ));
        }
        self.runtime.set_theme(theme);
        Ok(())
    }

    /// 在目标窗口下一次真实绘制帧注入一次 `GraphicsDeviceLost`。
    ///
    /// 仅随 `test-harness` feature 提供，用于应用级恢复测试；失败帧仍由
    /// 正常 `RecoveryDriver` 边界处理，不会绕过或直接替换引擎。
    /// 调用方应同时更新一个可观察 State，以保证下一帧确实需要呈现。
    #[cfg(feature = "test-harness")]
    pub fn inject_graphics_device_lost_for_test(&self) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot inject a graphics fault from a closed AppHandle",
            ));
        }
        self.runtime
            .inject_graphics_device_lost_for_test(self.window_id)
    }

    /// 在目标窗口下一次真实绘制帧注入一次 `GraphicsSurfaceLost`。
    ///
    /// 仅随 `test-harness` feature 提供；surface 错误仍由 RHI acquire 和
    /// 正常 `RecoveryDriver` 边界处理，不绕过或直接替换图形引擎。
    #[cfg(feature = "test-harness")]
    pub fn inject_graphics_surface_lost_for_test(&self) -> Result<()> {
        // 关闭后的 AppHandle 不能再安排 owner-thread 图形故障。
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot inject a surface fault from a closed AppHandle",
            ));
        }
        // 把 surface 故障交给窗口会话的恢复信号。
        self.runtime
            .inject_graphics_surface_lost_for_test(self.window_id)
    }

    /// 请求目标窗口下一次成功 GPU 帧的规范 surface 像素快照。
    ///
    /// 返回票据应在非 UI 线程通过 `recv_timeout` 等待；调用方仍需通过状态更新
    /// 触发一帧实际绘制。结果采用左上原点与 `0xAARRGGBB` 像素格式。
    #[cfg(feature = "test-harness")]
    pub fn request_surface_readback_for_test(&self) -> Result<SurfaceReadbackTicket> {
        // 关闭后的 AppHandle 不得产生永远无人完成的票据。
        if !self.alive.load(Ordering::Acquire) {
            // 返回稳定的生命周期错误。
            return Err(Error::new(
                // 当前句柄已经不可用。
                Errc::InvalidState,
                // 诊断说明请求目标已经关闭。
                "cannot request surface readback from a closed AppHandle",
            ));
        }
        // 投递到目标窗口共享信号，实际回读仍在图形 owner thread 执行。
        self.runtime
            .request_surface_readback_for_test(self.window_id)
    }

    // 反馈 capability 启用时统一检查窗口存活与 Application owner 可用性。
    #[cfg(feature = "feedback")]
    fn feedback_state(&self) -> Result<AppFeedbackState> {
        // 关闭后的 AppHandle 不得继续写入已释放的逐窗队列。
        if !self.alive.load(Ordering::Acquire) {
            // 返回类型化 InvalidState，禁止 no-op 或伪造稳定 ID。
            return Err(Error::new(
                Errc::InvalidState,
                "cannot send feedback from a closed AppHandle",
            ));
        }
        // feedback capability 正常启动时容器必须安装唯一 Application owner。
        self.container
            .resolve_clone::<AppFeedbackState>()
            // 缺失 owner 也是生命周期配置错误，不降级为静默失败。
            .ok_or_else(|| Error::new(Errc::InvalidState, "application feedback is unavailable"))
    }

    /// 交回一次 Agent 确认的用户决定（确认 UI 回调收到
    /// [`AgentConfirmationRequest`] 后调用）。
    ///
    /// `allow = true` 时在目标窗口 UI turn 内执行登记的动作并完成 AI 的
    /// `confirm` 请求；`allow = false` 时以 `confirmation_rejected` 完成。
    #[cfg(feature = "agent-control")]
    pub fn resolve_agent_confirmation(
        &self,
        window_id: WindowId,
        confirm_id: u64,
        allow: bool,
    ) -> Result<()> {
        use crate::app::queues::agent_command_queue::AgentCommandRequest;
        self.runtime
            .submit_agent_command(
                window_id,
                AgentCommandRequest::ResolveConfirmation { confirm_id, allow },
            )
            .map(|_| ())
            .map_err(|error| {
                Error::new(
                    Errc::InvalidState,
                    format!("agent confirmation resolve failed: {error:?}"),
                )
            })
    }

    // 把消息条目显式投递到此 AppHandle 对应的窗口。
    #[cfg(feature = "feedback")]
    /// 向此窗口的消息队列投递条目并返回本地稳定 ID。
    pub fn push_message(&self, item: MessageItem) -> Result<u64> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // owner 返回对应窗口队列生成的稳定 ID。
        let id = feedback.push_message(self.window_id, item);
        // 安排 UI 线程观察队列 generation 变化。
        self.runtime.post_to_ui(self.window_id, || {});
        // 把真实稳定 ID 返回调用方。
        Ok(id)
    }

    // 把通知条目显式投递到此 AppHandle 对应的窗口。
    #[cfg(feature = "feedback")]
    /// 向此窗口的通知队列投递条目并返回本地稳定 ID。
    pub fn push_notification(&self, item: NotificationItem) -> Result<u64> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // owner 返回对应窗口队列生成的稳定 ID。
        let id = feedback.push_notification(self.window_id, item);
        // 安排 UI 线程观察队列 generation 变化。
        self.runtime.post_to_ui(self.window_id, || {});
        // 把真实稳定 ID 返回调用方。
        Ok(id)
    }

    // 把应用错误转换为目标窗口原生语义通知。
    #[cfg(feature = "feedback")]
    /// 按反馈策略把非致命错误转换为此窗口的通知。
    pub fn notify_error(&self, error: &Error) -> Result<Option<u64>> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // 某些错误级别可按现有策略选择不生成通知。
        let id = feedback.notify_error(self.window_id, error);
        // 只有实际产生条目时才唤醒 UI。
        if id.is_some() {
            // 安排 UI 线程观察通知队列变化。
            self.runtime.post_to_ui(self.window_id, || {});
        }
        // 区分“合法但无需通知”与窗口生命周期失败。
        Ok(id)
    }

    // 反馈 capability 启用时才暴露消息关闭入口。
    #[cfg(feature = "feedback")]
    /// 请求关闭此窗口中使用本地稳定 ID 标识的消息。
    pub fn dismiss_message(&self, id: u64) -> Result<bool> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // Message 的公开投递使用本地稳定 ID。
        let dismissed = feedback.handles(self.window_id).message.dismiss(id);
        // 只在队列确实变化时唤醒 UI。
        if dismissed {
            // 安排 Host 启动离场动画。
            self.runtime.post_to_ui(self.window_id, || {});
        }
        // 返回真实移除结果。
        Ok(dismissed)
    }

    // 反馈 capability 启用时才暴露通知关闭入口。
    #[cfg(feature = "feedback")]
    /// 请求关闭此窗口中使用本地稳定 ID 标识的通知。
    pub fn dismiss_notification(&self, id: u64) -> Result<bool> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // Rust 公开投递使用 Notification 的本地稳定 ID。
        let dismissed = feedback.handles(self.window_id).notification.dismiss(id);
        // 只在队列确实变化时唤醒 UI。
        if dismissed {
            // 安排 Host 启动离场动画。
            self.runtime.post_to_ui(self.window_id, || {});
        }
        // 返回真实移除结果。
        Ok(dismissed)
    }

    // 关闭由 notify_error 返回外部稳定 ID 对应的原生错误通知。
    #[cfg(feature = "feedback")]
    /// 请求关闭由 [`Self::notify_error`] 返回外部 ID 标识的错误通知。
    pub fn dismiss_error_notification(&self, id: u64) -> Result<bool> {
        // 先验证窗口生命周期，再访问逐窗 owner。
        let feedback = self.feedback_state()?;
        // 原生错误通知使用独立的外部 ID 命名空间。
        let dismissed = feedback
            .handles(self.window_id)
            .notification
            .dismiss_external(id);
        // 只在队列确实变化时唤醒 UI。
        if dismissed {
            // 安排 Host 启动离场动画。
            self.runtime.post_to_ui(self.window_id, || {});
        }
        // 返回真实移除结果。
        Ok(dismissed)
    }

    /// 在窗口 UI turn 内按 automationId 定位唯一节点并执行语义动作。
    ///
    /// 这是应用内部（非外部 Agent 通道）的自动化入口：不经过 Agent 策略与
    /// 代际校验，直接走窗口正常 UI 路径。动作结果经通道回传；目标不存在、
    /// 不唯一或动作未生效时回传错误描述。窗口关闭后命令不会执行，接收方
    /// 应使用带超时的等待。
    pub fn perform_automation_action(
        &self,
        automation_id: impl Into<String>,
        action: SemanticAction,
    ) -> Result<std::sync::mpsc::Receiver<Result<(), String>>> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "应用已关闭，无法执行自动化动作",
            ));
        }
        let automation_id = automation_id.into();
        let (tx, rx) = std::sync::mpsc::channel();
        let accepted = self.runtime.enqueue_with_context(
            self.window_id,
            move |context: &mut MainThreadContext<'_>| {
                let outcome =
                    perform_automation_action_on_tree(context.tree_mut(), &automation_id, &action);
                // 接收方已放弃等待时发送失败是合法结果。
                let _ = tx.send(outcome);
            },
        );
        if !accepted {
            return Err(Error::new(
                Errc::InvalidState,
                "窗口不存在或已关闭，自动化动作被拒绝",
            ));
        }
        Ok(rx)
    }

    /// 在窗口 UI 上下文中重新构建并替换应用根视图。
    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            // 捕获 Application owner，使替换根继续绑定同一逐窗反馈队列。
            let feedback = self.container.resolve_clone::<AppFeedbackState>();
            let window_id = self.window_id;
            let _accepted = self
                .runtime
                .enqueue_with_context(self.window_id, move |ctx| {
                    let root = ViewAdapter::capture_root(build_root);
                    // 运行期替换与初始窗口复用同一应用根默认处理。
                    let root = prepare_app_root(root, feedback, window_id);
                    ctx.update_root(root);
                });
        }
    }

    /// 使用给定声明式 View 替换此窗口的应用根视图。
    pub fn set_root<V>(&self, view: V)
    where
        V: View + Send + 'static,
    {
        self.update_view(move || view.build());
    }

    /// 请求创建另一个应用窗口并返回其独立句柄。
    pub fn open_window(&self, config: WindowConfig) -> Result<AppHandle> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot open a window from a closed AppHandle",
            ));
        }
        let session = self.runtime.request_open_window(config);
        Ok(Self::new(
            session.window_id,
            self.app_state.clone(),
            self.runtime.clone(),
            self.container.clone(),
            session.alive,
        ))
    }

    pub(crate) fn mark_closed(&self) {
        self.alive.store(false, Ordering::Release);
        // 反馈能力启用时同步释放逐窗反馈句柄。
        #[cfg(feature = "feedback")]
        {
            if let Some(feedback) = self.container.resolve_clone::<AppFeedbackState>() {
                feedback.remove_window(self.window_id);
            }
        }
        self.runtime.close_session(self.window_id);
    }
}


/// 在窗口 WidgetTree 上按 automationId 定位唯一节点并执行语义动作。
fn perform_automation_action_on_tree(
    tree: &mut crate::ui::WidgetTree,
    automation_id: &str,
    action: &SemanticAction,
) -> Result<(), String> {
    let body = tree.semantic_snapshot_body();
    let matches: Vec<_> = body
        .nodes
        .iter()
        .filter(|node| node.automation_id.as_deref() == Some(automation_id))
        .collect();
    let node = match matches.as_slice() {
        [node] => node.id,
        [] => return Err(format!("未找到自动化目标 {automation_id}")),
        _ => return Err(format!("自动化目标 {automation_id} 不唯一")),
    };
    tree.perform_semantic_action(node, action)
        .map_err(|error| format!("{error:?}"))
}
