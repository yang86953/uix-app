use std::fmt;

/// RGBA color (8-bit channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// 红色通道。
    pub r: u8,
    /// 绿色通道。
    pub g: u8,
    /// 蓝色通道。
    pub b: u8,
    /// 透明度通道。
    pub a: u8,
}

impl Color {
    /// 不透明黑色。
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    /// 不透明白色。
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);
    /// 完全透明的黑色。
    pub const TRANSPARENT: Self = Self::from_rgba(0, 0, 0, 0);
    /// 不透明红色。
    pub const RED: Self = Self::from_rgb(255, 0, 0);
    /// 不透明绿色。
    pub const GREEN: Self = Self::from_rgb(0, 255, 0);
    /// 不透明蓝色。
    pub const BLUE: Self = Self::from_rgb(0, 0, 255);

    /// 使用红、绿、蓝和透明度通道创建颜色。
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    /// 使用红、绿、蓝通道创建不透明颜色。
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// 使用 8-bit RGBA 通道构造颜色。
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from_rgba(r, g, b, a)
    }

    /// 解析 6 位 RGB 或 8 位 RGBA 十六进制颜色；无效输入回退为黑色。
    pub fn hex(value: &str) -> Self {
        parse_hex(value).unwrap_or(Self::BLACK)
    }

    /// 返回不透明黑色。
    pub const fn black() -> Self {
        Self::BLACK
    }
    /// 返回不透明白色。
    pub const fn white() -> Self {
        Self::WHITE
    }
    /// 返回完全透明的颜色。
    pub const fn transparent() -> Self {
        Self::TRANSPARENT
    }
    /// 返回不透明红色。
    pub const fn red() -> Self {
        Self::RED
    }
    /// 返回不透明绿色。
    pub const fn green() -> Self {
        Self::GREEN
    }
    /// 返回不透明蓝色。
    pub const fn blue() -> Self {
        Self::BLUE
    }
    /// 返回中性灰色。
    pub const fn gray() -> Self {
        Self::from_rgb(128, 128, 128)
    }

    /// Returns the premultiplied RGBA value as u32 (AARRGGBB).
    pub fn premultiplied(&self) -> u32 {
        let a = self.a as u32;
        let r = (self.r as u32 * a / 255) & 0xFF;
        let g = (self.g as u32 * a / 255) & 0xFF;
        let b = (self.b as u32 * a / 255) & 0xFF;
        (a << 24) | (r << 16) | (g << 8) | b
    }

    /// Returns the RGBA value as u32 (AARRGGBB).
    pub fn to_rgba(&self) -> u32 {
        (self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// Linear interpolation toward white. factor=0 → self, factor=1 → white.
    pub fn lighten(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::from_rgb(
            (self.r as f32 + (255.0 - self.r as f32) * f) as u8,
            (self.g as f32 + (255.0 - self.g as f32) * f) as u8,
            (self.b as f32 + (255.0 - self.b as f32) * f) as u8,
        )
    }

    /// Linear interpolation toward black. factor=0 → self, factor=1 → black.
    pub fn darken(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::from_rgb(
            (self.r as f32 * (1.0 - f)) as u8,
            (self.g as f32 * (1.0 - f)) as u8,
            (self.b as f32 * (1.0 - f)) as u8,
        )
    }

    /// Mix two colors. t=0 → self, t=1 → other.
    pub fn mix(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self::from_rgba(
            (self.r as f32 * inv + other.r as f32 * t) as u8,
            (self.g as f32 * inv + other.g as f32 * t) as u8,
            (self.b as f32 * inv + other.b as f32 * t) as u8,
            (self.a as f32 * inv + other.a as f32 * t) as u8,
        )
    }

    /// Replace alpha channel.
    pub fn with_alpha(&self, a: u8) -> Self {
        Self::from_rgba(self.r, self.g, self.b, a)
    }

    /// Perceived brightness (0=dark, 255=bright) using sRGB luminance weights.
    pub fn luminance(&self) -> u8 {
        (self.r as f32 * 0.2126 + self.g as f32 * 0.7152 + self.b as f32 * 0.0722) as u8
    }

    /// Returns true if the color is perceived as light (luminance > 128).
    pub fn is_light(&self) -> bool {
        self.luminance() > 128
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

/// 从 hex 字符串（如 `"#ff4d4f"` 或 `"ff4d4f"`）解析颜色。
/// 支持 6 位 RGB 和 8 位 RGBA 格式。
fn parse_hex(hex: &str) -> Option<Color> {
    let s = hex.trim_start_matches('#');
    if s.len() != 6 && s.len() != 8 {
        return None;
    }
    let val = u32::from_str_radix(s, 16).ok()?;
    if s.len() == 6 {
        Some(Color::from_rgb(
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        ))
    } else {
        Some(Color::from_rgba(
            ((val >> 24) & 0xFF) as u8,
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        ))
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        Self::hex(s)
    }
}

impl From<String> for Color {
    fn from(s: String) -> Self {
        Color::from(s.as_str())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}{:02X}",
            self.r, self.g, self.b, self.a
        )
    }
}

/// Predefined theme colors.
pub mod colors {
    use super::Color;
    /// 默认品牌主色。
    pub const PRIMARY: Color = Color::from_rgb(24, 144, 255);
    /// 成功状态色。
    pub const SUCCESS: Color = Color::from_rgb(82, 196, 26);
    /// 警告状态色。
    pub const WARNING: Color = Color::from_rgb(250, 173, 20);
    /// 危险或错误状态色。
    pub const DANGER: Color = Color::from_rgb(255, 77, 79);
    /// 信息状态色。
    pub const INFO: Color = Color::from_rgb(22, 119, 255);
    /// 深色主题背景色。
    pub const BG_DARK: Color = Color::from_rgb(30, 30, 30);
    /// 浅色主题背景色。
    pub const BG_LIGHT: Color = Color::from_rgb(245, 245, 245);
    /// 深色主题表面色。
    pub const SURFACE_DARK: Color = Color::from_rgb(45, 45, 45);
    /// 浅色主题表面色。
    pub const SURFACE_LIGHT: Color = Color::from_rgb(255, 255, 255);
    /// 深色背景上的文本色。
    pub const TEXT_DARK: Color = Color::from_rgb(200, 200, 200);
    /// 浅色背景上的文本色。
    pub const TEXT_LIGHT: Color = Color::from_rgb(51, 51, 51);
    /// 禁用状态色。
    pub const DISABLED: Color = Color::from_rgb(191, 191, 191);
    /// 默认边框色。
    pub const BORDER: Color = Color::from_rgb(217, 217, 217);
}
