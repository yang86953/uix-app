//! 消息条目与挂载句柄。

use crate::platform::capabilities::StatusLevel;
use crate::ui::widgets::feedback::toast_motion::ToastQueue;

// 默认展示时长直接读取同目录 UIX 生成的唯一视觉静态项。
use super::MESSAGE_VISUAL_REF;
// 引入声明条目关闭原因。
use crate::ui::widgets::feedback::declaration::FeedbackCloseReason;

#[derive(Debug, Clone)]
/// 可加入全局提示队列的消息配置。
pub struct MessageItem {
    /// 决定图标和颜色的提示状态级别。
    pub type_: StatusLevel,
    /// 提示正文。
    pub content: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
    pub duration_ms: u64,
    /// 是否显示并响应关闭按钮。
    pub closable: bool,
}

/// 向已挂载的 [`Message`] 队列增删提示，不暴露内部动画与定时器。
#[derive(Clone)]
pub struct MessageHandle {
    pub(super) queue: ToastQueue<MessageItem>,
}

impl MessageHandle {
    /// 创建独立消息队列句柄，所有权由窗口反馈状态接管。
    pub(crate) fn new() -> Self {
        // 每个窗口只创建一次队列，克隆句柄仍指向同一状态。
        Self {
            // 使用反馈模块现有的稳定 ID 队列实现。
            queue: ToastQueue::new(),
        }
    }

    /// 添加提示并返回稳定 ID。
    pub fn add(&self, item: MessageItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    // 首次写入带关闭观察器的 keyed 声明条目。
    pub(crate) fn push_declaration<F>(&self, id: u64, item: MessageItem, on_close: F)
    where
        // Host 可能在任意 UI 生命周期边界调用观察器。
        F: Fn(FeedbackCloseReason) + Send + Sync + 'static,
    {
        // 保存时长供队列调度。
        let duration_ms = item.duration_ms;
        // 外部高位 ID 与命令式本地 ID 分离。
        self.queue
            .push_external_with_close(id, item, duration_ms, std::sync::Arc::new(on_close));
    }

    // 幂等更新 keyed 声明条目。
    pub(crate) fn update_declaration(&self, id: u64, item: MessageItem) {
        // 保存时长供队列判断是否重启计时。
        let duration_ms = item.duration_ms;
        // 保留现有关闭观察器并替换配置。
        self.queue.update_external(id, item, duration_ms);
    }

    // 声明卸载时无关闭事实地释放条目。
    pub(crate) fn release_declaration(&self, id: u64) -> bool {
        // 释放外部稳定 key 与观察器。
        self.queue.release_external(id)
    }

    // 由 owner 或聚焦测试按程序化原因关闭声明条目。
    #[cfg(test)]
    pub(crate) fn close_declaration(&self, id: u64) -> bool {
        // 外部声明 ID 的关闭观察器会建立 tombstone。
        self.queue.remove_external(id)
    }

    /// 添加使用标准时长的成功提示并返回稳定标识。
    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Success,
            content: content.into(),
            duration_ms: MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Success),
            closable: true,
        })
    }

    /// 添加使用标准时长的信息提示并返回稳定标识。
    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.into(),
            duration_ms: MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Info),
            closable: true,
        })
    }

    /// 添加使用标准时长的警告提示并返回稳定标识。
    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Warning,
            content: content.into(),
            duration_ms: MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Warning),
            closable: true,
        })
    }

    /// 添加使用标准时长的错误提示并返回稳定标识。
    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Error,
            content: content.into(),
            duration_ms: MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Error),
            closable: true,
        })
    }

    /// 请求移除指定提示；离场动画完成后才从画面消失。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除全部提示。
    pub fn clear(&self) {
        self.queue.clear();
    }

    /// 返回当前队列中提示项的值快照。
    pub fn items(&self) -> Vec<MessageItem> {
        self.queue.values()
    }

    /// 返回当前队列中的提示项数量。
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 返回当前队列是否没有提示项。
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
