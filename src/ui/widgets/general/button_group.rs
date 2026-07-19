//! ButtonGroup — 视觉连体按钮组。
//!
//! 标记各按钮在组中的位置（左/中/右/单独），
//! 供 View 层 `row()` + gap(0.0) 组合时各按钮自行调整圆角。

use crate::ui::widgets::general::button::{Button, ButtonGroupPosition};

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
