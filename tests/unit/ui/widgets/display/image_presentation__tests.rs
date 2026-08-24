// 引入被测私有调色板、UIX 视觉角色与构造器。
use super::{ImageColorRole, ImageOverlayPalette, image_overlay_visual};
// 引入标准明暗主题。
use crate::ui::Theme;

// 预览遮罩、表面与前景必须直接来自当前主题。
#[test]
fn image_overlay_palette_follows_theme_tokens() {
    // 构造与同目录 UIX 声明一致的语义角色表。
    let visual = image_overlay_visual(
        ImageColorRole::Mask,
        ImageColorRole::Overlay,
        ImageColorRole::Text,
        ImageColorRole::BorderSecondary,
        0.5,
    );
    // 构造标准亮色主题。
    let light = Theme::antd_light();
    // 解析亮色预览调色板。
    let light_palette = ImageOverlayPalette::resolve(light.tokens(), visual);
    // 核对亮色遮罩 token。
    assert_eq!(light_palette.mask, light.tokens().color_bg_mask());
    // 核对亮色浮层表面 token。
    assert_eq!(light_palette.surface, light.tokens().color_bg_overlay());
    // 构造标准暗色主题。
    let dark = Theme::antd_dark();
    // 解析暗色预览调色板。
    let dark_palette = ImageOverlayPalette::resolve(dark.tokens(), visual);
    // 明暗主题的浮层表面必须不同。
    assert_ne!(light_palette.surface, dark_palette.surface);
    // 默认相邻字号中点保持原 13px。
    assert_eq!(light_palette.compact_font, 13.0);
}
