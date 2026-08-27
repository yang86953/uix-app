// 引入解析后的绘制颜色值。
use crate::draw::Color;
// 引入主题 token 契约。
use crate::ui::ThemeTokens;

// 图片与图片组可由 UIX 声明的主题语义颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::widgets::display) enum ImageColorRole {
    Mask,
    Overlay,
    Text,
    TextSecondary,
    BorderSecondary,
    FillTertiary,
    Container,
    Primary,
}

impl ImageColorRole {
    // 在当前组件主题作用域内解析 UIX 声明的语义颜色。
    pub(in crate::ui::widgets::display) fn resolve(self, tokens: &dyn ThemeTokens) -> Color {
        match self {
            Self::Mask => tokens.color_bg_mask(),
            Self::Overlay => tokens.color_bg_overlay(),
            Self::Text => tokens.color_text(),
            Self::TextSecondary => tokens.color_text_secondary(),
            Self::BorderSecondary => tokens.color_border_secondary(),
            Self::FillTertiary => tokens.color_fill_tertiary(),
            Self::Container => tokens.color_bg_container(),
            Self::Primary => tokens.color_primary(),
        }
    }
}

// 图片组件可由 UIX 声明的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::widgets::display) enum ImageFontRole {
    Large,
}

impl ImageFontRole {
    // 在当前组件主题作用域内解析 UIX 声明的字号角色。
    pub(in crate::ui::widgets::display) fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Large => tokens.font_size_lg(),
        }
    }
}

// 图片组件可由 UIX 声明的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::widgets::display) enum ImageRadiusRole {
    Body,
    Small,
}

impl ImageRadiusRole {
    // 在当前组件主题作用域内解析 UIX 声明的圆角角色。
    pub(in crate::ui::widgets::display) fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.border_radius(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由各自 UIX 文件声明的图片预览语义角色与紧凑字号比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::ui::widgets::display) struct ImageOverlayVisual {
    mask: ImageColorRole,
    surface: ImageColorRole,
    foreground: ImageColorRole,
    border: ImageColorRole,
    compact_font_midpoint_weight: f32,
}

// 组合 UIX 声明的图片预览语义角色。
pub(in crate::ui::widgets::display) const fn image_overlay_visual(
    mask: ImageColorRole,
    surface: ImageColorRole,
    foreground: ImageColorRole,
    border: ImageColorRole,
    compact_font_midpoint_weight: f32,
) -> ImageOverlayVisual {
    ImageOverlayVisual {
        mask,
        surface,
        foreground,
        border,
        compact_font_midpoint_weight,
    }
}

// 保存图片预览一次绘制内解析出的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::ui::widgets::display) struct ImageOverlayPalette {
    // 全窗口预览使用语义遮罩色。
    pub(in crate::ui::widgets::display) mask: Color,
    // 预览面板与控制按钮使用浮层背景色。
    pub(in crate::ui::widgets::display) surface: Color,
    // 浮层文字与图标使用主题正文色。
    pub(in crate::ui::widgets::display) foreground: Color,
    // 浮层弱边界使用次级边框色。
    pub(in crate::ui::widgets::display) border: Color,
    // 计数与不可用说明使用紧凑主题字号。
    pub(in crate::ui::widgets::display) compact_font: f32,
}

// 将 UIX 声明的主题角色转换为图片预览私有绘制值。
impl ImageOverlayPalette {
    // 从当前组件主题作用域解析一次调色板，供整个绘制批次复用。
    pub(in crate::ui::widgets::display) fn resolve(
        tokens: &dyn ThemeTokens,
        visual: ImageOverlayVisual,
    ) -> Self {
        // 读取相邻排版 token 以按 UIX 比例派生紧凑字号。
        let small = tokens.font_size_sm();
        let body = tokens.font_size();
        Self {
            mask: visual.mask.resolve(tokens),
            surface: visual.surface.resolve(tokens),
            foreground: visual.foreground.resolve(tokens),
            border: visual.border.resolve(tokens),
            compact_font: small
                + (body - small) * visual.compact_font_midpoint_weight.clamp(0.0, 1.0),
        }
    }
}
