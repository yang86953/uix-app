//! UI 声明投影：已校验的 `UiNode` 树转换为 UIX `ViewNode` 子树。
//!
//! 投影是纯构建：事件闭包只向扩展队列发送 owned 事件，UI 线程不求值
//! Lisp；输入草稿按稳定 key 存于投影器，声明变更不覆盖在编辑草稿，
//! `(reset #t)` 显式重置。layout / paint / 命中路径无解释器参与。

use std::collections::BTreeMap;

use crate::ui::reactive::state::State;
use crate::ui::view::View;
use crate::ui::widgets::button;
use crate::ui::widgets::{column, label, row};
use crate::ui::widgets::combinators::input;

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

/// 投影器：持挂载位事件出口与输入草稿库，可跨重建复用。
pub struct UiProjector {
    extension: String,
    generation: u64,
    events: UiEventSender,
    drafts: BTreeMap<String, State<String>>,
    list_gap_hint: Option<f32>,
}

impl UiProjector {
    pub fn new(extension: &str, generation: u64, events: UiEventSender) -> Self {
        Self {
            extension: extension.to_string(),
            generation,
            events,
            drafts: BTreeMap::new(),
            list_gap_hint: Some(2.0),
        }
    }

    /// 投影整棵声明树；每个节点带 `automation_id`（扩展 / 挂载位 / key）。
    pub fn project(&mut self, mount: &str, node: &UiNode) -> crate::ui::view::ViewNode {
        let prefix = format!("{}/{}/{}", self.extension, mount, node.key());
        self.project_node(mount, node, &prefix)
    }

    fn project_node(
        &mut self,
        mount: &str,
        node: &UiNode,
        automation_prefix: &str,
    ) -> crate::ui::view::ViewNode {
        match node {
            UiNode::Column { key, padding, gap, background, children } => {
                let children: Vec<_> = children
                    .iter()
                    .map(|child| {
                        let prefix = format!("{automation_prefix}/{}", child.key());
                        self.project_node(mount, child, &prefix)
                    })
                    .collect();
                let mut view = column(children);
                if let Some(padding) = padding {
                    view = view.padding(*padding as f32);
                }
                if let Some(gap) = gap {
                    view = view.gap(*gap as f32);
                }
                if let Some(background) = background {
                    view = view.bg(background.as_str());
                }
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
            UiNode::Row { key, padding, gap, background, children } => {
                let children: Vec<_> = children
                    .iter()
                    .map(|child| {
                        let prefix = format!("{automation_prefix}/{}", child.key());
                        self.project_node(mount, child, &prefix)
                    })
                    .collect();
                let mut view = row(children);
                if let Some(padding) = padding {
                    view = view.padding(*padding as f32);
                }
                if let Some(gap) = gap {
                    view = view.gap(*gap as f32);
                }
                if let Some(background) = background {
                    view = view.bg(background.as_str());
                }
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
            UiNode::Text { key, content, color, size } => {
                let mut view = label(content.as_str());
                if let Some(color) = color {
                    view = view.color(color.as_str());
                }
                if let Some(size) = size {
                    view = view.font_size(*size as f32);
                }
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
            UiNode::Input { key, value, placeholder, on_change, reset } => {
                // 草稿保留：稳定 key 复用同一 State；仅显式 reset 重建。
                let draft = match reset {
                    true => {
                        let state = State::new(value.clone());
                        self.drafts.insert(key.clone(), state.clone());
                        state
                    }
                    false => match self.drafts.get(key) {
                        Some(existing) => existing.clone(),
                        None => {
                            let state = State::new(value.clone());
                            self.drafts.insert(key.clone(), state.clone());
                            state
                        }
                    },
                };
                let mut builder = input().value(&draft);
                if let Some(placeholder) = placeholder {
                    builder = builder.placeholder(placeholder.as_str());
                }
                if let Some(handler) = on_change {
                    let event = self.event_sender(handler.clone());
                    builder = builder.on_change(move |text| {
                        let _ = event.send(UiEventPayload::Change { text: text.to_string() });
                    });
                }
                let mut view = builder.build();
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
            UiNode::Button { key, label: text, on_click, disabled } => {
                let mut builder = button(text.as_str());
                if *disabled {
                    builder = builder.disabled(true);
                }
                if let Some(handler) = on_click {
                    let event = self.event_sender(handler.clone());
                    builder = builder.on_click_fn(move || {
                        let _ = event.send(UiEventPayload::Click);
                    });
                }
                let mut view = builder.build();
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
            UiNode::List { key, items } => {
                // 列表项无独立稳定身份：按位置协调，automation_id 带索引。
                let children: Vec<_> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let mut view = label(item.as_str());
                        view.automation_id = Some(format!("{automation_prefix}/{index}"));
                        view
                    })
                    .collect();
                let mut view = column(children);
                if let Some(gap) = self.list_gap_hint {
                    view = view.gap(gap);
                }
                view.key = Some(key.clone());
                view.automation_id = Some(automation_prefix.to_string());
                view
            }
        }
    }

    fn event_sender(&self, handler: String) -> EventForwarder {
        EventForwarder {
            extension: self.extension.clone(),
            generation: self.generation,
            handler,
            events: self.events.clone(),
        }
    }
}

/// 事件转发器：闭包侧只发送，不执行任何脚本。
#[derive(Clone)]
struct EventForwarder {
    extension: String,
    generation: u64,
    handler: String,
    events: UiEventSender,
}

impl EventForwarder {
    fn send(&self, payload: UiEventPayload) -> Result<(), String> {
        self.events
            .send(UiEvent {
                extension: self.extension.clone(),
                generation: self.generation,
                handler: self.handler.clone(),
                payload,
            })
            .map_err(|_| "扩展事件通道已关闭".to_string())
    }
}

// UiProjector 的构造后配置。
impl UiProjector {
    /// 配置列表项间距（默认 2.0）。
    pub fn with_list_gap(mut self, gap: f32) -> Self {
        self.list_gap_hint = Some(gap);
        self
    }

    /// 热替换后切换事件代际；草稿库与挂载位保持，兼容输入沿合同保留。
    pub fn update_generation(&mut self, generation: u64) {
        self.generation = generation;
    }
}
