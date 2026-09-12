use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    Custom(Color),
    Token(&'static str, Color),
}
impl ColorValue {
    pub const fn custom(value: Color) -> Self {
        Self::Custom(value)
    }
    pub const fn token(key: &'static str, fallback: Color) -> Self {
        Self::Token(key, fallback)
    }
    pub fn resolve(self, tokens: &dyn ThemeTokens) -> Color {
        match self {
            Self::Custom(value) => value,
            Self::Token(key, fallback) => tokens.color(key, fallback),
        }
    }
}
impl Default for ColorValue {
    fn default() -> Self {
        Self::Token("uix.foreground", Color::black())
    }
}
impl From<Color> for ColorValue {
    fn from(value: Color) -> Self {
        Self::Custom(value)
    }
}
impl From<&str> for ColorValue {
    fn from(value: &str) -> Self {
        Self::Custom(Color::from(value))
    }
}
impl From<String> for ColorValue {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypographyToken {
    Custom(f32),
    Token(&'static str, f32),
}
impl TypographyToken {
    pub fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Custom(value) => value,
            Self::Token(key, fallback) => tokens.number(key, fallback),
        }
    }
    pub const fn default_size(self) -> f32 {
        match self {
            Self::Custom(value) | Self::Token(_, value) => value,
        }
    }
}
impl Default for TypographyToken {
    fn default() -> Self {
        Self::Token("uix.font-size", 14.0)
    }
}
impl From<f32> for TypographyToken {
    fn from(value: f32) -> Self {
        Self::Custom(value)
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
    /// 阴影基准几何向外扩张的距离；负值表示向内收缩。
    pub spread: f32,
}

impl BoxShadowDef {
    /// 创建盒阴影定义。
    pub const fn new(color: Color, blur: f32, offset_x: f32, offset_y: f32) -> Self {
        Self {
            color,
            blur,
            offset_x,
            offset_y,
            // 保持既有四参数构造器的零扩张语义。
            spread: 0.0,
        }
    }

    /// 返回设置了阴影扩张距离的新定义。
    pub const fn with_spread(mut self, spread: f32) -> Self {
        // 只替换扩张距离，保留颜色、模糊与偏移。
        self.spread = spread;
        // 返回可继续用于 const 上下文的值对象。
        self
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
