//! UI 声明投影：已校验的 `UiNode` 树转换为 UIX `ViewNode` 子树。
//!
//! 投影是纯构建：事件闭包只向扩展队列发送 owned 事件，UI 线程不求值
//! Lisp；输入草稿按稳定 key 存于投影器，声明变更不覆盖在编辑草稿，
//! `(reset #t)` 显式重置。layout / paint / 命中路径无解释器参与。

use super::ui_declare::UiNode;

/// 挂载位事件的负载。
#[derive(Debug, Clone, PartialEq)]
pub enum UiEventPayload {
    Click,
    Change { text: String },
    Submit { text: String },
}

/// 投影闭包发出的事件：携带扩展身份、提交代与处理名。
#[derive(Debug, Clone)]
pub struct UiEvent {
    pub extension: String,
    pub generation: u64,
    pub handler: String,
    pub payload: UiEventPayload,
}

/// 事件出口：投影闭包持有，向扩展执行线程投递；只发送不执行。
#[derive(Clone)]
pub struct UiEventSender {
    pub(crate) inner: std::sync::mpsc::Sender<super::worker::WorkerCommand>,
}

impl UiEventSender {
    /// 投递面板事件（只发送；由扩展执行线程串行消费）。
    pub fn send(&self, event: UiEvent) -> Result<(), String> {
        self.inner
            .send(super::worker::WorkerCommand::Event(event))
            .map_err(|_| "扩展事件通道已关闭".to_string())
    }
}

/// 声明提交后经宿主线程回投到 UI 的更新。
#[derive(Debug, Clone, PartialEq)]
pub enum UiUpdate {
    /// 新声明已通过校验并计为下一修订；应用与否由挂载位回执确认。
    Applied {
        mount: String,
        extension: String,
        generation: u64,
        revision: u64,
        node: UiNode,
    },
    /// 声明被拒绝：原树保留，原因随回执交付。
    Rejected {
        mount: String,
        extension: String,
        reason: String,
    },
}
