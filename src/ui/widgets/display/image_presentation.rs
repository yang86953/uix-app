// 引入解析后的绘制颜色值。
use crate::draw::Color;
// 引入主题 token 契约。
use crate::ui::ThemeTokens;

// 紧凑说明字号位于小号正文与正文 token 的中点。
const COMPACT_FONT_MIDPOINT_WEIGHT: f32 = 0.5;

// 保存图片预览一次绘制内解析出的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ImageOverlayPalette {
    // 全窗口预览使用语义遮罩色。
    pub(super) mask: Color,
    // 预览面板与控制按钮使用浮层背景色。
    pub(super) surface: Color,
    // 浮层文字与图标使用主题正文色。
    pub(super) foreground: Color,
    // 浮层弱边界使用次级边框色。
    pub(super) border: Color,
    // 计数与不可用说明使用紧凑主题字号。
    pub(super) compact_font: f32,
}

// 将主题 token 转换为图片预览私有绘制值。
impl ImageOverlayPalette {
    // 从当前组件主题作用域解析调色板。
    pub(super) fn resolve(tokens: &dyn ThemeTokens) -> Self {
        // 读取相邻排版 token 以派生原 13px 紧凑字号。
        let small = tokens.font_size_sm();
        // 读取主题正文字号。
        let body = tokens.font_size();
        // 返回供当前绘制批次复用的稳定值。
        Self {
            // 使用主题语义遮罩。
            mask: tokens.color_bg_mask(),
            // 使用主题浮层表面。
            surface: tokens.color_bg_overlay(),
            // 使用主题正文前景。
            foreground: tokens.color_text(),
            // 使用主题次级边框。
            border: tokens.color_border_secondary(),
            // 默认主题下保持 13px，同时随主题排版缩放。
            compact_font: small + (body - small) * COMPACT_FONT_MIDPOINT_WEIGHT,
        }
    }
}

// 验证图片浮层调色板随主题解析。
#[cfg(test)]
mod tests {
    // 引入被测私有调色板。
    use super::ImageOverlayPalette;
    // 引入标准明暗主题。
    use crate::ui::Theme;

    // 预览遮罩、表面与前景必须直接来自当前主题。
    #[test]
    fn image_overlay_palette_follows_theme_tokens() {
        // 构造标准亮色主题。
        let light = Theme::antd_light();
        // 解析亮色预览调色板。
        let light_palette = ImageOverlayPalette::resolve(light.tokens());
        // 核对亮色遮罩 token。
        assert_eq!(light_palette.mask, light.tokens().color_bg_mask());
        // 核对亮色浮层表面 token。
        assert_eq!(light_palette.surface, light.tokens().color_bg_overlay());
        // 构造标准暗色主题。
        let dark = Theme::antd_dark();
        // 解析暗色预览调色板。
        let dark_palette = ImageOverlayPalette::resolve(dark.tokens());
        // 明暗主题的浮层表面必须不同。
        assert_ne!(light_palette.surface, dark_palette.surface);
        // 默认相邻字号中点保持原 13px。
        assert_eq!(light_palette.compact_font, 13.0);
    }
}
