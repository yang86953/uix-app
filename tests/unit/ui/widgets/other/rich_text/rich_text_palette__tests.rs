// 引入被测调色板。
use super::RichTextPalette;
// 引入颜色和主题 token 实现。
use crate::draw::Color;
// 引入颜色 token 查询契约。
use crate::ui::theme::IColorTokens;
// 引入默认主题实现。
use crate::ui::theme::DesignTokens;

// 主题调色板必须逐项读取当前 token。
#[test]
fn themed_palette_uses_semantic_tokens() {
    // 使用暗色主题覆盖与旧固定浅色值不同的路径。
    let tokens = DesignTokens::antd_dark();
    // 显式正文色仍应拥有最高优先级。
    let default_text = Color::from_rgb(1, 2, 3);
    // 从当前主题构造布局调色板。
    let palette = RichTextPalette::from_tokens(default_text, &tokens);
    // 普通正文必须保留已解析显式色。
    assert_eq!(palette.default_text, default_text);
    // 代码文字必须跟随主题正文色。
    assert_eq!(palette.code_text, tokens.color_text());
    // 代码背景必须跟随主题次级填充色。
    assert_eq!(palette.code_background, tokens.color_fill_secondary());
    // 链接必须跟随主题链接色。
    assert_eq!(palette.link, tokens.color_link());
}

// 主题切换必须改变缓存比较使用的完整调色板。
#[test]
fn theme_change_changes_palette_cache_key() {
    // 使用同一个显式正文色隔离其余主题 token 的变化。
    let default_text = Color::from_rgb(1, 2, 3);
    // 构造浅色主题调色板。
    let light = RichTextPalette::from_tokens(default_text, &DesignTokens::antd_light());
    // 构造暗色主题调色板。
    let dark = RichTextPalette::from_tokens(default_text, &DesignTokens::antd_dark());
    // 完整值比较必须观察到代码或链接语义色变化。
    assert_ne!(light, dark);
}
