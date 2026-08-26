// ============================================================================
// platform/notification.rs — 消息通知服务
//
// 职责：
//   1. 包装平台 INotification（系统通知）
//   2. 管理应用内通知队列（Toast 数据源）
//   3. 提供回调注册，通知 UI 层刷新
//   4. 支持通知级别：Info / Success / Warning / Error
//
// 原位于 services crate，迁入 platform 层以消除服务层。
// ============================================================================

use crate::core::error::{Error, Result};
use crate::diagnostics::{Diagnostics, DiagnosticsConfig};
use crate::platform::capabilities::StatusLevel;
use crate::platform::services::NotificationSource;
use crate::platform::system::INotification;
use std::collections::VecDeque;

pub(crate) use crate::platform::services::ToastEntry;

/// 通知服务 — 管理系统通知和应用内 Toast。
///
/// 通过 DI 注入使用：
/// ```ignore
/// let mut svc = NotificationService::new();
/// svc.notify("Hello", "World", StatusLevel::Info, 4000);
/// ```
pub struct NotificationService {
    /// 平台通知实现（可选 — headless 模式下为 None）。
    platform_notifier: Option<Box<dyn INotification>>,
    /// 应用内 Toast 队列。
    toasts: VecDeque<ToastEntry>,
    /// 同时可见的最大 Toast 数。
    max_visible: usize,
    /// 自增 ID 计数器。
    next_id: u64,
    /// 可选回调：Toast 队列变化时触发（UI 刷新）。
    on_change: Option<Box<dyn Fn() + Send>>,
    /// 运行时 Diagnostics — 通知失败在此最终责任边界提交观察。
    diagnostics: Diagnostics,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self {
            platform_notifier: None,
            toasts: VecDeque::new(),
            max_visible: 5,
            next_id: 0,
            on_change: None,
            diagnostics: Diagnostics::new(DiagnosticsConfig::default()),
        }
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self::default()
    }

    /// 附加平台通知后端（如 Platform trait 提供）。
    pub fn with_platform(mut self, notifier: Box<dyn INotification>) -> Self {
        self.platform_notifier = Some(notifier);
        self
    }

    /// 注入运行时 Diagnostics（通知失败在此最终责任边界提交观察）。
    pub fn with_diagnostics(mut self, diagnostics: Diagnostics) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// 设置同时可见的最大 Toast 数。
    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n.max(1);
        self
    }

    /// 更新同时可见的最大 Toast 数（原地）。
    pub fn set_max_visible(&mut self, n: usize) {
        self.max_visible = n.max(1);
    }

    /// 注册 Toast 队列变化时的回调。
    pub fn on_change(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_change = Some(cb);
    }

    // ── 核心 API ─────────────────────────────────────────────────────────

    const DURATION_INFO: u32 = 4000;
    const DURATION_SUCCESS: u32 = 4000;
    const DURATION_WARNING: u32 = 5000;
    const DURATION_ERROR: u32 = 6000;

    /// 发送默认通知（Info, 4s）。
    pub fn info(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Info, Self::DURATION_INFO);
    }

    /// 发送成功通知。
    pub fn success(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Success, Self::DURATION_SUCCESS);
    }

    /// 发送警告通知。
    pub fn warning(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Warning, Self::DURATION_WARNING);
    }

    /// 发送错误通知。
    pub fn error(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Error, Self::DURATION_ERROR);
    }

    /// Enqueue a non-fatal framework error as an app toast.
    ///
    /// Fatal errors stay on the diagnostic/crash path and deliberately do not
    /// create UI toast entries.
    pub fn notify_error(&mut self, error: &Error) -> Option<u64> {
        let entry = ToastEntry::from_error(self.next_id, error, std::time::Instant::now())?;
        let id = self.notify(&entry.title, &entry.message, entry.level, entry.duration_ms);
        Some(id)
    }

    /// Enqueue the error side of a Result as a toast and leave Ok values silent.
    pub fn notify_result_error<T>(&mut self, result: &Result<T>) -> Option<u64> {
        result
            .as_ref()
            .err()
            .and_then(|error| self.notify_error(error))
    }

    /// 发送通知（完整控制）。
    pub fn notify(
        &mut self,
        title: &str,
        message: &str,
        level: StatusLevel,
        duration_ms: u32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // 1. 发送平台通知（系统级弹窗/气球提示）
        if let Some(ref mut pn) = self.platform_notifier {
            if let Err(error) = pn.show(title, message) {
                self.diagnostics.report(error);
            }
        }

        // 2. 入队应用内 Toast
        let entry = ToastEntry {
            id,
            title: title.to_string(),
            message: message.to_string(),
            level,
            duration_ms,
            visible: true,
            created_at: std::time::Instant::now(),
        };
        self.toasts.push_back(entry);
        self.trim_excess();

        // 3. 通知 UI 层
        if let Some(ref cb) = self.on_change {
            cb();
        }

        tracing::info!("[Notification] {:?}: {} — {}", level, title, message);

        id
    }

    /// 关闭指定 ID 的 Toast（从队列中移除）。
    pub fn dismiss(&mut self, id: u64) {
        if let Some(pos) = self.toasts.iter().position(|t| t.id == id && t.visible) {
            self.toasts.remove(pos);
            if let Some(ref cb) = self.on_change {
                cb();
            }
        }
    }

    /// 关闭所有可见 Toast（清空队列）。
    pub fn dismiss_all(&mut self) {
        if self.has_active() {
            self.toasts.clear();
            if let Some(ref cb) = self.on_change {
                cb();
            }
        }
    }

    /// 移除过期 Toast（超时自动清理），返回当前可见列表。
    pub fn update(&mut self) -> Vec<ToastEntry> {
        self.update_at(std::time::Instant::now())
    }

    pub(crate) fn update_at(&mut self, now: std::time::Instant) -> Vec<ToastEntry> {
        self.toasts.retain(|t| {
            if t.duration_ms > 0
                && now.duration_since(t.created_at).as_millis() as u32 >= t.duration_ms
            {
                return false;
            }
            true
        });
        self.toasts.iter().cloned().collect()
    }

    /// 获取当前可见的 Toast 列表。
    pub fn visible_toasts(&self) -> Vec<ToastEntry> {
        self.toasts.iter().filter(|t| t.visible).cloned().collect()
    }

    /// 检查是否有活跃的 Toast。
    pub fn has_active(&self) -> bool {
        self.toasts.iter().any(|t| t.visible)
    }

    /// 移除超出 max_visible 数量的最旧 Toast。
    fn trim_excess(&mut self) {
        let visible_count = self.toasts.iter().filter(|t| t.visible).count();
        let excess = visible_count.saturating_sub(self.max_visible);
        if excess == 0 {
            return;
        }
        let mut removed = 0usize;
        self.toasts.retain(|t| {
            if removed < excess && t.visible {
                removed += 1;
                false
            } else {
                true
            }
        });
    }
}

impl NotificationSource for NotificationService {
    fn update_notifications(&mut self) -> Vec<ToastEntry> {
        self.update()
    }

    fn notify_error(&mut self, error: &Error) -> Option<u64> {
        NotificationService::notify_error(self, error)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════
