//! ButtonGroup — 视觉连体按钮组。
//!
//! 标记各按钮在组中的位置（左/中/右/单独），
//! 供 View 层 `row()` + gap(0.0) 组合时各按钮自行调整圆角。

use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::general::button::{Button, ButtonGroupPosition};
use crate::ui::widgets::row;

/// 视觉连体按钮组。将多个 Button 标记位置后返回，由 View 层组合布局。
pub struct ButtonGroup {
    buttons: Vec<Button>,
}

impl ButtonGroup {
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
        }
    }

    pub fn buttons(mut self, buttons: Vec<Button>) -> Self {
        self.buttons = buttons;
        self
    }

    /// 返回经过位置标记的按钮列表。
    pub fn into_positioned_buttons(self) -> Vec<Button> {
        let len = self.buttons.len();
        self.buttons
            .into_iter()
            .enumerate()
            .map(|(i, btn)| {
                let pos = if len <= 1 {
                    ButtonGroupPosition::Single
                } else if i == 0 {
                    ButtonGroupPosition::Left
                } else if i == len - 1 {
                    ButtonGroupPosition::Right
                } else {
                    ButtonGroupPosition::Middle
                };
                btn.group_position(pos)
            })
            .collect()
    }
}

impl Default for ButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ButtonGroup {
    fn build(self) -> ViewNode {
        let children = self
            .into_positioned_buttons()
            .into_iter()
            .map(ViewNode::leaf)
            .collect::<Vec<_>>();
        row(children).gap(0.0)
    }
}

impl From<ButtonGroup> for ViewNode {
    fn from(group: ButtonGroup) -> Self {
        group.build()
    }
}

// 集中验证 ButtonBuilder 与既有 ButtonGroupPosition 的桥接契约。
#[cfg(test)]
mod tests {
    // 引入当前模块公开的按钮组位置类型。
    use super::*;
    // 引入按钮构建器入口与组件快照字段。
    use crate::ui::{SnapshotFields, button};

    // 验证构建器物化时同时保留连体位置与事件处理器。
    #[test]
    fn button_builder_group_position_preserves_button_contract() {
        // 构造带点击处理器的中间位置按钮。
        let node = button("Middle")
            // 先登记按钮点击处理器。
            .on_click_fn(|| {})
            // 再把按钮物化为连体中间节点。
            .group_position(ButtonGroupPosition::Middle);
        // 物化不能丢失已登记的点击处理器。
        assert_eq!(node.handlers.len(), 1);
        // 底层组件快照必须记录中间位置。
        match node.widget.snapshot_fields() {
            // 检查 Button 快照中的连体位置。
            SnapshotFields::Button { group_position, .. } => {
                // 位置必须与构建器输入一致。
                assert_eq!(group_position, Some(ButtonGroupPosition::Middle));
            }
            // 其他组件类型表示构建器物化契约被破坏。
            _ => panic!("ButtonBuilder::group_position 必须生成 Button 节点"),
        }
    }
}
