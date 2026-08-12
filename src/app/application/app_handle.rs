// 反馈 capability 启用时才维护逐窗通知句柄表。
#[cfg(feature = "feedback")]
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
// 反馈 capability 启用时才需要通知序号与同步容器。
#[cfg(feature = "feedback")]
use std::sync::{atomic::AtomicU64, Mutex};
use std::time::Duration;
// 反馈 capability 启用时才为外部通知记录创建时刻。
#[cfg(feature = "feedback")]
use std::time::Instant;

use crate::app::application::di::Container;
use crate::app::queues::app_timer::TimerHandle;
use crate::app::session_runtime::AppRuntime;
use crate::app::window::window_config::WindowConfig;
pub use crate::core::WindowId;
use crate::core::{Errc, Error, Result};
// 反馈 capability 启用时才构造通知浮层根组件。
#[cfg(feature = "feedback")]
use crate::core::{ComponentId, Constraints, Rect, Size};
use crate::diagnostics::Diagnostics;
// 反馈 capability 启用时才生成通知浮层根组件实现。
#[cfg(feature = "feedback")]
use crate::impl_widget_component;
// 反馈 capability 启用时才把原生错误通知转换为 UI 项。
#[cfg(feature = "feedback")]
use crate::native::notification::ToastEntry;
use crate::ui::adapter::ViewAdapter;
// 反馈 capability 启用时才实现通知浮层根布局。
#[cfg(feature = "feedback")]
use crate::ui::component::traits::WidgetLayout;
// 引入应用根默认背景所需的主题样式值。
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
// 反馈 capability 启用时才连接应用通知状态与组件实现。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::notification::{
    Notification, NotificationHandle, NotificationItem,
};
// 引入应用状态、主题与布局背景语义角色。
use crate::ui::{AppState, NeutralRole, Theme};

// 反馈 capability 启用时才需要额外的应用浮层根节点。
#[cfg(feature = "feedback")]
#[derive(Default)]
pub(crate) struct AppOverlayRoot;

// 反馈 capability 启用时才生成浮层根组件能力集合。
#[cfg(feature = "feedback")]
impl_widget_component!(AppOverlayRoot; Layout);

// 反馈 capability 启用时才参与通知浮层布局。
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

#[derive(Clone)]
pub(crate) struct AppNotificationState {
    // 反馈能力启用时保存逐窗通知句柄。
    #[cfg(feature = "feedback")]
    windows: Arc<Mutex<HashMap<WindowId, NotificationHandle>>>,
    // 反馈能力启用时生成稳定外部通知 ID。
    #[cfg(feature = "feedback")]
    next_id: Arc<AtomicU64>,
    // 反馈能力启用时限制同时可见通知数。
    #[cfg(feature = "feedback")]
    max_visible: usize,
}

impl AppNotificationState {
    pub(crate) fn new() -> Self {
        // 反馈能力启用时初始化完整逐窗通知状态。
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

    // 反馈 capability 启用时才暴露内部逐窗通知句柄。
    #[cfg(feature = "feedback")]
    pub(crate) fn handle(&self, window_id: WindowId) -> NotificationHandle {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(window_id)
            .or_insert_with(NotificationHandle::new)
            .clone()
    }

    // 反馈 capability 启用时才把应用错误加入通知队列。
    #[cfg(feature = "feedback")]
    pub(crate) fn notify_error(&self, window_id: WindowId, error: &Error) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let toast = ToastEntry::from_error(id, error, Instant::now())?;
        let item = NotificationItem::from_toast_entry(&toast);
        let handle = self.handle(window_id);
        handle.push_external(id, item);
        handle.retain_latest(self.max_visible);
        Some(id)
    }

    // 反馈 capability 启用时才清理逐窗通知状态。
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
    notifications: Option<AppNotificationState>,
    window_id: WindowId,
) -> ViewNode {
    // 先应用不覆盖用户声明的应用级主题背景默认值。
    let root = apply_default_app_root_background(root);
    // 只有已安装通知状态时才继续组装通知浮层。
    let Some(notifications) = notifications else {
        // 没有通知状态时返回已经应用默认值的根节点。
        return root;
    };
    // 反馈能力启用时把通知容器挂载到应用根节点上层。
    #[cfg(feature = "feedback")]
    {
        let notification = Notification::from_handle(notifications.handle(window_id));
        ViewNode::new(
            AppOverlayRoot,
            vec![root, ViewNode::leaf(notification).z_index(10_000)],
        )
    }
    // 关闭反馈能力时根视图直通，且不构造通知组件。
    #[cfg(not(feature = "feedback"))]
    {
        let _ = (notifications, window_id);
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

    // 反馈 capability 启用时才暴露应用错误通知入口。
    #[cfg(feature = "feedback")]
    pub fn notify_error(&self, error: &Error) -> Option<u64> {
        if !self.alive.load(Ordering::Acquire) {
            return None;
        }
        let notifications = self.container.resolve_clone::<AppNotificationState>()?;
        let id = notifications.notify_error(self.window_id, error)?;
        self.runtime.post_to_ui(self.window_id, || {});
        Some(id)
    }

    // 反馈 capability 启用时才暴露通知关闭入口。
    #[cfg(feature = "feedback")]
    pub fn dismiss_notification(&self, id: u64) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        let Some(notifications) = self.container.resolve_clone::<AppNotificationState>() else {
            return false;
        };
        let dismissed = notifications.handle(self.window_id).dismiss_external(id);
        if dismissed {
            self.runtime.post_to_ui(self.window_id, || {});
        }
        dismissed
    }

    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            let notifications = self.container.resolve_clone::<AppNotificationState>();
            let window_id = self.window_id;
            self.runtime
                .enqueue_with_context(self.window_id, move |ctx| {
                    let root = ViewAdapter::capture_root(build_root);
                    // 运行期替换与初始窗口复用同一应用根默认处理。
                    let root = prepare_app_root(root, notifications, window_id);
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
        // 反馈能力启用时同步释放逐窗通知句柄。
        #[cfg(feature = "feedback")]
        {
            if let Some(notifications) = self.container.resolve_clone::<AppNotificationState>() {
                notifications.remove_window(self.window_id);
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

    // 验证通知浮层不会遮断内部应用根背景默认处理。
    #[cfg(feature = "feedback")]
    #[test]
    fn app_root_background_is_applied_before_notification_overlay() {
        // 构造没有显式背景的最小应用根节点。
        let root = ViewNode::leaf(Label::new("root"));
        // 安装通知状态并准备带浮层的应用根节点。
        let prepared = prepare_app_root(
            root,
            Some(AppNotificationState::new()),
            WindowId::ROOT,
        );
        // 浮层下的业务根必须获得主题布局背景。
        assert_eq!(
            prepared.children[0].style.background,
            Some(ColorValue::Neutral(NeutralRole::BgLayout))
        );
    }
}
