use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 可由主题令牌解析的语义调色板角色。
pub enum PaletteColor {
    /// 品牌主色。
    Primary,
    /// 主色悬停态。
    PrimaryHover,
    /// 主色激活态。
    PrimaryActive,
    /// 主色弱背景。
    PrimaryBg,
    /// 主色边框。
    PrimaryBorder,
    /// 成功状态主色。
    Success,
    /// 成功状态背景。
    SuccessBg,
    /// 成功状态边框。
    SuccessBorder,
    /// 警告状态主色。
    Warning,
    /// 警告状态背景。
    WarningBg,
    /// 警告状态边框。
    WarningBorder,
    /// 错误状态主色。
    Error,
    /// 错误状态背景。
    ErrorBg,
    /// 错误状态边框。
    ErrorBorder,
    /// 信息状态主色。
    Info,
    /// 信息状态背景。
    InfoBg,
    /// 信息状态边框。
    InfoBorder,
    /// 链接默认色。
    Link,
    /// 链接悬停色。
    LinkHover,
    /// 链接激活色。
    LinkActive,
    /// 主题白色。
    White,
    /// 主题黑色。
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 可解析为最终颜色的样式值。
pub enum ColorValue {
    /// 主题语义调色板颜色。
    Palette(PaletteColor),
    /// 主题中性色角色。
    Neutral(NeutralRole),
    /// 不经主题转换的自定义颜色。
    Custom(Color),
}

impl ColorValue {
    /// 创建自定义颜色值。
    pub const fn custom(color: Color) -> Self {
        Self::Custom(color)
    }

    /// 创建中性色角色值。
    pub const fn neutral(role: NeutralRole) -> Self {
        Self::Neutral(role)
    }

    /// 创建语义调色板颜色值。
    pub const fn palette(color: PaletteColor) -> Self {
        Self::Palette(color)
    }

    /// 使用主题令牌解析最终颜色。
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
/// 可由主题令牌解析的排版字号角色。
pub enum TypographyToken {
    /// 小号正文。
    Small,
    /// 默认正文。
    Body,
    /// 大号正文。
    Large,
    /// 超大正文。
    XLarge,
    /// 一级标题。
    Heading1,
    /// 二级标题。
    Heading2,
    /// 三级标题。
    Heading3,
    /// 四级标题。
    Heading4,
    /// 五级标题。
    Heading5,
    /// 不经主题转换的自定义字号。
    Custom(f32),
}

impl TypographyToken {
    /// 使用主题令牌解析最终字号。
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

    /// 返回该角色不依赖主题的默认字号。
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
    /// 阴影颜色。
    pub color: Color,
    /// 阴影模糊半径。
    pub blur: f32,
    /// 阴影水平偏移。
    pub offset_x: f32,
    /// 阴影垂直偏移。
    pub offset_y: f32,
}

impl BoxShadowDef {
    /// 创建盒阴影定义。
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
