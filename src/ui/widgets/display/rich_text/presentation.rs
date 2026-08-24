//! RichText 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

use super::RichTextPalette;

// 保存 UIX 声明的默认字号与布局缓存阈值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextDefaultsVisual {
    pub(crate) font_size: f32,
    pub(crate) unconstrained_width: f32,
    pub(crate) relayout_epsilon: f32,
    pub(crate) measurement_dpi: f32,
}

// 保存字体、行高、代码与缺字估算共同消费的排版比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextMetricsVisual {
    pub(crate) line_height_factor: f32,
    pub(crate) code_font_scale: f32,
    pub(crate) code_baseline_offset: f32,
    pub(crate) italic_shear: f32,
    pub(crate) bold_offset: f32,
    pub(crate) space_advance: f32,
    pub(crate) tab_advance: f32,
    pub(crate) wide_advance: f32,
    pub(crate) narrow_advance: f32,
    pub(crate) cjk_advance: f32,
    pub(crate) cjk_punctuation_advance: f32,
    pub(crate) default_advance: f32,
}

// 保存字形背景、选择、链接状态与文字装饰几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextDecorationVisual {
    pub(crate) background_padding: f32,
    pub(crate) background_radius: f32,
    pub(crate) selection_alpha: u8,
    pub(crate) link_pressed_alpha: u8,
    pub(crate) link_active_alpha: u8,
    pub(crate) inactive_decoration_alpha: u8,
    pub(crate) underline_ratio: f32,
    pub(crate) strikethrough_ratio: f32,
    pub(crate) stroke: f32,
}

// 保存代码复制按钮的尺寸、图标与交互外观。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextCopyVisual {
    pub(crate) trailing_gap: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) radius: f32,
    pub(crate) icon_size: f32,
    pub(crate) icon_height_ratio: f32,
    pub(crate) icon: &'static str,
}

// 保存 Markdown 主题分隔线的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextBreakVisual {
    pub(crate) horizontal_inset: f32,
    pub(crate) stroke: f32,
}

// 保存内联图片占位、替代文本与派生图尺寸上限。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextImageVisual {
    pub(crate) border_stroke: f32,
    pub(crate) min_alt_width: f32,
    pub(crate) min_alt_height: f32,
    pub(crate) alt_font_size: f32,
    pub(crate) alt_height_ratio: f32,
    pub(crate) max_device_extent: f32,
}

// 保存由 UIX 声明的主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RichTextPaletteVisual {
    theme_text: ColorValue,
    code_text: ColorValue,
    code_background: ColorValue,
    link: ColorValue,
    primary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    border_secondary: ColorValue,
    estimated_code_background_alpha: u8,
}

// 全部 RichText 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextVisual {
    pub(crate) defaults: RichTextDefaultsVisual,
    pub(crate) metrics: RichTextMetricsVisual,
    pub(crate) decoration: RichTextDecorationVisual,
    pub(crate) copy: RichTextCopyVisual,
    pub(crate) thematic_break: RichTextBreakVisual,
    pub(crate) image: RichTextImageVisual,
    palette: RichTextPaletteVisual,
}

// 保存 RichText 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedRichTextVisual {
    pub(crate) layout_palette: RichTextPalette,
    pub(crate) primary: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) border_secondary: Color,
}

impl RichTextVisual {
    pub(crate) fn estimated_palette(self, default_text: Color) -> RichTextPalette {
        RichTextPalette {
            default_text,
            code_text: default_text,
            code_background: default_text.with_alpha(self.palette.estimated_code_background_alpha),
            link: default_text,
        }
    }

    pub(crate) fn resolve(
        self,
        explicit_default: Color,
        use_theme_color: bool,
        tokens: &dyn ThemeTokens,
    ) -> ResolvedRichTextVisual {
        let default_text = if use_theme_color {
            self.palette.theme_text.resolve(tokens)
        } else {
            explicit_default
        };
        ResolvedRichTextVisual {
            layout_palette: RichTextPalette {
                default_text,
                code_text: self.palette.code_text.resolve(tokens),
                code_background: self.palette.code_background.resolve(tokens),
                link: self.palette.link.resolve(tokens),
            },
            primary: self.palette.primary.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            border_secondary: self.palette.border_secondary.resolve(tokens),
        }
    }
}

pub(crate) const fn rich_text_defaults_visual(
    font_size: f32,
    unconstrained_width: f32,
    relayout_epsilon: f32,
    measurement_dpi: f32,
) -> RichTextDefaultsVisual {
    RichTextDefaultsVisual {
        font_size,
        unconstrained_width,
        relayout_epsilon,
        measurement_dpi,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_metrics_visual(
    line_height_factor: f32,
    code_font_scale: f32,
    code_baseline_offset: f32,
    italic_shear: f32,
    bold_offset: f32,
    space_advance: f32,
    tab_advance: f32,
    wide_advance: f32,
    narrow_advance: f32,
    cjk_advance: f32,
    cjk_punctuation_advance: f32,
    default_advance: f32,
) -> RichTextMetricsVisual {
    RichTextMetricsVisual {
        line_height_factor,
        code_font_scale,
        code_baseline_offset,
        italic_shear,
        bold_offset,
        space_advance,
        tab_advance,
        wide_advance,
        narrow_advance,
        cjk_advance,
        cjk_punctuation_advance,
        default_advance,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_decoration_visual(
    background_padding: f32,
    background_radius: f32,
    selection_alpha: f32,
    link_pressed_alpha: f32,
    link_active_alpha: f32,
    inactive_decoration_alpha: f32,
    underline_ratio: f32,
    strikethrough_ratio: f32,
    stroke: f32,
) -> RichTextDecorationVisual {
    RichTextDecorationVisual {
        background_padding,
        background_radius,
        selection_alpha: selection_alpha as u8,
        link_pressed_alpha: link_pressed_alpha as u8,
        link_active_alpha: link_active_alpha as u8,
        inactive_decoration_alpha: inactive_decoration_alpha as u8,
        underline_ratio,
        strikethrough_ratio,
        stroke,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_copy_visual(
    trailing_gap: f32,
    width: f32,
    height: f32,
    radius: f32,
    icon_size: f32,
    icon_height_ratio: f32,
    icon: &'static str,
) -> RichTextCopyVisual {
    RichTextCopyVisual {
        trailing_gap,
        width,
        height,
        radius,
        icon_size,
        icon_height_ratio,
        icon,
    }
}

pub(crate) const fn rich_text_break_visual(
    horizontal_inset: f32,
    stroke: f32,
) -> RichTextBreakVisual {
    RichTextBreakVisual {
        horizontal_inset,
        stroke,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_image_visual(
    border_stroke: f32,
    min_alt_width: f32,
    min_alt_height: f32,
    alt_font_size: f32,
    alt_height_ratio: f32,
    max_device_extent: f32,
) -> RichTextImageVisual {
    RichTextImageVisual {
        border_stroke,
        min_alt_width,
        min_alt_height,
        alt_font_size,
        alt_height_ratio,
        max_device_extent,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_palette_visual(
    theme_text: ColorValue,
    code_text: ColorValue,
    code_background: ColorValue,
    link: ColorValue,
    primary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    border_secondary: ColorValue,
    estimated_code_background_alpha: f32,
) -> RichTextPaletteVisual {
    RichTextPaletteVisual {
        theme_text,
        code_text,
        code_background,
        link,
        primary,
        fill_secondary,
        fill_tertiary,
        text_secondary,
        text_quaternary,
        border_secondary,
        estimated_code_background_alpha: estimated_code_background_alpha as u8,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn rich_text_visual(
    defaults: RichTextDefaultsVisual,
    metrics: RichTextMetricsVisual,
    decoration: RichTextDecorationVisual,
    copy: RichTextCopyVisual,
    thematic_break: RichTextBreakVisual,
    image: RichTextImageVisual,
    palette: RichTextPaletteVisual,
) -> RichTextVisual {
    RichTextVisual {
        defaults,
        metrics,
        decoration,
        copy,
        thematic_break,
        image,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的静态文案与主题角色。
pub(crate) const fn rich_text_copy_icon() -> &'static str {
    "copy"
}
pub(crate) const fn rich_text_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn rich_text_fill_secondary_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn rich_text_fill_tertiary_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn rich_text_link_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Link)
}
pub(crate) const fn rich_text_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn rich_text_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn rich_text_quaternary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn rich_text_secondary_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

pub(crate) static DEFAULT_RICH_TEXT_VISUAL: RichTextVisual = rich_text_visual(
    rich_text_defaults_visual(14.0, 400.0, 0.5, 96.0),
    rich_text_metrics_visual(
        1.5, 0.9, 2.0, 0.18, 0.6, 0.35, 2.0, 0.7, 0.3, 1.0, 0.9, 0.55,
    ),
    rich_text_decoration_visual(2.0, 3.0, 64.0, 48.0, 24.0, 180.0, 0.95, 0.52, 1.0),
    rich_text_copy_visual(4.0, 24.0, 16.0, 3.0, 10.0, 0.65, "copy"),
    rich_text_break_visual(12.0, 1.0),
    rich_text_image_visual(1.0, 24.0, 16.0, 12.0, 0.6, 4096.0),
    rich_text_palette_visual(
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        ColorValue::Palette(PaletteColor::Link),
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::TextQuaternary),
        ColorValue::Neutral(NeutralRole::BorderSecondary),
        24.0,
    ),
);

// 首次 UIX 构建固化声明值，全部 RichText 实例共享一份视觉表。
pub(crate) static UIX_RICH_TEXT_VISUAL: OnceLock<RichTextVisual> = OnceLock::new();
