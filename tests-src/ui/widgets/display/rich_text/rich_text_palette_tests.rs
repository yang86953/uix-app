//! `src/ui/widgets/display/rich_text/rich_text_palette.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl RichTextPalette） ——

impl RichTextPalette {
    // 从当前主题作用域投影真实绘制颜色。
    #[cfg(test)]
    pub(crate) fn from_tokens(default_text: Color, tokens: &dyn ThemeTokens) -> Self {
        // 只读取语义 token，不缓存或拥有主题实例。
        Self {
            // 普通文本保留显式颜色优先级解析结果。
            default_text,
            // 代码文字使用主题正文色。
            code_text: tokens.color_text(),
            // 代码底色使用主题次级填充色。
            code_background: tokens.color_fill_secondary(),
            // 链接使用主题链接色。
            link: tokens.color_link(),
        }
    }
}
