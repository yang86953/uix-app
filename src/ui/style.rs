//! Style — 类 CSS 样式系统。
//!
//! 将 widget 的视觉属性（背景、边框、字体、间距）集中到 `Style`，
//! widget 不再在 render 中手写绘制逻辑，而是声明"我用什么样式"。
//!
//! # 设计原则
//!
//! - Style 是数据，不是行为（没有方法，只有字段 + 预设构造器）
//! - 每个 widget 可选持有一个 Style，render 时通过 `ctx.apply_style()` 应用
//! - 预设构造器（如 `Style::button_primary()`）提供 Ant Design 5 风格默认值
//!
//! # 示例
//!
//! ```ignore
//! let s = Style {
//!     background: Some(t.color_primary),
//!     color: Color::white(),
//!     border_radius: 6.0,
//!     ..Style::default()
//! };
//! ctx.apply_style(rect, &s);
//! ctx.text_center("Click me", rect.inset(s.padding), s.color, s.font_size);
//! ```

use crate::base::EdgeInsets;
use crate::graphics::Color;

// ════════════════════════════════════════════════════════════════════════════
// Style — 视觉样式
// ════════════════════════════════════════════════════════════════════════════

/// 类 CSS 视觉样式。
///
/// 覆盖 widget 常见的视觉属性。widget 在 `render` 中：
/// 1. `ctx.apply_style(rect, &self.style)` — 画背景/边框
/// 2. `ctx.text_center(text, content_rect, self.style.color, self.style.font_size)` — 画文本
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ── Box ───────────────────────────────────────────────────────
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,

    // ── Spacing ───────────────────────────────────────────────────
    pub padding: EdgeInsets,

    // ── Typography ────────────────────────────────────────────────
    pub color: Color,
    pub font_size: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::zero(),
            color: Color::black(),
            font_size: 14.0,
        }
    }
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    // ── 预设 ─────────────────────────────────────────────────────

    /// Ant Design 风格的默认按钮（白色背景 + 灰色边框）。
    pub fn button_default() -> Self {
        Self {
            background: None,
            border_color: Some(Color::from_rgba(217, 217, 217, 255)),
            border_width: 1.0,
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: Color::from_rgb(0, 0, 0),
            font_size: 14.0,
        }
    }

    /// Ant Design 风格的主按钮（蓝色背景 + 白色文字）。
    pub fn button_primary() -> Self {
        Self {
            background: Some(Color::from_rgba(22, 119, 255, 255)),
            border_color: Some(Color::from_rgba(22, 119, 255, 255)),
            border_width: 1.0,
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: Color::white(),
            font_size: 14.0,
        }
    }

    /// 默认 Label 样式。
    pub fn label() -> Self {
        Self {
            background: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            color: Color::black(),
            font_size: 12.0,
        }
    }

    /// 链式修改 — background
    pub fn with_bg(mut self, c: Color) -> Self {
        self.background = Some(c);
        self
    }

    /// 链式修改 — color
    pub fn with_color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    /// 链式修改 — font_size
    pub fn with_font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }

    /// 链式修改 — padding
    pub fn with_padding(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::EdgeInsets;

    #[test]
    fn default_style_values() {
        let s = Style::default();
        assert_eq!(s.background, None);
        assert_eq!(s.border_color, None);
        assert_eq!(s.border_width, 0.0);
        assert_eq!(s.border_radius, 0.0);
        assert_eq!(s.padding, EdgeInsets::zero());
        assert_eq!(s.color, Color::black());
        assert_eq!(s.font_size, 14.0);
    }

    #[test]
    fn button_default_preset() {
        let s = Style::button_default();
        assert_eq!(s.background, None);
        assert!(s.border_color.is_some());
        assert_eq!(s.border_width, 1.0);
        assert_eq!(s.border_radius, 6.0);
        assert_eq!(s.font_size, 14.0);
        assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
    }

    #[test]
    fn button_primary_preset() {
        let s = Style::button_primary();
        assert_eq!(s.background, Some(Color::from_rgba(22, 119, 255, 255)));
        assert_eq!(s.color, Color::white());
        assert_eq!(s.border_width, 1.0);
        assert_eq!(s.border_radius, 6.0);
    }

    #[test]
    fn label_preset() {
        let s = Style::label();
        assert_eq!(s.background, None);
        assert_eq!(s.font_size, 12.0);
        assert_eq!(s.padding, EdgeInsets::new(2.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn with_bg_changes_background() {
        let s = Style::default().with_bg(Color::red());
        assert_eq!(s.background, Some(Color::red()));
    }

    #[test]
    fn with_color_changes_color() {
        let s = Style::default().with_color(Color::blue());
        assert_eq!(s.color, Color::blue());
    }

    #[test]
    fn with_font_size_changes_size() {
        let s = Style::default().with_font_size(18.0);
        assert_eq!(s.font_size, 18.0);
    }

    #[test]
    fn with_padding_changes_padding() {
        let p = EdgeInsets::new(5.0, 10.0, 5.0, 10.0);
        let s = Style::default().with_padding(p);
        assert_eq!(s.padding, p);
    }

    #[test]
    fn chained_modifications() {
        let s = Style::default()
            .with_bg(Color::from_rgba(0, 0, 0, 255))
            .with_color(Color::white())
            .with_font_size(16.0)
            .with_padding(EdgeInsets::uniform(8.0));
        assert_eq!(s.background, Some(Color::from_rgba(0, 0, 0, 255)));
        assert_eq!(s.color, Color::white());
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.padding, EdgeInsets::uniform(8.0));
    }

    #[test]
    fn style_clone_equality() {
        let s1 = Style::button_primary();
        let s2 = s1.clone();
        assert_eq!(s1, s2);
    }

    #[test]
    fn style_debug_format() {
        let s = Style::default();
        let debug = format!("{:?}", s);
        assert!(debug.contains("background"));
        assert!(debug.contains("font_size"));
    }

    #[test]
    fn new_is_same_as_default() {
        assert_eq!(Style::new(), Style::default());
    }
}
