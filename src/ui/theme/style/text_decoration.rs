// 定义 UI System 拥有的闭合文本装饰值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration {
    // 不绘制文本装饰线，并可显式覆盖继承值。
    #[default]
    None,
    // 在每个视觉行的文字下缘绘制下划线。
    Underline,
    // 在每个视觉行的文字上缘绘制上划线。
    Overline,
    // 在每个视觉行的文字中部绘制删除线。
    LineThrough,
}

// 验证显式默认值在样式合并中仍能覆盖继承值。
#[cfg(test)]
mod tests {
    // 引入文本装饰和统一样式契约。
    use super::{super::Style, TextDecoration};

    // 显式 none 必须能够覆盖继承的下划线。
    #[test]
    fn text_decoration_explicit_none_overrides_inherited_underline() {
        // 构造带下划线的基础样式。
        let base = Style::default().with_text_decoration(TextDecoration::Underline);
        // 构造显式关闭文本装饰的覆盖样式。
        let overlay = Style::default().with_text_decoration(TextDecoration::None);
        // 合并后必须保留显式默认值。
        let merged = base.apply(overlay);
        // 有效装饰必须变为 none。
        assert_eq!(merged.effective_text_decoration(), TextDecoration::None);
    }
}
