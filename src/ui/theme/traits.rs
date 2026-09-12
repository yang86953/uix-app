//! 主题与设计令牌契约。

use crate::core::EdgeInsets;
use crate::ui::theme::style::{ColorValue, PaletteColor, Style, TypographyToken};
use crate::ui::theme::{IColorTokens, NeutralRole, ShadowToken};

/// 排版设计令牌。
pub trait ITypographyTokens: Send + Sync {
    /// 返回默认字体族。
    fn font_family(&self) -> &str;
    /// 返回小号正文字号。
    fn font_size_sm(&self) -> f32 {
        12.0
    }
    /// 返回默认正文字号。
    fn font_size(&self) -> f32 {
        14.0
    }
    /// 返回大号正文字号。
    fn font_size_lg(&self) -> f32 {
        16.0
    }
    /// 返回超大正文字号。
    fn font_size_xl(&self) -> f32 {
        20.0
    }
    /// 返回一级标题字号。
    fn font_size_heading_1(&self) -> f32 {
        38.0
    }
    /// 返回二级标题字号。
    fn font_size_heading_2(&self) -> f32 {
        30.0
    }
    /// 返回三级标题字号。
    fn font_size_heading_3(&self) -> f32 {
        24.0
    }
    /// 返回四级标题字号。
    fn font_size_heading_4(&self) -> f32 {
        20.0
    }
    /// 返回五级标题字号。
    fn font_size_heading_5(&self) -> f32 {
        16.0
    }
    /// 返回常规字重。
    fn font_weight_regular(&self) -> f32 {
        400.0
    }
    /// 返回中等字重。
    fn font_weight_medium(&self) -> f32 {
        500.0
    }
    /// 返回半粗字重。
    fn font_weight_semibold(&self) -> f32 {
        600.0
    }
    /// 返回粗体字重。
    fn font_weight_bold(&self) -> f32 {
        700.0
    }
    /// 返回默认文本行高倍率。
    fn line_height(&self) -> f32 {
        1.5715
    }
}

/// 间距与尺寸设计令牌。
pub trait ISpacingTokens: Send + Sync {
    /// 返回极小内边距。
    fn padding_xss(&self) -> f32 {
        4.0
    }
    /// 返回超小内边距。
    fn padding_xs(&self) -> f32 {
        8.0
    }
    /// 返回小内边距。
    fn padding_sm(&self) -> f32 {
        12.0
    }
    /// 返回默认内边距。
    fn padding(&self) -> f32 {
        16.0
    }
    /// 返回中等内边距。
    fn padding_md(&self) -> f32 {
        20.0
    }
    /// 返回大内边距。
    fn padding_lg(&self) -> f32 {
        24.0
    }
    /// 返回超大内边距。
    fn padding_xl(&self) -> f32 {
        32.0
    }
    /// 返回默认圆角半径。
    fn border_radius(&self) -> f32 {
        6.0
    }
    /// 返回小圆角半径。
    fn border_radius_sm(&self) -> f32 {
        4.0
    }
    /// 返回大圆角半径。
    fn border_radius_lg(&self) -> f32 {
        8.0
    }
    /// 返回超大圆角半径。
    fn border_radius_xl(&self) -> f32 {
        12.0
    }
    /// 返回胶囊形圆角半径。
    fn border_radius_round(&self) -> f32 {
        999.0
    }
    /// 返回小控件高度。
    fn control_height_sm(&self) -> f32 {
        24.0
    }
    /// 返回默认控件高度。
    fn control_height(&self) -> f32 {
        32.0
    }
    /// 返回大控件高度。
    fn control_height_lg(&self) -> f32 {
        40.0
    }
    /// overlay backdrop blur 的默认逻辑像素半径。
    fn backdrop_blur_radius(&self) -> f32 {
        // 产品默认值由 #805 决策固定为 8 逻辑像素。
        8.0
    }
    /// 返回快速动效持续时间秒数。
    fn motion_duration_fast(&self) -> f32 {
        0.1
    }
    /// 返回中速动效持续时间秒数。
    fn motion_duration_mid(&self) -> f32 {
        0.2
    }
    /// 返回慢速动效持续时间秒数。
    fn motion_duration_slow(&self) -> f32 {
        0.3
    }
    /// 返回默认动效缓动表达式。
    fn motion_easing_default(&self) -> &str {
        "cubic-bezier(0.25, 0.1, 0.25, 1)"
    }
    /// 返回进入动效缓动表达式。
    fn motion_easing_in(&self) -> &str {
        "cubic-bezier(0.42, 0, 1, 1)"
    }
    /// 返回离开动效缓动表达式。
    fn motion_easing_out(&self) -> &str {
        "cubic-bezier(0, 0, 0.58, 1)"
    }
    /// 返回双向动效缓动表达式。
    fn motion_easing_in_out(&self) -> &str {
        "cubic-bezier(0.42, 0, 0.58, 1)"
    }
    /// 返回超小响应式断点。
    fn screen_xs(&self) -> f32 {
        480.0
    }
    /// 返回小响应式断点。
    fn screen_sm(&self) -> f32 {
        576.0
    }
    /// 返回中等响应式断点。
    fn screen_md(&self) -> f32 {
        768.0
    }
    /// 返回大响应式断点。
    fn screen_lg(&self) -> f32 {
        992.0
    }
    /// 返回超大响应式断点。
    fn screen_xl(&self) -> f32 {
        1200.0
    }
    /// 返回双倍超大响应式断点。
    fn screen_xxl(&self) -> f32 {
        1600.0
    }
}

/// 结构化多层阴影令牌。
pub trait IBoxShadowTokens: Send + Sync {
    /// 返回默认结构化盒阴影。
    fn box_shadow(&self) -> ShadowToken;
    /// 返回次级结构化盒阴影。
    fn box_shadow_secondary(&self) -> ShadowToken;
}

/// UI 主题设计令牌聚合。
pub trait ThemeTokens:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    /// 判断当前主题是否为深色主题。
    fn is_dark(&self) -> bool {
        false
    }
}

/// 抽象设计令牌提供者 — 聚合 ThemeTokens 与 UI 域 style 助手。
pub trait TokenProvider: ThemeTokens + Send + Sync {}
