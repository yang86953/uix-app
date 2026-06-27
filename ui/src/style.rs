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

use uix_platform::EdgeInsets;
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

