//! Style — 统一的类 CSS 样式系统。
//!
//! 融合了原有的 `Style` 和 `WidgetStylePreset`，增加状态变体（hover/active）、
//! 盒阴影、边距和主题感知能力。
//!
//! # 设计原则
//!
//! - Style 是纯数据（无方法，只有字段 + 构造器/Builder）
//! - 每个 widget 可选持有一个 `Style`，render 时通过 `ctx.apply_style()` 应用
//! - 状态变体在 widget 内部由事件更新，Style 只定义各状态的色值
//! - 主题感知：Style 可从 `TokenProvider` 获取默认值，用户可选择性覆盖

use uix_core::EdgeInsets;
use uix_graphics::Color;

// ════════════════════════════════════════════════════════════════════════════
// Style — 视觉样式
// ════════════════════════════════════════════════════════════════════════════

/// 类 CSS 视觉样式——覆盖 widget 常见的视觉属性。
///
/// widget 在 `render` 中通过 `ctx.apply_style(rect, &self.style)` 统一绘制背景/边框/阴影，
/// 然后使用 `self.style.color` 和 `self.style.font_size` 绘制文本。
///
/// # 优先级
///
/// 用户自定义 style > 状态变体 > widget 默认值 > 主题 tokens 默认值
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ── Background ──────────────────────────────────────────────
    /// 背景色
    pub background: Option<Color>,
    /// 悬停状态背景色
    pub background_hover: Option<Color>,
    /// 按下/激活状态背景色
    pub background_active: Option<Color>,

    // ── Border ──────────────────────────────────────────────────
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,

    // ── Box Shadow ──────────────────────────────────────────────
    pub shadow_color: Color,
    pub shadow_blur: f32,
    pub shadow_offset_x: f32,
    pub shadow_offset_y: f32,

    // ── Spacing ─────────────────────────────────────────────────
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,

    // ── Typography ──────────────────────────────────────────────
    pub color: Color,
    pub font_size: f32,

    // ── Misc ────────────────────────────────────────────────────
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            background_hover: None,
            background_active: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            shadow_color: Color::from_rgba(0, 0, 0, 64),
            shadow_blur: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            padding: EdgeInsets::zero(),
            margin: EdgeInsets::zero(),
            color: Color::black(),
            font_size: 14.0,
            opacity: 1.0,
        }
    }
}

impl Style {
    /// 创建一个空样式（所有字段使用默认值）。
    pub fn new() -> Self {
        Self::default()
    }

    // ── 预设构造器 ──────────────────────────────────────────────

    /// 默认按钮样式（白色背景 + 灰色边框）。
    pub fn button_default() -> Self {
        Self {
            background: None,
            border_color: Some(Color::from_rgba(217, 217, 217, 255)),
            border_width: 1.0,
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: Color::from_rgb(0, 0, 0),
            font_size: 14.0,
            ..Self::default()
        }
    }

    /// 主按钮样式（主题色背景 + 白色文字）。
    pub fn button_primary() -> Self {
        Self {
            background: Some(Color::from_rgba(22, 119, 255, 255)),
            border_color: Some(Color::from_rgba(22, 119, 255, 255)),
            border_width: 1.0,
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: Color::white(),
            font_size: 14.0,
            ..Self::default()
        }
    }

    /// 默认 Label 样式。
    pub fn label() -> Self {
        Self {
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            color: Color::black(),
            font_size: 12.0,
            ..Self::default()
        }
    }

    /// 默认容器样式。
    pub fn container() -> Self {
        Self {
            ..Self::default()
        }
    }

    // ── State helpers ──────────────────────────────────────────

    /// 根据 hover/pressed 状态返回当前背景色（优先返回状态色，fallback 到 background）。
    pub fn effective_bg(&self, hovered: bool, pressed: bool) -> Option<Color> {
        if pressed { self.background_active.or(self.background) }
        else if hovered { self.background_hover.or(self.background) }
        else { self.background }
    }

    // ── 链式 Builder ───────────────────────────────────────────

    pub fn with_bg(mut self, c: Color) -> Self { self.background = Some(c); self }
    pub fn with_bg_hover(mut self, c: Color) -> Self { self.background_hover = Some(c); self }
    pub fn with_bg_active(mut self, c: Color) -> Self { self.background_active = Some(c); self }
    pub fn with_color(mut self, c: Color) -> Self { self.color = c; self }
    pub fn with_font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn with_padding(mut self, p: EdgeInsets) -> Self { self.padding = p; self }
    pub fn with_margin(mut self, p: EdgeInsets) -> Self { self.margin = p; self }

    /// 设置边框。
    pub fn with_border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    /// 设置圆角。
    pub fn with_rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self
    }

    /// 设置盒阴影/辉光。
    pub fn with_shadow(mut self, color: Color, blur: f32) -> Self {
        self.shadow_color = color;
        self.shadow_blur = blur;
        self
    }

    /// 设置阴影偏移。
    pub fn with_shadow_offset(mut self, x: f32, y: f32) -> Self {
        self.shadow_offset_x = x;
        self.shadow_offset_y = y;
        self
    }

    /// 设置透明度。
    pub fn with_opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
}

// ────────────────────────────────────────────────────────────────────────────
// StyleVariant — 按交互状态区分的样式集合
// ────────────────────────────────────────────────────────────────────────────

/// 携带交互状态的样式集合——让 widget 根据 normal / hover / active / disabled
/// 自动选择对应的视觉颜色。
///
/// 使用方式：widget 在 `render` 中根据自身状态（hovered/pressed/disabled）
/// 从 variant 中取色，回退到 Style 基础色。
#[derive(Debug, Clone, PartialEq)]
pub struct StyleVariant {
    /// 正常状态基础样式
    pub normal: Style,
    /// 悬停样式覆盖（None = 使用 normal）
    pub hover: Option<Style>,
    /// 按下样式覆盖
    pub active: Option<Style>,
    /// 禁用样式覆盖
    pub disabled: Option<Style>,
}

impl Default for StyleVariant {
    fn default() -> Self {
        Self {
            normal: Style::default(),
            hover: None,
            active: None,
            disabled: None,
        }
    }
}

impl StyleVariant {
    pub fn new(base: Style) -> Self {
        Self { normal: base, ..Self::default() }
    }

    /// 链式设置悬停样式。
    pub fn hover(mut self, s: Style) -> Self { self.hover = Some(s); self }

    /// 链式设置按下样式。
    pub fn active(mut self, s: Style) -> Self { self.active = Some(s); self }

    /// 链式设置禁用样式。
    pub fn disabled(mut self, s: Style) -> Self { self.disabled = Some(s); self }

    /// 根据 widget 状态获取当前有效的 Style。
    pub fn resolve(&self, hovered: bool, pressed: bool, disabled: bool) -> &Style {
        if disabled { self.disabled.as_ref().unwrap_or(&self.normal) }
        else if pressed { self.active.as_ref().unwrap_or(&self.normal) }
        else if hovered { self.hover.as_ref().unwrap_or(&self.normal) }
        else { &self.normal }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uix_core::EdgeInsets;

    // ── 原有 Style 测试（保持向后兼容）──

    #[test]
    fn default_style_values() {
        let s = Style::default();
        assert_eq!(s.background, None);
        assert_eq!(s.background_hover, None);
        assert_eq!(s.background_active, None);
        assert_eq!(s.border_color, None);
        assert_eq!(s.border_width, 0.0);
        assert_eq!(s.border_radius, 0.0);
        assert_eq!(s.padding, EdgeInsets::zero());
        assert_eq!(s.margin, EdgeInsets::zero());
        assert_eq!(s.color, Color::black());
        assert_eq!(s.font_size, 14.0);
        assert_eq!(s.opacity, 1.0);
        assert!(s.shadow_blur == 0.0);
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
    fn with_margin_changes_margin() {
        let m = EdgeInsets::uniform(8.0);
        let s = Style::default().with_margin(m);
        assert_eq!(s.margin, m);
    }

    #[test]
    fn chained_modifications() {
        let s = Style::default()
            .with_bg(Color::from_rgba(0, 0, 0, 255))
            .with_color(Color::white())
            .with_font_size(16.0)
            .with_padding(EdgeInsets::uniform(8.0))
            .with_rounded(4.0)
            .with_shadow(Color::from_rgba(0, 0, 0, 128), 8.0);
        assert_eq!(s.background, Some(Color::from_rgba(0, 0, 0, 255)));
        assert_eq!(s.color, Color::white());
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.padding, EdgeInsets::uniform(8.0));
        assert_eq!(s.border_radius, 4.0);
        assert_eq!(s.shadow_blur, 8.0);
    }

    #[test]
    fn style_clone_equality() {
        let s1 = Style::button_primary();
        let s2 = s1.clone();
        assert_eq!(s1, s2);
    }

    #[test]
    fn new_is_same_as_default() {
        assert_eq!(Style::new(), Style::default());
    }

    // ── 新功能测试 ──

    #[test]
    fn effective_bg_without_states_falls_back() {
        let s = Style::default().with_bg(Color::blue());
        assert_eq!(s.effective_bg(false, false), Some(Color::blue()));
        assert_eq!(s.effective_bg(true, false), Some(Color::blue())); // no hover → fallback
        assert_eq!(s.effective_bg(false, true), Some(Color::blue())); // no active → fallback
    }

    #[test]
    fn effective_bg_with_states() {
        let gray = Color::from_rgb(128, 128, 128);
        let s = Style::default()
            .with_bg(gray)
            .with_bg_hover(Color::blue())
            .with_bg_active(Color::red());
        assert_eq!(s.effective_bg(false, false), Some(gray));
        assert_eq!(s.effective_bg(true, false), Some(Color::blue()));
        assert_eq!(s.effective_bg(false, true), Some(Color::red()));
    }

    #[test]
    fn with_border_sets_both() {
        let s = Style::default().with_border(Color::red(), 2.0);
        assert_eq!(s.border_color, Some(Color::red()));
        assert_eq!(s.border_width, 2.0);
    }

    #[test]
    fn with_shadow_sets_color_and_blur() {
        let s = Style::default().with_shadow(Color::from_rgba(0, 0, 0, 100), 10.0);
        assert_eq!(s.shadow_blur, 10.0);
        assert_eq!(s.shadow_color, Color::from_rgba(0, 0, 0, 100));
    }

    // ── StyleVariant 测试 ──

    #[test]
    fn style_variant_default_resolves_normal() {
        let v = StyleVariant::default();
        let r = v.resolve(false, false, false);
        assert_eq!(r.background, None);
    }

    #[test]
    fn style_variant_resolves_hover() {
        let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
        let hover_s = Style::default().with_bg(Color::blue());
        let v = StyleVariant::new(normal.clone()).hover(hover_s);
        assert_eq!(v.resolve(false, false, false).background, normal.background);
        assert_eq!(v.resolve(true, false, false).background, Some(Color::blue()));
    }

    #[test]
    fn style_variant_resolves_active() {
        let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
        let active_s = Style::default().with_bg(Color::red());
        let v = StyleVariant::new(normal).active(active_s);
        assert_eq!(v.resolve(false, true, false).background, Some(Color::red()));
    }

    #[test]
    fn style_variant_resolves_disabled() {
        let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
        let disabled_s = Style::default().with_bg(Color::from_rgb(64, 64, 64));
        let v = StyleVariant::new(normal).disabled(disabled_s);
        assert_eq!(v.resolve(false, false, true).background, Some(Color::from_rgb(64, 64, 64)));
    }

    #[test]
    fn style_variant_fallback() {
        let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
        let v = StyleVariant::new(normal.clone());
        // hover/active/disabled 都为 None → 回退到 normal
        assert_eq!(v.resolve(true, false, false).background, normal.background);
        assert_eq!(v.resolve(false, true, false).background, normal.background);
        assert_eq!(v.resolve(false, false, true).background, normal.background);
    }
}
