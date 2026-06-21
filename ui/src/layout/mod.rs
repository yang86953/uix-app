//! UIX Layout — 纯函数式 Flexbox 和 Grid 布局引擎。
//!
//! 与 Web CSS Flexbox/Grid 行为一致，无副作用，无渲染依赖。
//! 输入布局约束，输出子节点位置。

use uix_core::{EdgeInsets, Rect, Size};
use std::f32;

pub mod flex;
pub mod grid;

// ── Shared enums ──

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

/// Individual child flex properties。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexChild {
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

/// Input to the flex layout computation。
#[derive(Debug, Clone)]
pub struct FlexInput {
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

/// Output from the flex layout computation。
#[derive(Debug, Clone)]
pub struct FlexOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}

// ── Grid types ──

/// A single grid track (column or row) sizing。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    Px(f32),
    Fr(f32),
    Auto,
}

/// A child in a grid, with optional column/row span。
#[derive(Debug, Clone)]
pub struct GridChild {
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

/// Input to the grid layout computation。
#[derive(Debug, Clone)]
pub struct GridInput {
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

/// Output from the grid layout computation。
#[derive(Debug, Clone)]
pub struct GridOutput {
    pub child_rects: Vec<Rect>,
    pub col_positions: Vec<(f32, f32)>,
    pub row_positions: Vec<(f32, f32)>,
    pub total_size: Size,
}
