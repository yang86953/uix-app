//! UIX Layout — Flexbox 和 Grid 布局引擎。
//!
//! 布局入口是 [`LayoutEngine`] trait 及其实现 [`FlexLayout`] / [`GridLayout`]。
//!
//! 推荐用法：容器 widget 组合 `FlexLayout` 或 `GridLayout`，
//! 在 `layout_children` 中调用 `LayoutEngine::layout()` 获取子节点位置。

use uix_platform::{EdgeInsets, Rect, Size};
use std::f32;

// 内部模块（仅同 crate 内部使用，不对外公开）
pub(crate) mod flex;
pub(crate) mod grid;
pub mod engine;

// 重新导出统一布局引擎的核心类型
pub use engine::{
    BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutEngine, LayoutOutput, child_from_tree,
};

// ── 枚举类型（新旧引擎共用）──

/// Flex container direction。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Main-axis alignment。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

/// Cross-axis alignment。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

// ── 旧版布局类型（pub(crate)，仅内部兼容使用，不对外暴露）──

/// 单个 flex 子项的弹性属性。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlexChild {
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Option<f32>,
    pub align_self: Option<AlignItems>,
    pub min_size: Size,
    pub max_size: Size,
}

impl Default for FlexChild {
    fn default() -> Self {
        Self {
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            align_self: None,
            min_size: Size::zero(),
            max_size: Size::infinite(),
        }
    }
}

/// 旧版 flex 布局输入（内部兼容）。
#[derive(Debug, Clone)]
pub(crate) struct FlexInput {
    pub direction: FlexDirection,
    pub wrap: bool,
    pub gap: f32,
    pub padding: EdgeInsets,
    pub container: Rect,
    pub children: Vec<FlexChild>,
    pub child_sizes: Vec<Size>,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
}

impl Default for FlexInput {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: false,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            container: Rect::zero(),
            children: Vec::new(),
            child_sizes: Vec::new(),
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
        }
    }
}

/// 旧版 flex 布局输出（内部兼容）。
#[derive(Debug, Clone)]
pub(crate) struct FlexOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}

// ── Grid 内部类型 ──

/// Grid track（列/行尺寸定义），公开供 widget 使用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    Px(f32),
    Fr(f32),
    Auto,
}

/// 旧版 grid 布局子项（内部兼容）。
#[derive(Debug, Clone)]
pub(crate) struct GridChild {
    pub cell: usize,
    pub col_span: u32,
    pub row_span: u32,
    pub preferred_size: Size,
    pub align: Option<AlignItems>,
    pub justify: Option<JustifyContent>,
}

impl Default for GridChild {
    fn default() -> Self {
        Self { cell: 0, col_span: 1, row_span: 1, preferred_size: Size::zero(), align: None, justify: None }
    }
}

/// 旧版 grid 布局输入（内部兼容）。
#[derive(Debug, Clone)]
pub(crate) struct GridInput {
    pub container: Rect,
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub col_gap: f32,
    pub row_gap: f32,
    pub padding: EdgeInsets,
    pub children: Vec<GridChild>,
    pub align_items: AlignItems,
    pub justify_items: JustifyContent,
}

impl Default for GridInput {
    fn default() -> Self {
        Self {
            container: Rect::zero(), columns: Vec::new(), rows: Vec::new(),
            col_gap: 0.0, row_gap: 0.0, padding: EdgeInsets::zero(),
            children: Vec::new(), align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
        }
    }
}

/// 旧版 grid 布局输出（内部兼容）。
#[derive(Debug, Clone)]
pub(crate) struct GridOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}
