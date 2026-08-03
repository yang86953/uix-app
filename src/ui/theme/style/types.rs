use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteColor {
    Primary,
    PrimaryHover,
    PrimaryActive,
    PrimaryBg,
    PrimaryBorder,
    Success,
    SuccessBg,
    SuccessBorder,
    Warning,
    WarningBg,
    WarningBorder,
    Error,
    ErrorBg,
    ErrorBorder,
    Info,
    InfoBg,
    InfoBorder,
    Link,
    LinkHover,
    LinkActive,
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    Palette(PaletteColor),
    Neutral(NeutralRole),
    Custom(Color),
}

impl ColorValue {
    pub const fn custom(color: Color) -> Self {
        Self::Custom(color)
    }

    pub const fn neutral(role: NeutralRole) -> Self {
        Self::Neutral(role)
    }

    pub const fn palette(color: PaletteColor) -> Self {
        Self::Palette(color)
    }

    pub fn resolve(self, tokens: &dyn ThemeTokens) -> Color {
        match self {
            Self::Palette(color) => match color {
                PaletteColor::Primary => tokens.color_primary(),
                PaletteColor::PrimaryHover => tokens.color_primary_hover(),
                PaletteColor::PrimaryActive => tokens.color_primary_active(),
                PaletteColor::PrimaryBg => tokens.color_primary_bg(),
                PaletteColor::PrimaryBorder => tokens.color_primary_border(),
                PaletteColor::Success => tokens.color_success(),
                PaletteColor::SuccessBg => tokens.color_success_bg(),
                PaletteColor::SuccessBorder => tokens.color_success_border(),
                PaletteColor::Warning => tokens.color_warning(),
                PaletteColor::WarningBg => tokens.color_warning_bg(),
                PaletteColor::WarningBorder => tokens.color_warning_border(),
                PaletteColor::Error => tokens.color_error(),
                PaletteColor::ErrorBg => tokens.color_error_bg(),
                PaletteColor::ErrorBorder => tokens.color_error_border(),
                PaletteColor::Info => tokens.color_info(),
                PaletteColor::InfoBg => tokens.color_info_bg(),
                PaletteColor::InfoBorder => tokens.color_info_border(),
                PaletteColor::Link => tokens.color_link(),
                PaletteColor::LinkHover => tokens.color_link_hover(),
                PaletteColor::LinkActive => tokens.color_link_active(),
                PaletteColor::White => tokens.color_white(),
                PaletteColor::Black => tokens.color_black(),
            },
            Self::Neutral(role) => match role {
                NeutralRole::Text => tokens.color_text(),
                NeutralRole::TextSecondary => tokens.color_text_secondary(),
                NeutralRole::TextTertiary => tokens.color_text_tertiary(),
                NeutralRole::TextQuaternary => tokens.color_text_quaternary(),
                NeutralRole::TextInverse => {
                    if tokens.is_dark() {
                        tokens.color_black()
                    } else {
                        tokens.color_white()
                    }
                }
                NeutralRole::Border => tokens.color_border(),
                NeutralRole::BorderSecondary => tokens.color_border_secondary(),
                NeutralRole::Fill => tokens.color_fill(),
                NeutralRole::FillSecondary => tokens.color_fill_secondary(),
                NeutralRole::FillTertiary => tokens.color_fill_tertiary(),
                NeutralRole::FillQuaternary => tokens.color_fill_quaternary(),
                NeutralRole::BgContainer => tokens.color_bg_container(),
                NeutralRole::BgElevated => tokens.color_bg_elevated(),
                NeutralRole::BgLayout => tokens.color_bg_layout(),
                NeutralRole::BgMask => tokens.color_bg_mask(),
            },
            Self::Custom(color) => color,
        }
    }
}

impl From<Color> for ColorValue {
    fn from(color: Color) -> Self {
        Self::Custom(color)
    }
}

impl From<&str> for ColorValue {
    fn from(color: &str) -> Self {
        Self::Custom(Color::from(color))
    }
}

impl From<String> for ColorValue {
    fn from(color: String) -> Self {
        Self::from(color.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypographyToken {
    Small,
    Body,
    Large,
    XLarge,
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Custom(f32),
}

impl TypographyToken {
    pub fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
            Self::Body => tokens.font_size(),
            Self::Large => tokens.font_size_lg(),
            Self::XLarge => tokens.font_size_xl(),
            Self::Heading1 => tokens.font_size_heading_1(),
            Self::Heading2 => tokens.font_size_heading_2(),
            Self::Heading3 => tokens.font_size_heading_3(),
            Self::Heading4 => tokens.font_size_heading_4(),
            Self::Heading5 => tokens.font_size_heading_5(),
            Self::Custom(size) => size,
        }
    }

    pub const fn default_size(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Body => 14.0,
            Self::Large => 16.0,
            Self::XLarge => 20.0,
            Self::Heading1 => 38.0,
            Self::Heading2 => 30.0,
            Self::Heading3 => 24.0,
            Self::Heading4 => 20.0,
            Self::Heading5 => 16.0,
            Self::Custom(size) => size,
        }
    }
}

impl From<f32> for TypographyToken {
    fn from(size: f32) -> Self {
        Self::Custom(size)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 盒阴影
// ════════════════════════════════════════════════════════════════════════════

/// 盒阴影定义。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadowDef {
    pub color: Color,
    pub blur: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl BoxShadowDef {
    pub const fn new(color: Color, blur: f32, offset_x: f32, offset_y: f32) -> Self {
        Self {
            color,
            blur,
            offset_x,
            offset_y,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 显示模式
// ════════════════════════════════════════════════════════════════════════════

/// 显示模式——决定容器子节点的布局方式。
///
/// 注意：FlexDirection、JustifyContent、AlignItems 定义在 `layout/mod.rs` 中，
/// 这里只是 re-export 使用。`DisplayMode` 是样式系统的独有概念。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// 不参与布局/渲染（相当于 CSS `display: none`）
    None,
    /// Flexbox 布局（默认）
    #[default]
    Flex,
    /// Grid 布局
    Grid,
}

// ════════════════════════════════════════════════════════════════════════════
// Style — 视觉样式
// ════════════════════════════════════════════════════════════════════════════
