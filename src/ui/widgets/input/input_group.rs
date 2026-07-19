//! InputGroup — 带前后文本附加项的输入组合。

use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::Input;

/// 将前置文本、输入框与后置文本组合为一个可直接声明的 View。
///
/// 附加项复用 `Input` 的单一绘制与命中区域，不引入额外 Widget 层级。
pub struct InputGroup {
    addon_before: String,
    input: Option<Input>,
    addon_after: String,
}

impl InputGroup {
    pub fn new() -> Self {
        Self {
            addon_before: String::new(),
            input: None,
            addon_after: String::new(),
        }
    }

    pub fn addon_before(mut self, text: impl Into<String>) -> Self {
        self.addon_before = text.into();
        self
    }

    pub fn input(mut self, input: Input) -> Self {
        self.input = Some(input);
        self
    }

    pub fn addon_after(mut self, text: impl Into<String>) -> Self {
        self.addon_after = text.into();
        self
    }

    pub fn into_input(self) -> Input {
        self.input
            .unwrap_or_else(|| Input::new(""))
            .addon_before(&self.addon_before)
            .addon_after(&self.addon_after)
    }
}

impl Default for InputGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl View for InputGroup {
    fn build(self) -> ViewNode {
        ViewNode::leaf(self.into_input())
    }
}

impl From<InputGroup> for ViewNode {
    fn from(group: InputGroup) -> Self {
        group.build()
    }
}
