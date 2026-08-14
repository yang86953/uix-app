// 反馈 capability 启用时才维护逐窗反馈句柄表。
#[cfg(feature = "feedback")]
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
// 反馈 capability 启用时才需要原生通知序号与同步容器。
#[cfg(feature = "feedback")]
use std::sync::{atomic::AtomicU64, Mutex};
use std::time::Duration;
// 反馈 capability 启用时才为原生错误通知记录创建时刻。
#[cfg(feature = "feedback")]
use std::time::Instant;

use crate::app::application::di::Container;
use crate::app::queues::app_timer::TimerHandle;
use crate::app::session_runtime::AppRuntime;
use crate::app::window::window_config::WindowConfig;
pub use crate::core::WindowId;
use crate::core::{Errc, Error, Result};
// 反馈 capability 启用时才构造反馈浮层根组件。
#[cfg(feature = "feedback")]
use crate::core::{ComponentId, Constraints, Rect, Size};
use crate::diagnostics::Diagnostics;
// 反馈 capability 启用时才生成反馈浮层根组件实现。
#[cfg(feature = "feedback")]
use crate::impl_widget_component;
// 反馈 capability 启用时才把原生错误通知转换为 UI 项。
#[cfg(feature = "feedback")]
use crate::native::notification::ToastEntry;
use crate::ui::adapter::ViewAdapter;
// 反馈 capability 启用时才实现反馈浮层根布局。
#[cfg(feature = "feedback")]
use crate::ui::component::traits::WidgetLayout;
// 引入应用根默认背景所需的主题样式值。
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
// 反馈 capability 启用时才连接应用逐窗反馈状态与 Message Host。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::message::{Message, MessageHandle, MessageItem};
// Notification Host 与条目继续使用反馈 Module 公开的窄句柄契约。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::notification::{
    Notification, NotificationHandle, NotificationItem,
};
// 引入应用状态、主题与布局背景语义角色。
use crate::ui::{AppState, NeutralRole, Theme};

// 反馈 capability 启用时才需要额外的应用反馈浮层根节点。
#[cfg(feature = "feedback")]
#[derive(Default)]
pub(crate) struct AppOverlayRoot;

// 反馈 capability 启用时才生成反馈浮层根组件能力集合。
#[cfg(feature = "feedback")]
impl_widget_component!(AppOverlayRoot; Layout);

// 反馈 capability 启用时才参与反馈浮层布局。
#[cfg(feature = "feedback")]
impl WidgetLayout for AppOverlayRoot {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.definite.unwrap_or_default()
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &crate::ui::WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

// 保存单个窗口的两类反馈队列句柄，避免在 Application System 外复制 owner。
#[cfg(feature = "feedback")]
#[derive(Clone)]
struct AppWindowFeedback {
    // Message Host 与声明条目共享这一窗口私有队列。
    message: MessageHandle,
    // Notification Host 与原生错误桥共享这一窗口私有队列。
    notification: NotificationHandle,
}

// Application System 持有的唯一逐窗反馈状态。
#[derive(Clone)]
pub(crate) struct AppFeedbackState {
    // 反馈能力启用时保存逐窗 Message 与 Notification 句柄。
    #[cfg(feature = "feedback")]
    windows: Arc<Mutex<HashMap<WindowId, AppWindowFeedback>>>,
    // 反馈能力启用时生成稳定的原生错误通知 ID。
    #[cfg(feature = "feedback")]
    next_id: Arc<AtomicU64>,
    // 反馈能力启用时限制同时可见通知数。
    #[cfg(feature = "feedback")]
    max_visible: usize,
}

impl AppFeedbackState {
    pub(crate) fn new() -> Self {
        // 反馈能力启用时初始化完整逐窗反馈状态。
        #[cfg(feature = "feedback")]
        {
            Self {
                windows: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(AtomicU64::new(0)),
                max_visible: 5,
            }
        }
        // 关闭反馈能力时保留零尺寸 DI 哨兵，通用应用流程无需分叉。
        #[cfg(not(feature = "feedback"))]
        {
            Self {}
        }
    }

    // 反馈 capability 启用时按需创建并返回同一窗口的完整反馈句柄组。
    #[cfg(feature = "feedback")]
    fn handles(&self, window_id: WindowId) -> AppWindowFeedback {
        // 锁中只进行轻量查找或首次队列创建，不执行用户回调。
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(window_id)
            .or_insert_with(|| AppWindowFeedback {
                // 为目标窗口创建唯一 Message 队列。
                message: MessageHandle::new(),
                // 为目标窗口创建唯一 Notification 队列。
                notification: NotificationHandle::new(),
            })
            .clone()
    }

    // 把 Rust 侧消息加入目标窗口队列。
    #[cfg(feature = "feedback")]
    fn push_message(&self, window_id: WindowId, item: MessageItem) -> u64 {
        // 队列生成的 ID 在对应窗口和反馈类型内稳定。
        self.handles(window_id).message.add(item)
    }

    // 把 Rust 侧通知加入目标窗口队列。
    #[cfg(feature = "feedback")]
    fn push_notification(&self, window_id: WindowId, item: NotificationItem) -> u64 {
        // 队列生成的 ID 在对应窗口和反馈类型内稳定。
        self.handles(window_id).notification.add(item)
    }

    // 反馈 capability 启用时才把应用错误加入通知队列。
    #[cfg(feature = "feedback")]
    pub(crate) fn notify_error(&self, window_id: WindowId, error: &Error) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let toast = ToastEntry::from_error(id, error, Instant::now())?;
        let item = NotificationItem::from_toast_entry(&toast);
        // 原生错误桥只写入目标窗口的 Notification 队列。
        let handle = self.handles(window_id).notification;
        handle.push_external(id, item);
        handle.retain_latest(self.max_visible);
        Some(id)
    }

    // 反馈 capability 启用时才清理逐窗反馈状态。
    #[cfg(feature = "feedback")]
    pub(crate) fn remove_window(&self, window_id: WindowId) {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&window_id);
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

    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

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

    pub fn resolve<T: 'static + Send + Sync + Clone>(&self) -> Option<T> {
        self.container.resolve_clone::<T>()
    }

    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_after(self.window_id, delay, f)
    }

    pub fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_interval(self.window_id, interval, f)
    }

    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            self.runtime.post_to_ui(self.window_id, f);
        }
    }

    /// Replaces the app-wide theme and schedules palette-only repaint work for
    /// every live window.
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

    // 把消息条目显式投递到此 AppHandle 对应的窗口。
    #[cfg(feature = "feedback")]
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

    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            // 捕获 Application owner，使替换根继续绑定同一逐窗反馈队列。
            let feedback = self.container.resolve_clone::<AppFeedbackState>();
            let window_id = self.window_id;
            self.runtime
                .enqueue_with_context(self.window_id, move |ctx| {
                    let root = ViewAdapter::capture_root(build_root);
                    // 运行期替换与初始窗口复用同一应用根默认处理。
                    let root = prepare_app_root(root, feedback, window_id);
                    ctx.update_root(root);
                });
        }
    }

    pub fn set_root<V>(&self, view: V)
    where
        V: View + Send + 'static,
    {
        self.update_view(move || view.build());
    }

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

// 验证应用根主题背景默认值及用户覆写优先级。
#[cfg(test)]
mod tests {
    // 引入当前模块私有的应用根准备函数。
    use super::*;
    // 引入最小文本组件构造测试根节点。
    use crate::ui::widgets::Label;
    // 引入反馈状态等级以构造最小消息条目。
    #[cfg(feature = "feedback")]
    use crate::native::capabilities::system::StatusLevel;

    // 验证透明根节点获得主题布局背景。
    #[test]
    fn app_root_uses_layout_background_when_unspecified() {
        // 构造没有显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root"));
        // 在不安装通知浮层的路径准备应用根节点。
        let prepared = prepare_app_root(root, None, WindowId::ROOT);
        // 默认值必须保持为可随主题解析的布局背景语义令牌。
        assert_eq!(
            prepared.style.background,
            Some(ColorValue::Neutral(NeutralRole::BgLayout))
        );
    }

    // 验证用户显式背景高于应用根默认值。
    #[test]
    fn app_root_preserves_explicit_background() {
        // 选择与默认布局背景不同的显式容器背景。
        let explicit = ColorValue::Neutral(NeutralRole::BgContainer);
        // 构造已经声明显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root")).bg(explicit);
        // 在不安装通知浮层的路径准备应用根节点。
        let prepared = prepare_app_root(root, None, WindowId::ROOT);
        // 应用默认处理不得覆盖调用方声明的背景。
        assert_eq!(prepared.style.background, Some(explicit));
    }

    // 验证反馈浮层不会遮断内部应用根背景默认处理。
    #[cfg(feature = "feedback")]
    #[test]
    fn app_root_installs_window_feedback_hosts_after_business_root() {
        // 构造没有显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root"));
        // 安装反馈状态并准备带浮层的应用根节点。
        let prepared = prepare_app_root(root, Some(AppFeedbackState::new()), WindowId::ROOT);
        // 应用根必须依次包含业务根、Message Host 与 Notification Host。
        assert_eq!(prepared.children.len(), 3);
        // 浮层下的业务根必须获得主题布局背景。
        assert_eq!(
            prepared.children[0].style.background,
            Some(ColorValue::Neutral(NeutralRole::BgLayout))
        );
        // 第二个子节点必须绑定 Message Host。
        assert!(prepared.children[1].widget.as_any().is::<Message>());
        // 第三个子节点必须绑定 Notification Host。
        assert!(prepared.children[2].widget.as_any().is::<Notification>());
    }

    // 验证不同 WindowId 不会共享反馈队列。
    #[cfg(feature = "feedback")]
    #[test]
    fn feedback_state_isolates_message_queues_by_window() {
        // 创建 Application System 唯一反馈 owner。
        let feedback = AppFeedbackState::new();
        // 构造可观察的最小消息条目。
        let item = MessageItem {
            // 使用信息等级避免引入错误语义。
            type_: StatusLevel::Info,
            // 内容只用于区分测试输入。
            content: "root".to_string(),
            // 零时长避免自动关闭影响队列断言。
            duration_ms: 0,
            // 本测试不需要关闭按钮。
            closable: false,
        };
        // 只向根窗口写入一个条目。
        feedback.push_message(WindowId::ROOT, item);
        // 根窗口队列必须保存该条目。
        assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
        // 第二个窗口必须获得独立的空队列。
        assert_eq!(feedback.handles(WindowId::new(1)).message.len(), 0);
    }

    // 验证关闭后的 AppHandle 返回类型化失败而不是伪 ID。
    #[cfg(feature = "feedback")]
    #[test]
    fn closed_app_handle_rejects_feedback_with_invalid_state() {
        // 容器仍安装反馈 owner，用于证明失败来自窗口生命周期。
        let mut container = Container::new();
        // 注册 Application System 唯一反馈状态。
        container.singleton(AppFeedbackState::new());
        // 构造已经关闭的窗口存活标记。
        let alive = Arc::new(AtomicBool::new(false));
        // 创建只用于聚焦契约测试的 AppHandle。
        let handle = AppHandle::new(
            // 绑定根窗口身份。
            WindowId::ROOT,
            // 使用独立应用组件状态。
            AppState::new(),
            // 使用独立运行时，不注册窗口会话。
            AppRuntime::new(),
            // 注入已安装反馈 owner 的容器。
            container,
            // 注入关闭状态。
            alive,
        );
        // 构造不会自动关闭的最小消息条目。
        let item = MessageItem {
            // 使用信息等级。
            type_: StatusLevel::Info,
            // 内容只用于触发公开入口。
            content: "closed".to_string(),
            // 零时长避免无关计时语义。
            duration_ms: 0,
            // 本测试不需要关闭按钮。
            closable: false,
        };
        // 关闭窗口必须返回 InvalidState，禁止 no-op 或返回零 ID。
        let error = handle.push_message(item).expect_err("关闭窗口必须拒绝反馈");
        // 错误码必须稳定表达生命周期违规。
        assert_eq!(error.code(), Errc::InvalidState);
    }
}
