//! FloatButtonBackTop 的回到顶部预设外观。

use super::FloatButton;

// 保存回到顶部便捷封装的图标与提示文字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FloatButtonBackTopVisual {
    icon: &'static str,
    tooltip: &'static str,
}

crate::uix_items!("src/ui/widgets/general/float_button/back_top/back_top.uix");

/// 回到顶部按钮的便捷封装。
pub struct FloatButtonBackTop;

impl FloatButtonBackTop {
    #[allow(clippy::new_ret_no_self)]
    /// 创建使用 UIX 图标与提示文案的浮动按钮。
    pub fn new() -> FloatButton {
        FloatButton::new(FLOAT_BUTTON_BACK_TOP_VISUAL_REF.icon)
            .tooltip(FLOAT_BUTTON_BACK_TOP_VISUAL_REF.tooltip)
    }
}
