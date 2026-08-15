//! 实际树节点的文字选择声明与 used-value 元数据。

// 引入父模块运行时节点类型。
use super::super::BoxedWidget;
// 引入 UI System 公开选择策略值。
use crate::ui::UserSelect;

// 集中实现不属于组件本体的文字选择元数据访问。
impl BoxedWidget {
    // 返回当前节点自己的文字选择声明。
    pub(crate) fn declared_user_select(&self) -> UserSelect {
        // 复制闭合小型枚举，避免暴露内部存储。
        self.declared_user_select
    }

    // 替换当前节点自己的文字选择声明。
    pub(crate) fn set_declared_user_select(&mut self, value: UserSelect) {
        // used-value 由 WidgetTree 在同一协调步骤单独更新。
        self.declared_user_select = value;
    }

    // 返回当前节点结合祖先边界后的最终选择策略。
    pub(crate) fn effective_user_select(&self) -> UserSelect {
        // 复制闭合小型枚举供事件与选择协调查询。
        self.effective_user_select
    }

    // 替换当前节点结合祖先边界后的最终选择策略。
    pub(crate) fn set_effective_user_select(&mut self, value: UserSelect) {
        // 组件私有状态由 UI System 的文字选择适配边界同步。
        self.effective_user_select = value;
    }
}
