//! InputGroup — 带前后文本附加项的输入组合。

use crate::ui::view::{View, ViewNode};
// 引入复合输入公开双向绑定使用的文本状态。
use crate::ui::State;
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
    /// 创建没有附加文本且尚未配置内部输入框的组合。
    pub fn new() -> Self {
        Self {
            addon_before: String::new(),
            input: None,
            addon_after: String::new(),
        }
    }

    /// 设置绘制在输入内容前方的附加文本。
    pub fn addon_before(mut self, text: impl Into<String>) -> Self {
        self.addon_before = text.into();
        self
    }

    /// 设置组合唯一拥有的内部输入框及其既有配置。
    pub fn input(mut self, input: Input) -> Self {
        self.input = Some(input);
        self
    }

    /// 把复合输入内部的文本框绑定到外部 `State<String>`。
    pub fn value(mut self, state: &State<String>) -> Self {
        // 复用已配置 Input，缺失时创建默认文本输入。
        let input = self.input.take().unwrap_or_else(|| Input::new(""));
        // 绑定后重新保存唯一内部输入实例。
        self.input = Some(input.value(state));
        // 返回可继续配置附加文本的组合。
        self
    }

    /// 设置绘制在输入内容后方的附加文本。
    pub fn addon_after(mut self, text: impl Into<String>) -> Self {
        self.addon_after = text.into();
        self
    }

    /// 消费组合并把两侧附加文本应用到唯一内部输入框。
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
        // Rust 只负责把附加文本和状态绑定融合进唯一 Input 内核。
        let grouped_input = ViewNode::leaf(self.into_input());
        // UIX 明确声明此组合不增加另一层视觉容器。
        crate::uix!("src/ui/widgets/input/input_group/input_group.uix")
    }
}

impl From<InputGroup> for ViewNode {
    fn from(group: InputGroup) -> Self {
        group.build()
    }
}

// 验证 InputGroup 附加文本与状态绑定组合契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/input/input_group__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
