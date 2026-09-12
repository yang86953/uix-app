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
mod methods;
// 定义背景图来源、定位与重复的纯值契约。
mod background;
// 定义四角圆角半径的纯值契约。
mod corners;
// 定义边框线型及其有效默认语义。
mod border;
// 定义有序字体族列表及其显式覆盖语义。
mod font_family;
// 定义一百到九百的精确字体粗细与字体面选择语义。
mod font_weight;
// 定义行高单位与字体尺寸解析语义。
mod line_height;
// 定义保留单位的长度、尺寸约束与百分比参照语义。
mod length;
// 定义闭合文本水平对齐及显式 left 语义。
mod text_align;
// 定义闭合文本装饰及显式 none 语义。
mod text_decoration;
mod types;
mod variant;

// 公开逐字段类型化差异声明，供状态层与覆盖层区分未声明与显式默认值。
mod diff;

// 公开 UI System 自有的边框线型契约。
pub use self::border::BorderStyle;
// 公开 UI System 自有的四角圆角值契约。
pub use self::corners::CornerRadii;
// 公开 UI System 自有的背景图层值契约。
pub use self::background::{
    GradientStopValue,
    BackgroundAxisPosition, BackgroundAxisSize, BackgroundImage, BackgroundPosition,
    BackgroundRepeat, BackgroundSize,
};
// 公开 UI System 自有的字体族列表契约。
pub use self::font_family::FontFamily;
// 公开 UI System 自有的字体粗细契约。
pub use self::font_weight::FontWeight;
// 公开 UI System 自有的行高值契约。
pub use self::line_height::LineHeight;
// 公开保留单位的长度与尺寸约束契约。
pub use self::length::{PercentReference, ResolvedSizeBounds, SizeConstraints, StyleLength};
// 公开 UI System 自有的文本水平对齐契约。
pub use self::text_align::TextAlign;
// 公开 UI System 自有的文本装饰契约。
pub use self::text_decoration::TextDecoration;
pub use self::types::{BoxShadowDef, ColorValue, DisplayMode, TypographyToken};

pub use crate::ui::style_paint::apply_style;
pub use diff::StyleDiff;
pub use variant::{DeclaredStyleSet, StateFlags, StyleSet, StyleState};

use crate::core::EdgeInsets;
use crate::draw::Color;

use crate::ui::theme::traits::ThemeTokens;
// Re-export layout enums so crate::ui::theme::style::FlexDirection etc. work
pub use crate::ui::layout::{
    AlignItems, FlexDirection, GridTrack, GridTrackMax, GridTrackMin, JustifyContent,
};

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
    /// 显式边框线型；None 表示使用 CSS 默认 solid。
    pub border_style: Option<BorderStyle>,
    /// 边框圆角（单值形式；显式写入该值会覆盖四角声明）。
    ///
    /// 与 `border_radius_corners` 表示同一个 `borderRadius` 属性的两种输入，
    /// 不做叠加：任一形式的显式声明都使另一形式失效；最终有效圆角统一由
    /// [`Style::effective_border_radius`] 解析。
    pub border_radius: f32,
    /// 四角圆角半径（tl/tr/br/bl）；`None` 表示未使用四角形式。
    ///
    /// 迁移说明（S4 新增公开字段）：`Style`/`StyleDiff` 的全字段结构体字面量
    /// 构造需要补齐本字段或改用 `..Default::default()`；builder 与字段读写
    /// 不受影响。
    pub border_radius_corners: Option<CornerRadii>,

    // ── 尺寸 ────────────────────────────────────────────────
    /// 固定宽度（None = 由子内容决定）
    pub width: Option<f32>,
    /// 固定高度（None = 由子内容决定）
    pub height: Option<f32>,
    /// 最小宽度；`Auto` 表示不约束，百分比参照父内容盒宽度。
    pub min_width: StyleLength,
    /// 最大宽度；`Auto` 表示不约束，百分比参照父内容盒宽度。
    pub max_width: StyleLength,
    /// 最小高度；`Auto` 表示不约束，百分比参照父内容盒高度。
    pub min_height: StyleLength,
    /// 最大高度；`Auto` 表示不约束，百分比参照父内容盒高度。
    pub max_height: StyleLength,

    // ── 弹性布局（容器属性）──
    /// 显示模式
    pub display: DisplayMode,
    /// Flex 主轴方向
    pub flex_direction: FlexDirection,
    /// Flex 是否换行
    pub flex_wrap: bool,
    /// 允许子内容溢出容器（流式堆叠，跳过 flex-shrink）
    pub overflow_content: bool,
    /// 子树裁剪声明；None 表示沿用组件默认不裁剪。
    pub clip_content: Option<bool>,
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
    /// 显式背景图来源；None 表示未声明。
    pub background_image: Option<BackgroundImage>,
    /// 显式背景定位；None 表示使用左上角。
    pub background_position: Option<BackgroundPosition>,
    /// 显式背景重复方式；None 表示两个轴重复。
    pub background_repeat: Option<BackgroundRepeat>,
    /// 背景图尺寸策略；`Auto`（默认）表示固有尺寸，也是显式恢复值。
    ///
    /// 只作用于图片来源；渐变始终填满背景盒。S4 新增公开字段，全字段
    /// 字面量构造的迁移说明同 `border_radius_corners`。
    pub background_size: BackgroundSize,
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
    /// 显式字体族回退列表；None 保留当前系统字体。
    pub font_family: Option<FontFamily>,
    /// 显式字体粗细；None 保留组件自身默认字重。
    pub font_weight: Option<FontWeight>,
    /// 显式行高；None 保持组件既有 normal 行高。
    pub line_height: Option<LineHeight>,
    /// 显式文本水平对齐；None 保留组件自身默认语义。
    pub text_align: Option<TextAlign>,
    /// 显式文本装饰；None 保留组件自身装饰语义。
    pub text_decoration: Option<TextDecoration>,
    /// 整体透明度
    pub opacity: f32,
    /// 盒阴影
    pub box_shadow: Option<BoxShadowDef>,
    /// 多外阴影，首项在最上层；Some(empty) 显式清除，优先于单阴影。最多八层。
    pub box_shadows: Option<Vec<BoxShadowDef>>,

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
            // 未显式声明时保持 CSS 默认实线。
            border_style: None,
            border_radius: 0.0,
            // 未使用四角形式时由单值或直角默认值表达。
            border_radius_corners: None,

            width: None,
            height: None,
            // 未声明时四个方向都不施加尺寸约束。
            min_width: StyleLength::Auto,
            max_width: StyleLength::Auto,
            min_height: StyleLength::Auto,
            max_height: StyleLength::Auto,

            display: DisplayMode::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: false,
            overflow_content: false,
            // 默认不改变组件既有子树裁剪语义。
            clip_content: None,
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
            // 未声明时保留 CSS 的无背景图默认值。
            background_image: None,
            // 未声明时由有效值方法提供左上角定位。
            background_position: None,
            // 未声明时由有效值方法提供双轴重复。
            background_repeat: None,
            // 未声明尺寸策略时图片保持固有尺寸。
            background_size: BackgroundSize::Auto,
            background_hover: None,
            background_focus: None,
            background_active: None,
            color: ColorValue::default(),
            font_size: TypographyToken::default(),
            // 未声明时保留绘制上下文当前系统字体。
            font_family: None,
            // 未声明时由各文本组件保留常规、标题或 strong 字重。
            font_weight: None,
            // 未声明时由各文本组件保持既有 normal 行高。
            line_height: None,
            // 未声明时由各文本组件保持既有左对齐语义。
            text_align: None,
            // 未声明时由各文本组件保持既有装饰语义。
            text_decoration: None,
            opacity: 1.0,
            box_shadow: None,
            box_shadows: None,

            visible: true,
        }
    }
}
