//! ButtonGroup — 视觉连体按钮组。
//!
//! 标记各按钮在组中的位置（左/中/右/单独），
//! 供 View 层 `row()` + gap(0.0) 组合时各按钮自行调整圆角。

use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::general::button::{Button, ButtonGroupPosition};

/// 视觉连体按钮组。将多个 Button 标记位置后返回，由 View 层组合布局。
pub struct ButtonGroup {
    buttons: Vec<Button>,
}

impl ButtonGroup {
    /// 创建尚未包含按钮的视觉连体按钮组。
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
        }
    }

    /// 设置由按钮组按位置标记的按钮列表。
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
        // Rust 只保留按钮行为需要的连体位置标记。
        let positioned_buttons = self
            .into_positioned_buttons()
            .into_iter()
            .map(ViewNode::leaf)
            .collect::<Vec<_>>();
        // 容器结构、横向排列与间距由 UIX 声明壳拥有。
        crate::uix!("src/ui/widgets/general/button_group/button_group.uix")
    }
}

impl From<ButtonGroup> for ViewNode {
    fn from(group: ButtonGroup) -> Self {
        group.build()
    }
}

// 集中验证 ButtonBuilder 与既有 ButtonGroupPosition 的桥接契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/general/button_group__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
