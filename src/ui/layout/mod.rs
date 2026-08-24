//! UIX Layout — Flexbox 和 Grid 布局引擎。
//!
//! 布局入口是 [`LayoutEngine`] trait 及其实现 [`FlexLayout`] / [`GridLayout`]。
//!
//! 推荐用法：容器 widget 组合 `FlexLayout` 或 `GridLayout`，
//! 在 `layout_children` 中调用 `LayoutEngine::layout()` 获取子节点位置。

use crate::core::{EdgeInsets, Rect, Size};
use std::f32;

// 内部模块（仅同 crate 内部使用，不对外公开）
pub(crate) mod engine;
pub(crate) mod flex;
pub(crate) mod grid;

// 重新导出统一布局引擎的核心类型
pub use engine::{
    BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutEngine, LayoutEngineScratch, LayoutOutput,
};

// ── 枚举类型（布局引擎共用）──

/// Flex container direction。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    /// 沿水平主轴从起点向终点排列。
    Row,
    /// 沿垂直主轴从起点向终点排列。
    Column,
    /// 沿水平主轴从终点向起点反向排列。
    RowReverse,
    /// 沿垂直主轴从终点向起点反向排列。
    ColumnReverse,
}

/// Main-axis alignment。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    /// 将内容贴靠主轴起点。
    Start,
    /// 将内容置于主轴中央。
    Center,
    /// 将内容贴靠主轴终点。
    End,
    /// 均分项目之间的剩余空间，首尾不留额外间距。
    SpaceBetween,
    /// 均分项目两侧空间，使首尾间距为项目间距的一半。
    SpaceAround,
    /// 让首尾和项目之间具有相等间距。
    SpaceEvenly,
    /// 将主轴剩余空间分配给可拉伸内容。
    Stretch,
}

/// Cross-axis alignment。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    /// 将项目贴靠交叉轴起点。
    Start,
    /// 将项目置于交叉轴中央。
    Center,
    /// 将项目贴靠交叉轴终点。
    End,
    #[default]
    /// 将项目拉伸到交叉轴可用尺寸。
    Stretch,
}

// ── 布局求解内部类型（pub(crate)，不对外暴露）──

/// 单个 flex 子项的弹性属性。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlexChild {
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Option<f32>,
    pub align_self: Option<AlignItems>,
    pub min_size: Size,
    pub max_size: Size,
    pub measured_size: Size,
    pub margin: EdgeInsets,
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
            measured_size: Size::zero(),
            margin: EdgeInsets::zero(),
        }
    }
}

/// flex 布局输入。
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlexInput<'a> {
    pub direction: FlexDirection,
    pub wrap: bool,
    pub gap: f32,
    pub padding: EdgeInsets,
    pub container: Rect,
    pub children: &'a [FlexChild],
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    /// 主轴无显式尺寸时由子项撑开（Web 式 intrinsic），仍保留 flex-grow 分配。
    pub intrinsic_main: bool,
    /// 交叉轴无显式尺寸时由最宽或最高子项撑开，禁止暂存 frame 反向压缩子项。
    pub intrinsic_cross: bool,
}

impl Default for FlexInput<'_> {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: false,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            container: Rect::zero(),
            children: &[],
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            intrinsic_main: false,
            // 默认调用方把容器交叉轴视为父级已经确定。
            intrinsic_cross: false,
        }
    }
}

/// flex 布局输出。
#[derive(Debug, Clone)]
pub(crate) struct FlexOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}

/// 同一布局帧内复用的 Flex 求解工作区。
#[derive(Default)]
pub(crate) struct FlexComputeScratch {
    pub(crate) base_main_sizes: Vec<f32>,
    pub(crate) cross_sizes: Vec<f32>,
    pub(crate) child_rects: Vec<Rect>,
}

// ── Grid 内部类型 ──

/// Grid track（列/行尺寸定义），公开供 widget 使用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    /// 使用固定逻辑像素尺寸的轨道。
    Px(f32),
    /// 按权重分配剩余空间的弹性轨道。
    Fr(f32),
    /// 根据轨道内容的固有尺寸自动确定大小。
    Auto,
}

/// grid 布局子项。
#[derive(Debug, Clone)]
pub(crate) struct GridChild {
    pub cell: Option<usize>,
    pub col_span: u32,
    pub row_span: u32,
    pub measured_size: Size,
    pub margin: EdgeInsets,
    pub align: Option<AlignItems>,
    pub justify: Option<JustifyContent>,
}

impl Default for GridChild {
    fn default() -> Self {
        Self {
            cell: None,
            col_span: 1,
            row_span: 1,
            measured_size: Size::zero(),
            margin: EdgeInsets::zero(),
            align: None,
            justify: None,
        }
    }
}

/// grid 布局输入。
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridInput<'a> {
    pub container: Rect,
    pub columns: &'a [GridTrack],
    pub rows: &'a [GridTrack],
    pub col_gap: f32,
    pub row_gap: f32,
    pub padding: EdgeInsets,
    pub children: &'a [GridChild],
    pub align_items: AlignItems,
    pub justify_items: JustifyContent,
    // 整组列轨在父级水平剩余空间中的对齐方式。
    pub justify_content: JustifyContent,
}

impl Default for GridInput<'_> {
    fn default() -> Self {
        Self {
            container: Rect::zero(),
            columns: &[],
            rows: &[],
            col_gap: 0.0,
            row_gap: 0.0,
            padding: EdgeInsets::zero(),
            children: &[],
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
            // 默认保持列轨组贴近水平起点。
            justify_content: JustifyContent::Start,
        }
    }
}

/// grid 布局输出。
#[derive(Debug, Clone)]
pub(crate) struct GridOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}
