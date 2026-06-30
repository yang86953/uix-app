use std::fmt;

/// RGBA color (8-bit channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self { Self { r, g, b, a } }
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b, a: 255 } }

    pub const fn black() -> Self { Self::from_rgb(0, 0, 0) }
    pub const fn white() -> Self { Self::from_rgb(255, 255, 255) }
    pub const fn transparent() -> Self { Self::from_rgba(0, 0, 0, 0) }
    pub const fn red() -> Self { Self::from_rgb(255, 0, 0) }
    pub const fn green() -> Self { Self::from_rgb(0, 255, 0) }
    pub const fn blue() -> Self { Self::from_rgb(0, 0, 255) }

    /// Returns the premultiplied RGBA value as u32 (AARRGGBB).
    pub fn premultiplied(&self) -> u32 {
        let a = self.a as u32;
        let r = (self.r as u32 * a / 255) & 0xFF;
        let g = (self.g as u32 * a / 255) & 0xFF;
        let b = (self.b as u32 * a / 255) & 0xFF;
        (a << 24) | (b << 16) | (g << 8) | r
    }

    /// Returns the RGBA value as u32 (AARRGGBB).
    pub fn to_rgba(&self) -> u32 {
        (self.a as u32) << 24 | (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
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

impl Default for Color { fn default() -> Self { Self::black() } }

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
}

/// Predefined theme colors.
pub mod colors {
    use super::Color;
    pub const PRIMARY: Color = Color::from_rgb(24, 144, 255);
    pub const SUCCESS: Color = Color::from_rgb(82, 196, 26);
    pub const WARNING: Color = Color::from_rgb(250, 173, 20);
    pub const DANGER: Color = Color::from_rgb(255, 77, 79);
    pub const INFO: Color = Color::from_rgb(22, 119, 255);
    pub const BG_DARK: Color = Color::from_rgb(30, 30, 30);
    pub const BG_LIGHT: Color = Color::from_rgb(245, 245, 245);
    pub const SURFACE_DARK: Color = Color::from_rgb(45, 45, 45);
    pub const SURFACE_LIGHT: Color = Color::from_rgb(255, 255, 255);
    pub const TEXT_DARK: Color = Color::from_rgb(200, 200, 200);
    pub const TEXT_LIGHT: Color = Color::from_rgb(51, 51, 51);
    pub const DISABLED: Color = Color::from_rgb(191, 191, 191);
    pub const BORDER: Color = Color::from_rgb(217, 217, 217);
}
