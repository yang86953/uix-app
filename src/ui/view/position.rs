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
    pub fn top(mut self, value: f32) -> Self {
        // 复用公开值对象的有限数值验证。
        self.position.insets = self.position.insets.top(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点右边定位值。
    pub fn right(mut self, value: f32) -> Self {
        // 复用公开值对象的有限数值验证。
        self.position.insets = self.position.insets.right(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点底边定位值。
    pub fn bottom(mut self, value: f32) -> Self {
        // 复用公开值对象的有限数值验证。
        self.position.insets = self.position.insets.bottom(value);
        // 返回可继续链式声明的节点。
        self
    }

    // 设置节点左边定位值。
    pub fn left(mut self, value: f32) -> Self {
        // 复用公开值对象的有限数值验证。
        self.position.insets = self.position.insets.left(value);
        // 返回可继续链式声明的节点。
        self
    }
}
