//! Style — 统一的类 CSS 样式系统。
//!
//! 融合了原有的 `Style` 和 `WidgetStylePreset`，增加状态变体（hover/active）、
//! 盒阴影、边距、Flex/Grid 布局属性和主题感知能力。
//!
//! # 设计原则
//!
//! - Style 是纯数据（无方法，只有字段 + 构造器/Builder）
//! - 每个 widget 可选持有一个 `Style`，render 时通过 `ctx.apply_style()` 应用
//! - 状态变体在 widget 内部由事件更新，Style 只定义各状态的色值
//! - 主题感知：Style 可从 `TokenProvider` 获取默认值，用户可选择性覆盖
//!
//! # Style 作为组件唯一视觉契约
//!
//! 所有组件不再保有 `bg_color`/`border_color`/`padding` 等独立字段，
//! 统一使用 `style: Style`。布局引擎从 `style.margin` 读取外边距参与盒模型计算。

use uix_platform::EdgeInsets;
use uix_graphics::Color;
// Re-export layout enums so crate::style::FlexDirection etc. work
pub use crate::layout::{FlexDirection, JustifyContent, AlignItems};

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
        Self { color, blur, offset_x, offset_y }
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

/// 类 CSS 视觉样式——覆盖 widget 常见的视觉属性。
///
/// widget 在 `render` 中通过 `ctx.apply_style(rect, &self.style)` 统一绘制
/// 背景/边框/阴影，然后使用 `self.style.color/font_size` 绘制文本。
///
/// 布局引擎通过以下字段影响布局：
/// - `margin`：外边距，参与盒模型计算（推开兄弟节点）
/// - `padding`：内边距，影响内容区域
/// - `width`/`height`：固定尺寸
/// - `display`/`flex_direction`/`gap` 等：容器布局行为
/// - `flex_grow`/`flex_shrink`：子项弹性
///
/// # 优先级
///
/// 用户自定义 style > 状态变体 > widget 默认值 > 主题 tokens 默认值
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // ── 盒模型 ──────────────────────────────────────────────
    /// 外边距（推动兄弟节点）
    pub margin: EdgeInsets,
    /// 内边距
    pub padding: EdgeInsets,
    /// 边框颜色（Some = 显示边框）
    pub border_color: Option<Color>,
    /// 边框宽度
    pub border_width: f32,
    /// 边框圆角
    pub border_radius: f32,

    // ── 尺寸 ────────────────────────────────────────────────
    /// 固定宽度（None = 由子内容决定）
    pub width: Option<f32>,
    /// 固定高度（None = 由子内容决定）
    pub height: Option<f32>,

    // ── 弹性布局（容器属性）──
    /// 显示模式
    pub display: DisplayMode,
    /// Flex 主轴方向
    pub flex_direction: FlexDirection,
    /// Flex 是否换行
    pub flex_wrap: bool,
    /// 主轴对齐
    pub justify_content: JustifyContent,
    /// 交叉轴对齐
    pub align_items: AlignItems,
    /// 子项间距
    pub gap: f32,

    // ── 弹性布局（子项属性）──
    /// 扩展比例
    pub flex_grow: f32,
    /// 收缩比例
    pub flex_shrink: f32,
    /// 子项单独覆盖 align_items
    pub align_self: Option<AlignItems>,

    // ── 视觉 ────────────────────────────────────────────────
    /// 背景色
    pub background: Option<Color>,
    /// 悬停状态背景色
    pub background_hover: Option<Color>,
    /// 按下/激活状态背景色
    pub background_active: Option<Color>,
    /// 文字颜色
    pub color: Color,
    /// 字号
    pub font_size: f32,
    /// 整体透明度
    pub opacity: f32,
    /// 盒阴影
    pub box_shadow: Option<BoxShadowDef>,

    // ── 显示 ────────────────────────────────────────────────
    /// 是否可见（不可见时不参与布局也不渲染）
    pub visible: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            margin: EdgeInsets::zero(),
            padding: EdgeInsets::zero(),
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,

            width: None,
            height: None,

            display: DisplayMode::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: false,
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            gap: 0.0,

            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_self: None,

            background: None,
            background_hover: None,
            background_active: None,
            color: Color::black(),
            font_size: 14.0,
            opacity: 1.0,
            box_shadow: None,

            visible: true,
        }
    }
}

impl Style {
    /// 创建一个空样式（所有字段使用默认值）。
    pub fn new() -> Self {
        Self::default()
    }

    // ── 便捷预设构造器 ──────────────────────────────────────

    /// Flex 行方向容器。
    pub fn row() -> Self {
        Self {
            display: DisplayMode::Flex,
            flex_direction: FlexDirection::Row,
            ..Self::default()
        }
    }

    /// Flex 列方向容器。
    pub fn column() -> Self {
        Self {
            display: DisplayMode::Flex,
            ..Self::default()
        }
    }

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

    /// 默认容器样式。
    pub fn container() -> Self {
        Self {
            display: DisplayMode::Flex,
            ..Self::default()
        }
    }

    // ── State helpers ──────────────────────────────────────

    /// 根据 hover/pressed 状态返回当前背景色（优先返回状态色，fallback 到 background）。
    pub fn effective_bg(&self, hovered: bool, pressed: bool) -> Option<Color> {
        if pressed { self.background_active.or(self.background) }
        else if hovered { self.background_hover.or(self.background) }
        else { self.background }
    }

    // ── 链式 Builder 方法 ──────────────────────────────────

    /// 批量设置完整的 Style。
    pub fn with_style(mut self, s: Self) -> Self {
        self.margin = s.margin;
        self.padding = s.padding;
        self.border_color = s.border_color;
        self.border_width = s.border_width;
        self.border_radius = s.border_radius;
        self.width = s.width;
        self.height = s.height;
        self.display = s.display;
        self.flex_direction = s.flex_direction;
        self.flex_wrap = s.flex_wrap;
        self.justify_content = s.justify_content;
        self.align_items = s.align_items;
        self.gap = s.gap;
        self.flex_grow = s.flex_grow;
        self.flex_shrink = s.flex_shrink;
        self.align_self = s.align_self;
        self.background = s.background;
        self.background_hover = s.background_hover;
        self.background_active = s.background_active;
        self.color = s.color;
        self.font_size = s.font_size;
        self.opacity = s.opacity;
        self.box_shadow = s.box_shadow;
        self.visible = s.visible;
        self
    }

    /// 应用另一个 Style 到自身（非 None 字段覆盖，None 字段保留原值）。
    pub fn apply(mut self, other: Self) -> Self {
        if other.margin != EdgeInsets::zero() { self.margin = other.margin; }
        if other.padding != EdgeInsets::zero() { self.padding = other.padding; }
        if other.border_color.is_some() { self.border_color = other.border_color; }
        if other.border_width != 0.0 { self.border_width = other.border_width; }
        if other.border_radius != 0.0 { self.border_radius = other.border_radius; }
        if other.width.is_some() { self.width = other.width; }
        if other.height.is_some() { self.height = other.height; }
        if other.display != DisplayMode::default() { self.display = other.display; }
        if other.flex_direction != FlexDirection::default() { self.flex_direction = other.flex_direction; }
        if other.flex_wrap { self.flex_wrap = other.flex_wrap; }
        if other.justify_content != JustifyContent::default() { self.justify_content = other.justify_content; }
        if other.align_items != AlignItems::default() { self.align_items = other.align_items; }
        if other.gap != 0.0 { self.gap = other.gap; }
        if other.flex_grow != 0.0 { self.flex_grow = other.flex_grow; }
        if other.flex_shrink != 1.0 { self.flex_shrink = other.flex_shrink; }
        if other.align_self.is_some() { self.align_self = other.align_self; }
        if other.background.is_some() { self.background = other.background; }
        if other.background_hover.is_some() { self.background_hover = other.background_hover; }
        if other.background_active.is_some() { self.background_active = other.background_active; }
        if other.color != Color::black() { self.color = other.color; }
        if other.font_size != 14.0 { self.font_size = other.font_size; }
        if other.opacity != 1.0 { self.opacity = other.opacity; }
        if other.box_shadow.is_some() { self.box_shadow = other.box_shadow; }
        if !other.visible { self.visible = other.visible; }
        self
    }

    // ── 盒模型链式方法 ──

    pub fn with_margin(mut self, m: EdgeInsets) -> Self { self.margin = m; self }
    pub fn with_padding(mut self, p: EdgeInsets) -> Self { self.padding = p; self }
    pub fn with_border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }
    pub fn with_rounded(mut self, r: f32) -> Self { self.border_radius = r; self }

    // ── 尺寸链式方法 ──

    pub fn with_width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn with_height(mut self, h: f32) -> Self { self.height = Some(h); self }
    pub fn with_size(mut self, w: f32, h: f32) -> Self { self.width = Some(w); self.height = Some(h); self }

    // ── 布局链式方法 ──

    pub fn with_display(mut self, d: DisplayMode) -> Self { self.display = d; self }
    pub fn with_direction(mut self, d: FlexDirection) -> Self { self.flex_direction = d; self }
    pub fn with_wrap(mut self, w: bool) -> Self { self.flex_wrap = w; self }
    pub fn with_justify(mut self, j: JustifyContent) -> Self { self.justify_content = j; self }
    pub fn with_align(mut self, a: AlignItems) -> Self { self.align_items = a; self }
    pub fn with_gap(mut self, g: f32) -> Self { self.gap = g; self }
    pub fn with_grow(mut self, g: f32) -> Self { self.flex_grow = g; self }
    pub fn with_shrink(mut self, s: f32) -> Self { self.flex_shrink = s; self }

    // ── 视觉链式方法 ──

    pub fn with_bg(mut self, c: Color) -> Self { self.background = Some(c); self }
    pub fn with_bg_hover(mut self, c: Color) -> Self { self.background_hover = Some(c); self }
    pub fn with_bg_active(mut self, c: Color) -> Self { self.background_active = Some(c); self }
    pub fn with_color(mut self, c: Color) -> Self { self.color = c; self }
    pub fn with_font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn with_opacity(mut self, o: f32) -> Self { self.opacity = o; self }
    pub fn with_shadow(mut self, shadow: BoxShadowDef) -> Self { self.box_shadow = Some(shadow); self }
    pub fn with_visible(mut self, v: bool) -> Self { self.visible = v; self }
}

/// 外边距/内边距快捷构造（不实现 From 以避免孤儿规则冲突）。
pub mod edge_insets {
    use uix_platform::EdgeInsets;

    /// 四边均匀外边距。
    pub fn all(v: f32) -> EdgeInsets { EdgeInsets::uniform(v) }

    /// 垂直/水平外边距：`[top_bottom, left_right]`。
    pub fn symmetric(vertical: f32, horizontal: f32) -> EdgeInsets {
        EdgeInsets::new(horizontal, vertical, horizontal, vertical)
    }

    /// 四边独立外边距：`[top, right, bottom, left]`。
    pub fn trbl(top: f32, right: f32, bottom: f32, left: f32) -> EdgeInsets {
        EdgeInsets::new(left, top, right, bottom)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// StyleVariant — 按交互状态区分的样式集合
// ════════════════════════════════════════════════════════════════════════════

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

// ════════════════════════════════════════════════════════════════════════════
// `style!` 宏 — 声明式 Style 构建
// ════════════════════════════════════════════════════════════════════════════

/// 声明式 Style 构建宏。
///
/// 以类 CSS 的语法构建 `Style` 实例，支持快捷别名：
///
/// | 完整名 | 别名 | 说明 |
/// |--------|------|------|
/// | `background` | `bg` | 背景色 |
/// | `border_radius` | `rounded` | 圆角 |
/// | `font_size` | `fs` | 字号 |
/// | `flex_direction` | `direction` | Flex 方向 |
/// | `flex_grow` | `grow` | 扩展比例 |
/// | `flex_shrink` | `shrink` | 收缩比例 |
/// | `justify_content` | `justify` | 主轴对齐 |
/// | `align_items` | `align` | 交叉轴对齐 |
/// | `box_shadow` | `shadow` | 盒阴影 |
/// | `width` | `w` | 宽度 |
/// | `height` | `h` | 高度 |
///
/// # 语法
///
/// ```ignore
/// style! {
///     bg: RED,                    // Color 值 → background: Some(RED)
///     color: WHITE,
///     margin: 8,                  // f32 → EdgeInsets::uniform(8)
///     margin: [0, 8],             // [f32; 2] → EdgeInsets::new(8, 0, 8, 0) [top_bottom, left_right]
///     margin: [0, 8, 0, 8],       // [f32; 4] → EdgeInsets::new(8, 0, 8, 0) [top, right, bottom, left]
///     padding: 16,
///     border: [RED, 1],           // [Color, f32] → BorderLine
///     rounded: 6,                 // border_radius 的别名
///     fs: 14,                     // font_size 的别名
///     display: Flex,
///     direction: Row,             // flex_direction 的别名
///     gap: 8,
///     shadow: [BLACK, 4, 2, 2],   // [Color, blur, offset_x, offset_y]
///     grow: 1,                    // flex_grow 的别名
///     shrink: 0,                  // flex_shrink 的别名
///     w: 200,                     // width 的别名
///     h: 100,                     // height 的别名
///     visible: true,
/// }
/// ```
#[macro_export]
macro_rules! style {
    // 单对 key: value
    (@inner $s:ident bg $v:expr) => { $s.background = Some($v.into()); };
    (@inner $s:ident background $v:expr) => { $s.background = Some($v.into()); };
    (@inner $s:ident background_hover $v:expr) => { $s.background_hover = Some($v.into()); };
    (@inner $s:ident background_active $v:expr) => { $s.background_active = Some($v.into()); };
    (@inner $s:ident color $v:expr) => { $s.color = $v.into(); };
    (@inner $s:ident fs $v:expr) => { $s.font_size = $v as f32; };
    (@inner $s:ident font_size $v:expr) => { $s.font_size = $v as f32; };
    (@inner $s:ident opacity $v:expr) => { $s.opacity = $v as f32; };
    (@inner $s:ident visible $v:expr) => { $s.visible = $v; };
    (@inner $s:ident w $v:expr) => { $s.width = Some($v as f32); };
    (@inner $s:ident width $v:expr) => { $s.width = Some($v as f32); };
    (@inner $s:ident h $v:expr) => { $s.height = Some($v as f32); };
    (@inner $s:ident height $v:expr) => { $s.height = Some($v as f32); };
    (@inner $s:ident margin $v:expr) => { $s.margin = $crate::style::edge_insets_from_expr($v); };
    (@inner $s:ident padding $v:expr) => { $s.padding = $crate::style::edge_insets_from_expr($v); };
    (@inner $s:ident border $v:expr) => {
        let (color, width_val): (uix_graphics::Color, f32) = $v;
        $s.border_color = Some(color);
        $s.border_width = width_val;
    };
    (@inner $s:ident border_color $v:expr) => { $s.border_color = Some($v.into()); };
    (@inner $s:ident border_width $v:expr) => { $s.border_width = $v as f32; };
    (@inner $s:ident rounded $v:expr) => { $s.border_radius = $v as f32; };
    (@inner $s:ident border_radius $v:expr) => { $s.border_radius = $v as f32; };
    (@inner $s:ident display $v:expr) => { $s.display = $v; };
    (@inner $s:ident direction $v:expr) => { $s.flex_direction = $v; };
    (@inner $s:ident flex_direction $v:expr) => { $s.flex_direction = $v; };
    (@inner $s:ident flex_wrap $v:expr) => { $s.flex_wrap = $v; };
    (@inner $s:ident wrap $v:expr) => { $s.flex_wrap = $v; };
    (@inner $s:ident justify $v:expr) => { $s.justify_content = $v; };
    (@inner $s:ident justify_content $v:expr) => { $s.justify_content = $v; };
    (@inner $s:ident align $v:expr) => { $s.align_items = $v; };
    (@inner $s:ident align_items $v:expr) => { $s.align_items = $v; };
    (@inner $s:ident align_self $v:expr) => { $s.align_self = Some($v); };
    (@inner $s:ident gap $v:expr) => { $s.gap = $v as f32; };
    (@inner $s:ident grow $v:expr) => { $s.flex_grow = $v as f32; };
    (@inner $s:ident flex_grow $v:expr) => { $s.flex_grow = $v as f32; };
    (@inner $s:ident shrink $v:expr) => { $s.flex_shrink = $v as f32; };
    (@inner $s:ident flex_shrink $v:expr) => { $s.flex_shrink = $v as f32; };
    (@inner $s:ident shadow $v:expr) => {
        let (sc, sb, sox, soy): (uix_graphics::Color, f32, f32, f32) = $v;
        $s.box_shadow = Some($crate::style::BoxShadowDef::new(sc, sb, sox, soy));
    };
    (@inner $s:ident box_shadow $v:expr) => { $s.box_shadow = Some($v); };

    // 递归处理多对
    (@each $s:ident $key:ident : $val:expr, $($rest:tt)*) => {
        $crate::style!(@inner $s $key $val);
        $crate::style!(@each $s $($rest)*);
    };
    (@each $s:ident $key:ident : $val:expr) => {
        $crate::style!(@inner $s $key $val);
    };
    (@each $s:ident $key:ident : [$($v:tt),+], $($rest:tt)*) => {
        $crate::style!(@inner $s $key [$($v),+]);
        $crate::style!(@each $s $($rest)*);
    };
    (@each $s:ident $key:ident : [$($v:tt),+]) => {
        $crate::style!(@inner $s $key [$($v),+]);
    };
    (@each $s:ident,) => {};
    (@each $s:ident) => {};

    // 入口
    ($($key:ident : $val:expr),+ $(,)?) => {
        {
            let mut __style = $crate::style::Style::default();
            $(
                $crate::style!(@inner __style $key $val);
            )+
            __style
        }
    };
    ($($key:ident : [$($v:tt),+]),+ $(,)?) => {
        {
            let mut __style = $crate::style::Style::default();
            $(
                $crate::style!(@inner __style $key [$($v),+]);
            )+
            __style
        }
    };
    () => {
        $crate::style::Style::default()
    };
}

/// 辅助函数：将各种形式的输入统一转为 EdgeInsets（供 `style!` 宏内部使用）。
#[doc(hidden)]
pub fn edge_insets_from_expr(v: impl Into<EdgeInsets>) -> EdgeInsets {
    v.into()
}

/// 辅助函数：将 f32 转为 EdgeInsets（uniform），供 Builder 方法使用。
pub fn uniform_insets(v: f32) -> EdgeInsets {
    EdgeInsets::uniform(v)
}
