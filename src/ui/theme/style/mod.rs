//! Style — 统一的类 CSS 样式系统。
//!
//! 融合了原有的 `Style` 和 `WidgetStylePreset`，增加状态变体（hover/focus/active）、
//! 盒阴影、边距、Flex/Grid 布局属性和主题感知能力。
//!
//! # 设计原则
//!
//! - Style 是纯数据（无方法，只有字段 + 构造器/Builder）
//! - 每个 widget 可选持有一个 `Style`，render 时通过 `apply_style(ctx, ...)` 应用
//! - 状态变体在 widget 内部由事件更新，Style 只定义各状态的色值
//! - 主题感知：Style 可从 `TokenProvider` 获取默认值，用户可选择性覆盖
//!
//! # Style 作为组件唯一视觉契约
//!
//! 所有组件不再保有 `bg_color`/`border_color`/`padding` 等独立字段，
//! 统一使用 `style: Style`。布局引擎从 `style.margin` 读取外边距参与盒模型计算。

pub mod edge_insets;
mod variant;
mod methods;
mod types;

pub use self::types::{BoxShadowDef, ColorValue, DisplayMode, PaletteColor, TypographyToken};

pub use crate::ui::style_paint::apply_style;
pub use variant::{StyleSet, StyleState};

use crate::core::EdgeInsets;
use crate::draw::Color;
use crate::ui::theme::traits::ThemeTokens;
use crate::ui::theme::NeutralRole;
// Re-export layout enums so crate::ui::theme::style::FlexDirection etc. work
pub use crate::ui::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};

/// 类 CSS 视觉样式——覆盖 widget 常见的视觉属性。
///
/// widget 在 `render` 中通过 `apply_style(ctx, rect, &self.style)` 统一绘制
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
    pub border_color: Option<ColorValue>,
    /// 四边边框宽度。
    pub border_width: EdgeInsets,
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
    /// 允许子内容溢出容器（流式堆叠，跳过 flex-shrink）
    pub overflow_content: bool,
    /// 主轴对齐
    pub justify_content: JustifyContent,
    /// 交叉轴对齐
    pub align_items: AlignItems,
    /// 子项间距
    pub gap: f32,
    /// Grid 列轨道模板
    pub grid_template_columns: Vec<GridTrack>,
    /// Grid 行轨道模板
    pub grid_template_rows: Vec<GridTrack>,
    /// Grid 列间距；0 时可由 `gap` 兜底
    pub grid_column_gap: f32,
    /// Grid 行间距；0 时可由 `gap` 兜底
    pub grid_row_gap: f32,

    // ── 弹性布局（子项属性）──
    /// 扩展比例
    pub flex_grow: f32,
    /// 收缩比例
    pub flex_shrink: f32,
    /// 子项单独覆盖 align_items
    pub align_self: Option<AlignItems>,
    /// Grid child explicit cell; None means auto-place.
    pub grid_cell: Option<usize>,
    /// Grid child column span.
    pub grid_column_span: u32,
    /// Grid child row span.
    pub grid_row_span: u32,

    // ── 视觉 ────────────────────────────────────────────────
    /// 背景色
    pub background: Option<ColorValue>,
    /// 悬停状态背景色
    pub background_hover: Option<ColorValue>,
    /// 焦点状态背景色
    pub background_focus: Option<ColorValue>,
    /// 按下/激活状态背景色
    pub background_active: Option<ColorValue>,
    /// 文字颜色
    pub color: ColorValue,
    /// 字号
    pub font_size: TypographyToken,
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
            border_width: EdgeInsets::zero(),
            border_radius: 0.0,

            width: None,
            height: None,

            display: DisplayMode::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: false,
            overflow_content: false,
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            gap: 0.0,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column_gap: 0.0,
            grid_row_gap: 0.0,

            flex_grow: 0.0,
            flex_shrink: 1.0,
            align_self: None,
            grid_cell: None,
            grid_column_span: 1,
            grid_row_span: 1,

            background: None,
            background_hover: None,
            background_focus: None,
            background_active: None,
            color: ColorValue::Neutral(NeutralRole::Text),
            font_size: TypographyToken::Body,
            opacity: 1.0,
            box_shadow: None,

            visible: true,
        }
    }
}
