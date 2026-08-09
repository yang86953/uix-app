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

    /// 把复合输入内部的文本框绑定到外部 `State<String>`。
    pub fn value(mut self, state: &State<String>) -> Self {
        // 复用已配置 Input，缺失时创建默认文本输入。
        let input = self.input.take().unwrap_or_else(|| Input::new(""));
        // 绑定后重新保存唯一内部输入实例。
        self.input = Some(input.value(state));
        // 返回可继续配置附加文本的组合。
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

// 验证 InputGroup 附加文本与状态绑定组合契约。
#[cfg(test)]
mod tests {
    // 引入当前组合构建器。
    use super::*;
    // 引入 Input 快照读取契约。
    use crate::ui::{SnapshotFields, SnapshotSource};

    // 验证附加文本和双向状态落到同一个 Input。
    #[test]
    fn value_and_addons_share_one_input_contract() {
        // 创建复合输入的外部文本状态。
        let amount = State::new(String::from("12.50"));
        // 构造完整前后附加文本组合并取出内部输入。
        let input = InputGroup::new()
            // 设置货币前缀。
            .addon_before("¥")
            // 绑定文本值。
            .value(&amount)
            // 设置单位后缀。
            .addon_after("元")
            // 物化唯一内部输入。
            .into_input();
        // 内部输入必须读取绑定状态的当前值。
        assert_eq!(input.current_value(), "12.50");
        // 读取输入公开快照。
        let SnapshotFields::Input {
            // 提取前置附加文本。
            addon_before,
            // 提取后置附加文本。
            addon_after,
            // 忽略其他输入配置。
            ..
        } = input.snapshot_fields()
        else {
            // 快照类型不匹配表示组合边界破坏。
            panic!("InputGroup 必须物化为 Input 快照");
        };
        // 两侧附加文本必须完整保留。
        assert_eq!(addon_before, "¥");
        // 后置单位必须完整保留。
        assert_eq!(addon_after, "元");
    }
}
