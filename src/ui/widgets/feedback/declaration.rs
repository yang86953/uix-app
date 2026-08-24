//! Message / Notification 的零布局声明租约组件。

// 引入线程安全关闭回调与租约端口所有权。
use std::sync::Arc;

// 引入组件声明宏。
use crate::widget;
// 引入类型化错误、布局约束和零尺寸。
use crate::core::{Constraints, Rect, Result, Size};
// 引入反馈状态等级。
use crate::platform::capabilities::StatusLevel;
// 引入零绘制实现所需上下文与树类型。
use crate::ui::WidgetTree;
use crate::ui::widget_runtime::paint_context::PaintContext;

// 声明条目的关闭原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 已挂载反馈条目离开可见队列的原因。
pub enum FeedbackCloseReason {
    // 用户点击关闭入口。
    /// 用户通过可见关闭入口移除条目。
    Manual,
    // 展示时长到期。
    /// 条目的展示时长到期。
    Timeout,
    // Rust API 主动关闭。
    /// 应用代码主动请求移除条目。
    Programmatic,
}

// 已确认关闭的类型化事实。
#[derive(Debug, Clone, PartialEq, Eq)]
/// 反馈条目真实关闭后传递给观察器的事实。
pub struct FeedbackClosed {
    // 返回声明条目的稳定 key。
    /// 被关闭声明条目的稳定 key。
    pub key: String,
    // 返回实际关闭原因。
    /// 条目实际离开可见队列的原因。
    pub reason: FeedbackCloseReason,
}

// 区分两类声明队列的内部身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FeedbackKind {
    // 顶部居中的短消息队列。
    Message,
    // 右上角的通知队列。
    Notification,
}

// 保存 Message 声明配置。
#[derive(Clone)]
pub(crate) struct MessageDeclarationSpec {
    // 窗口内 Message 声明稳定身份。
    pub(crate) key: String,
    // 展示状态等级。
    pub(crate) type_: StatusLevel,
    // 展示内容。
    pub(crate) content: String,
    // 毫秒展示时长，零表示不自动关闭。
    pub(crate) duration_ms: u64,
    // 是否允许手动关闭。
    pub(crate) closable: bool,
    // 只在真实关闭后调用的类型化回调。
    pub(crate) on_close: Option<Arc<dyn Fn(FeedbackClosed) + Send + Sync>>,
}

// 保存 Notification 声明配置。
#[derive(Clone)]
pub(crate) struct NotificationDeclarationSpec {
    // 窗口内 Notification 声明稳定身份。
    pub(crate) key: String,
    // 展示状态等级。
    pub(crate) type_: StatusLevel,
    // 通知标题。
    pub(crate) title: String,
    // 通知正文。
    pub(crate) content: String,
    // 毫秒展示时长，零表示不自动关闭。
    pub(crate) duration_ms: u64,
    // 是否允许手动关闭。
    pub(crate) closable: bool,
    // 只在真实关闭后调用的类型化回调。
    pub(crate) on_close: Option<Arc<dyn Fn(FeedbackClosed) + Send + Sync>>,
}

// 统一交给 Application owner 的声明配置。
#[derive(Clone)]
pub(crate) enum FeedbackDeclarationSpec {
    // Message 声明配置。
    Message(MessageDeclarationSpec),
    // Notification 声明配置。
    Notification(NotificationDeclarationSpec),
}

impl FeedbackDeclarationSpec {
    // 返回声明的稳定 key。
    pub(crate) fn key(&self) -> &str {
        // 两类配置共享同一身份契约。
        match self {
            // 读取 Message key。
            Self::Message(spec) => &spec.key,
            // 读取 Notification key。
            Self::Notification(spec) => &spec.key,
        }
    }
}

// Application System 注入声明节点的窄租约端口。
#[derive(Clone)]
pub(crate) struct FeedbackDeclarationBinding {
    // 首次挂载时获取逐窗租约。
    acquire: Arc<dyn Fn(FeedbackDeclarationSpec) -> Result<FeedbackLease> + Send + Sync>,
}

impl FeedbackDeclarationBinding {
    // 由 Application owner 构造窗口绑定端口。
    pub(crate) fn new(
        // 接收只暴露 acquire 的线程安全闭包。
        acquire: impl Fn(FeedbackDeclarationSpec) -> Result<FeedbackLease> + Send + Sync + 'static,
    ) -> Self {
        // 保存窄闭包，不暴露 owner 内部状态。
        Self {
            // 共享闭包供声明节点协调替换后继续使用。
            acquire: Arc::new(acquire),
        }
    }

    // 获取一个稳定声明租约。
    fn acquire(&self, spec: FeedbackDeclarationSpec) -> Result<FeedbackLease> {
        // 委托 Application System 校验 key、窗口和重复来源。
        (self.acquire)(spec)
    }
}

// 声明节点持有的窄租约。
pub(crate) struct FeedbackLease {
    // 同 key reconcile 的幂等更新入口。
    update: Arc<dyn Fn(FeedbackDeclarationSpec) -> Result<()> + Send + Sync>,
    // 真正卸载时释放 tombstone 与稳定身份。
    release: Arc<dyn Fn() + Send + Sync>,
}

impl FeedbackLease {
    // 由 Application owner 提供仅含更新与释放的能力。
    pub(crate) fn new(
        // 注入幂等更新闭包。
        update: impl Fn(FeedbackDeclarationSpec) -> Result<()> + Send + Sync + 'static,
        // 注入无关闭事件的释放闭包。
        release: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        // 保存两个最小能力。
        Self {
            // 共享更新入口。
            update: Arc::new(update),
            // 共享释放入口。
            release: Arc::new(release),
        }
    }

    // 更新当前租约配置。
    pub(crate) fn update(&self, spec: FeedbackDeclarationSpec) -> Result<()> {
        // owner 决定活动条目或 tombstone 的更新行为。
        (self.update)(spec)
    }

    // 释放当前租约。
    pub(crate) fn release(self) {
        // 卸载只释放，不生成 @close 事实。
        (self.release)();
    }
}

// Message 的零布局 keyed 声明节点。
widget! {
    /// 以稳定声明身份取得并维护逐窗 Message 队列租约的零布局组件。
    pub struct MessageDeclaration {
        // 保存当前声明配置。
        spec: MessageDeclarationSpec,
        // 在应用根准备阶段注入的逐窗端口。
        binding: Option<FeedbackDeclarationBinding>,
        // 挂载后持有的唯一租约。
        lease: Option<FeedbackLease>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        // 声明节点不占据任何布局空间。
        constraints.clamp(Size::new(0.0, 0.0))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 声明节点永远不直接绘制，实际内容由逐窗 Message Host 呈现。
    }

    on_mount => (&mut self) {
        // 首次挂载获取声明租约。
        self.mount_lease();
    }

    on_unmount => (&mut self) {
        // 真正卸载释放租约和 tombstone。
        self.release_lease();
    }
}

impl MessageDeclaration {
    // 创建必需 key 与 content 已满足的消息声明。
    /// 创建默认信息等级、展示三秒且不可手动关闭的消息声明。
    pub fn new(key: impl Into<String>, content: impl Into<String>) -> Self {
        // 保存文档默认值并等待 Application 绑定。
        Self {
            // 构造 Message 专属配置。
            spec: MessageDeclarationSpec {
                // 保存稳定 key。
                key: key.into(),
                // 默认信息等级。
                type_: StatusLevel::Info,
                // 保存显示内容。
                content: content.into(),
                // UIX 默认三秒。
                duration_ms: 3_000,
                // 默认不显示关闭入口。
                closable: false,
                // 默认没有关闭观察器。
                on_close: None,
            },
            // 绑定由应用根准备阶段注入。
            binding: None,
            // 挂载前没有租约。
            lease: None,
        }
    }

    // 设置消息状态等级。
    /// 设置消息的状态等级。
    pub fn type_(mut self, type_: StatusLevel) -> Self {
        // 更新声明配置。
        self.spec.type_ = type_;
        // 返回链式构建器。
        self
    }

    // 设置毫秒展示时长。
    /// 设置展示时长（毫秒）；零表示不自动关闭。
    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        // 零保留为不自动关闭。
        self.spec.duration_ms = duration_ms;
        // 返回链式构建器。
        self
    }

    // 按 UIX 秒单位设置展示时长并安全归一化非法动态输入。
    /// 设置展示时长（秒）；非法输入会记录错误并退化为不自动关闭。
    pub fn duration_seconds(mut self, seconds: f64) -> Self {
        // 只接受有限非负且能转换为毫秒的秒数。
        if seconds.is_finite() && seconds >= 0.0 && seconds <= u64::MAX as f64 / 1_000.0 {
            // 四舍五入到最近毫秒，避免系统计时使用分数毫秒。
            self.spec.duration_ms = (seconds * 1_000.0).round() as u64;
        } else {
            // 非法动态输入可观察，并安全退化为不自动关闭。
            tracing::error!("Message declaration duration must be finite and non-negative");
            // 零时长不会生成意外即时关闭。
            self.spec.duration_ms = 0;
        }
        // 返回链式构建器。
        self
    }

    // 设置手动关闭能力。
    /// 设置是否向用户提供手动关闭入口。
    pub fn closable(mut self, closable: bool) -> Self {
        // 保存声明能力。
        self.spec.closable = closable;
        // 返回链式构建器。
        self
    }

    // 注册真实关闭后的类型化观察器。
    /// 注册仅在条目真实关闭后调用的观察器。
    pub fn on_close<F>(mut self, callback: F) -> Self
    where
        // 回调可能由逐窗 owner 保存，必须满足线程安全边界。
        F: Fn(FeedbackClosed) + Send + Sync + 'static,
    {
        // 保存共享关闭回调。
        self.spec.on_close = Some(Arc::new(callback));
        // 返回链式构建器。
        self
    }

    // 由 Application 根准备阶段绑定目标窗口。
    pub(crate) fn bind(&mut self, binding: FeedbackDeclarationBinding) {
        // 建树前只写入一次窄端口。
        self.binding = Some(binding);
    }

    // 协调同类型声明节点并保留现有租约。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 判断稳定 key 是否发生变化。
        let key_changed = self.spec.key != next.spec.key;
        // 保存下一次声明配置。
        self.spec = next.spec;
        // key 变化等价于旧租约离开和新租约进入。
        if key_changed {
            // 释放旧身份且不触发关闭事件。
            self.release_lease();
            // 使用已注入端口获取新身份。
            self.mount_lease();
        } else if let Some(lease) = self.lease.as_ref() {
            // 同 key 只更新活动条目或 tombstone 配置。
            if let Err(error) = lease.update(FeedbackDeclarationSpec::Message(self.spec.clone())) {
                // 生命周期失败必须可观察，不能静默忽略。
                tracing::error!("Message declaration update failed: {}", error.short_what());
            }
        }
    }

    // 获取首次挂载租约。
    fn mount_lease(&mut self) {
        // 已持有租约时保持幂等。
        if self.lease.is_some() {
            // 普通重复挂载不重复入队。
            return;
        }
        // 未绑定 Application owner 是可观察的配置错误。
        let Some(binding) = self.binding.as_ref() else {
            // 记录错误并保持零布局安全状态。
            tracing::error!("Message declaration mounted without an application feedback owner");
            // 不能伪造租约。
            return;
        };
        // 向逐窗 owner 获取唯一 key 租约。
        match binding.acquire(FeedbackDeclarationSpec::Message(self.spec.clone())) {
            // 成功后保存租约。
            Ok(lease) => self.lease = Some(lease),
            // 重复 key 或失效窗口必须可观察。
            Err(error) => {
                tracing::error!("Message declaration mount failed: {}", error.short_what())
            }
        }
    }

    // 真正卸载当前租约。
    fn release_lease(&mut self) {
        // 只有已挂载节点需要释放。
        if let Some(lease) = self.lease.take() {
            // owner 清除条目或 tombstone，不发出 @close。
            lease.release();
        }
    }
}

// Notification 的零布局 keyed 声明节点。
widget! {
    /// 以稳定声明身份取得并维护逐窗 Notification 队列租约的零布局组件。
    pub struct NotificationDeclaration {
        // 保存当前声明配置。
        spec: NotificationDeclarationSpec,
        // 在应用根准备阶段注入的逐窗端口。
        binding: Option<FeedbackDeclarationBinding>,
        // 挂载后持有的唯一租约。
        lease: Option<FeedbackLease>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        // 声明节点不占据任何布局空间。
        constraints.clamp(Size::new(0.0, 0.0))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 声明节点永远不直接绘制，实际内容由逐窗 Notification Host 呈现。
    }

    on_mount => (&mut self) {
        // 首次挂载获取声明租约。
        self.mount_lease();
    }

    on_unmount => (&mut self) {
        // 真正卸载释放租约和 tombstone。
        self.release_lease();
    }
}

impl NotificationDeclaration {
    // 创建必需 key、title 与 content 已满足的通知声明。
    /// 创建默认信息等级、展示四点五秒且不可手动关闭的通知声明。
    pub fn new(
        key: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        // 保存文档默认值并等待 Application 绑定。
        Self {
            // 构造 Notification 专属配置。
            spec: NotificationDeclarationSpec {
                // 保存稳定 key。
                key: key.into(),
                // 默认信息等级。
                type_: StatusLevel::Info,
                // 保存标题。
                title: title.into(),
                // 保存正文。
                content: content.into(),
                // 默认时长读取 Notification 同目录 UIX 唯一视觉项。
                duration_ms: super::notification::Notification::default_duration_ms(),
                // 默认不显示关闭入口。
                closable: false,
                // 默认没有关闭观察器。
                on_close: None,
            },
            // 绑定由应用根准备阶段注入。
            binding: None,
            // 挂载前没有租约。
            lease: None,
        }
    }

    // 设置通知状态等级。
    /// 设置通知的状态等级。
    pub fn type_(mut self, type_: StatusLevel) -> Self {
        // 更新声明配置。
        self.spec.type_ = type_;
        // 返回链式构建器。
        self
    }

    // 设置毫秒展示时长。
    /// 设置展示时长（毫秒）；零表示不自动关闭。
    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        // 零保留为不自动关闭。
        self.spec.duration_ms = duration_ms;
        // 返回链式构建器。
        self
    }

    // 按 UIX 秒单位设置展示时长并安全归一化非法动态输入。
    /// 设置展示时长（秒）；非法输入会记录错误并退化为不自动关闭。
    pub fn duration_seconds(mut self, seconds: f64) -> Self {
        // 只接受有限非负且能转换为毫秒的秒数。
        if seconds.is_finite() && seconds >= 0.0 && seconds <= u64::MAX as f64 / 1_000.0 {
            // 四舍五入到最近毫秒，避免系统计时使用分数毫秒。
            self.spec.duration_ms = (seconds * 1_000.0).round() as u64;
        } else {
            // 非法动态输入可观察，并安全退化为不自动关闭。
            tracing::error!("Notification declaration duration must be finite and non-negative");
            // 零时长不会生成意外即时关闭。
            self.spec.duration_ms = 0;
        }
        // 返回链式构建器。
        self
    }

    // 设置手动关闭能力。
    /// 设置是否向用户提供手动关闭入口。
    pub fn closable(mut self, closable: bool) -> Self {
        // 保存声明能力。
        self.spec.closable = closable;
        // 返回链式构建器。
        self
    }

    // 注册真实关闭后的类型化观察器。
    /// 注册仅在条目真实关闭后调用的观察器。
    pub fn on_close<F>(mut self, callback: F) -> Self
    where
        // 回调可能由逐窗 owner 保存，必须满足线程安全边界。
        F: Fn(FeedbackClosed) + Send + Sync + 'static,
    {
        // 保存共享关闭回调。
        self.spec.on_close = Some(Arc::new(callback));
        // 返回链式构建器。
        self
    }

    // 由 Application 根准备阶段绑定目标窗口。
    pub(crate) fn bind(&mut self, binding: FeedbackDeclarationBinding) {
        // 建树前只写入一次窄端口。
        self.binding = Some(binding);
    }

    // 协调同类型声明节点并保留现有租约。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 判断稳定 key 是否发生变化。
        let key_changed = self.spec.key != next.spec.key;
        // 保存下一次声明配置。
        self.spec = next.spec;
        // key 变化等价于旧租约离开和新租约进入。
        if key_changed {
            // 释放旧身份且不触发关闭事件。
            self.release_lease();
            // 使用已注入端口获取新身份。
            self.mount_lease();
        } else if let Some(lease) = self.lease.as_ref() {
            // 同 key 只更新活动条目或 tombstone 配置。
            if let Err(error) =
                lease.update(FeedbackDeclarationSpec::Notification(self.spec.clone()))
            {
                // 生命周期失败必须可观察，不能静默忽略。
                tracing::error!(
                    "Notification declaration update failed: {}",
                    error.short_what()
                );
            }
        }
    }

    // 获取首次挂载租约。
    fn mount_lease(&mut self) {
        // 已持有租约时保持幂等。
        if self.lease.is_some() {
            // 普通重复挂载不重复入队。
            return;
        }
        // 未绑定 Application owner 是可观察的配置错误。
        let Some(binding) = self.binding.as_ref() else {
            // 记录错误并保持零布局安全状态。
            tracing::error!(
                "Notification declaration mounted without an application feedback owner"
            );
            // 不能伪造租约。
            return;
        };
        // 向逐窗 owner 获取唯一 key 租约。
        match binding.acquire(FeedbackDeclarationSpec::Notification(self.spec.clone())) {
            // 成功后保存租约。
            Ok(lease) => self.lease = Some(lease),
            // 重复 key 或失效窗口必须可观察。
            Err(error) => tracing::error!(
                "Notification declaration mount failed: {}",
                error.short_what()
            ),
        }
    }

    // 真正卸载当前租约。
    fn release_lease(&mut self) {
        // 只有已挂载节点需要释放。
        if let Some(lease) = self.lease.take() {
            // owner 清除条目或 tombstone，不发出 @close。
            lease.release();
        }
    }
}
