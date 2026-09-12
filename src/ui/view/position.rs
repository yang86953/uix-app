// 引入声明节点与公开定位值。
use super::ViewNode;
// 引入 UI System 自有的定位契约。
use crate::ui::{PositionInsets, PositionMode};

impl ViewNode {
    // 设置节点的布局定位模式。
    pub fn position(mut self, mode: PositionMode) -> Self {
        // 保留既有四边值并只替换模式。
        self.position.mode = mode;
        // 返回可继续链式声明的节点。
        self
    }

    // 一次性设置节点的四边定位值。
    pub fn position_insets(mut self, insets: PositionInsets) -> Self {
        // 保存已经验证为有限像素或 auto 的值。
        self.position.insets = insets;
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点顶边定位值。
    pub fn top(mut self, value: impl Into<crate::ui::theme::style::StyleLength>) -> Self {
        // 复用公开值对象的有限数值验证；接受 px 数值或保留单位的 StyleLength。
        self.position.insets = self.position.insets.top(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点右边定位值。
    pub fn right(mut self, value: impl Into<crate::ui::theme::style::StyleLength>) -> Self {
        // 复用公开值对象的有限数值验证；接受 px 数值或保留单位的 StyleLength。
        self.position.insets = self.position.insets.right(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点底边定位值。
    pub fn bottom(mut self, value: impl Into<crate::ui::theme::style::StyleLength>) -> Self {
        // 复用公开值对象的有限数值验证；接受 px 数值或保留单位的 StyleLength。
        self.position.insets = self.position.insets.bottom(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点左边定位值。
    pub fn left(mut self, value: impl Into<crate::ui::theme::style::StyleLength>) -> Self {
        // 复用公开值对象的有限数值验证；接受 px 数值或保留单位的 StyleLength。
        self.position.insets = self.position.insets.left(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 只把顶边定位恢复为 auto，并保留其他三边。
    #[doc(hidden)]
    pub fn top_auto(mut self) -> Self {
        // 清除顶边显式像素值以支持 UIX 差异级联。
        self.position.insets = self.position.insets.auto_top();
        // 返回可继续链式声明的节点。
        self
    }

    // 只把右边定位恢复为 auto，并保留其他三边。
    #[doc(hidden)]
    pub fn right_auto(mut self) -> Self {
        // 清除右边显式像素值以支持 UIX 差异级联。
        self.position.insets = self.position.insets.auto_right();
        // 返回可继续链式声明的节点。
        self
    }

    // 只把底边定位恢复为 auto，并保留其他三边。
    #[doc(hidden)]
    pub fn bottom_auto(mut self) -> Self {
        // 清除底边显式像素值以支持 UIX 差异级联。
        self.position.insets = self.position.insets.auto_bottom();
        // 返回可继续链式声明的节点。
        self
    }

    // 只把左边定位恢复为 auto，并保留其他三边。
    #[doc(hidden)]
    pub fn left_auto(mut self) -> Self {
        // 清除左边显式像素值以支持 UIX 差异级联。
        self.position.insets = self.position.insets.auto_left();
        // 返回可继续链式声明的节点。
        self
    }
}
