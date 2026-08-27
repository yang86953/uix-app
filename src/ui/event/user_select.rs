//! UI System 公开的文字选择策略值契约。

// 定义所有 View 节点都可声明的闭合文字选择策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UserSelect {
    // 保留组件默认能力，并沿用祖先已经解析出的约束。
    #[default]
    Auto,
    // 禁止当前节点子树建立普通文字选区。
    None,
    // 为支持文字选择的当前节点子树启用普通拖选。
    Text,
    // 把最近声明 all 的文本子树作为整体选择单元。
    All,
}

// 提供树运行时所需的父子 used-value 解析规则。
impl UserSelect {
    // 按祖先有效值解析当前声明的最终选择策略。
    pub(crate) fn resolve_with_parent(self, parent: Self) -> Self {
        // none 和 all 形成不可由后代局部声明拆开的子树边界。
        match parent {
            // 祖先禁止选择时所有后代继续禁止。
            Self::None => Self::None,
            // 祖先整体选择时所有后代仍属于该整体。
            Self::All => Self::All,
            // 根或无约束祖先直接使用当前声明。
            Self::Auto => self,
            // text 祖先只在后代保持 auto 时继续传播。
            Self::Text => match self {
                // auto 沿用祖先已启用的文本选择。
                Self::Auto => Self::Text,
                // 显式 none、text 或 all 建立新的当前策略。
                value => value,
            },
        }
    }

    // 判断最终策略是否允许组件建立普通文字选区。
    pub(crate) fn allows_text(self, widget_default: bool) -> bool {
        // auto 保留组件默认，其余三值直接表达允许或禁止。
        match self {
            // auto 不篡改 Label 或 RichText 的显式构建器能力。
            Self::Auto => widget_default,
            // none 始终禁止普通文字选择。
            Self::None => false,
            // text 显式启用普通文字选择。
            Self::Text => true,
            // all 需要先允许参与者建立完整范围。
            Self::All => true,
        }
    }
}
