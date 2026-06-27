//! 统一布局引擎 — FlexBox 和 Grid 的组件化封装。
//!
//! 将原本的纯函数 compute_flex_layout / compute_grid_layout 重新设计为
//! 可被组合使用的布局组件，提供统一的 LayoutEngine trait 接口。
//!
//! 盒模型计算也统一在此层，所有容器组件通过 BoxModel 获得一致的内框计算。

use uix_platform::{Rect, Size};
use crate::widget::{WidgetCore, WidgetId, WidgetTree};
use super::{AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent};
use super::{GridChild, GridInput, GridTrack};
use super::flex::compute_flex_layout;
use super::grid::compute_grid_layout;

// ── 统一盒模型 ────────────────────────────────────────────────────

/// 统一盒模型 — 所有容器共享的 margin/border/padding 计算。
///
/// 与 Web CSS 盒模型一致：frame → 扣除 margin → 扣除 border → 扣除 padding → 内容区域。
#[derive(Debug, Clone, Copy)]
pub struct BoxModel {
    pub margin: uix_platform::EdgeInsets,
    pub border_width: f32,
    pub padding: uix_platform::EdgeInsets,
}

impl BoxModel {
    pub const ZERO: Self = Self {
        margin: uix_platform::EdgeInsets::zero(),
        border_width: 0.0,
        padding: uix_platform::EdgeInsets::zero(),
    };

    /// 从 frame 计算内框（扣除 margin + border + padding）。
    pub fn content_rect(&self, frame: Rect) -> Rect {
        let mh = self.margin.horizontal();
        let mv = self.margin.vertical();
        let bh = self.border_width * 2.0;
        let bv = self.border_width * 2.0;
        Rect::new(
            frame.x + self.margin.left + self.border_width + self.padding.left,
            frame.y + self.margin.top + self.border_width + self.padding.top,
            (frame.w - mh - bh - self.padding.horizontal()).max(0.0),
            (frame.h - mv - bv - self.padding.vertical()).max(0.0),
        )
    }

    /// 从 frame 计算视觉区域（仅扣除 margin，供渲染用）。
    pub fn visual_rect(&self, frame: Rect) -> Rect {
        Rect::new(
            frame.x + self.margin.left,
            frame.y + self.margin.top,
            (frame.w - self.margin.horizontal()).max(0.0),
            (frame.h - self.margin.vertical()).max(0.0),
        )
    }
}

// ── 统一子节点信息 ─────────────────────────────────────────────────

/// 统一子节点布局信息，Flex 和 Grid 引擎共用。
///
/// grid_* 字段仅在 Grid 引擎中使用（Flex 引擎忽略）。
#[derive(Debug, Clone)]
pub struct LayoutChild {
    pub id: WidgetId,
    pub preferred_size: Size,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    /// Grid: 起始单元格索引
    pub grid_cell: usize,
    /// Grid: 列跨度
    pub grid_col_span: u32,
    /// Grid: 行跨度
    pub grid_row_span: u32,
}

impl LayoutChild {
    pub fn new(id: WidgetId, preferred_size: Size) -> Self {
        Self {
            id,
            preferred_size,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            grid_cell: 0,
            grid_col_span: 1,
            grid_row_span: 1,
        }
    }

    pub fn with_flex(mut self, grow: f32, shrink: f32) -> Self {
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self
    }
}

// ── 布局引擎 trait ─────────────────────────────────────────────────

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
///
/// 调用者负责盒模型计算（通过 BoxModel::content_rect），
/// 引擎在给定的内容区域内排列子节点。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    ///
    /// `content_rect` 是已扣除 margin/border/padding 的内部区域。
    /// 返回的子节点坐标相对于 `content_rect`。
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput;
}

/// 布局引擎的输出：子节点位置 + 内容总尺寸。
#[derive(Debug, Clone)]
pub struct LayoutOutput {
    pub positions: Vec<Rect>,
    pub total_size: Size,
}

// ── FlexBox 布局引擎 ───────────────────────────────────────────────

/// FlexBox 布局引擎组件。
///
/// 封装 FlexInput 构建和 compute_flex_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub gap: f32,
    pub justify: JustifyContent,
    pub align: AlignItems,
    pub wrap: bool,
    /// 允许内容溢出（跳过 flex-shrink，子节点按自然尺寸流式堆叠）
    pub overflow_content: bool,
}

impl FlexLayout {
    pub fn new() -> Self {
        Self {
            direction: FlexDirection::Column,
            gap: 0.0,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            wrap: false,
            overflow_content: false,
        }
    }

    /// 便捷构造：垂直堆叠。
    pub fn column() -> Self {
        Self::new()
    }

    /// 便捷构造：水平排列。
    pub fn row() -> Self {
        Self { direction: FlexDirection::Row, ..Self::new() }
    }

    pub fn with_direction(mut self, d: FlexDirection) -> Self { self.direction = d; self }
    pub fn with_gap(mut self, g: f32) -> Self { self.gap = g; self }
    pub fn with_justify(mut self, j: JustifyContent) -> Self { self.justify = j; self }
    pub fn with_align(mut self, a: AlignItems) -> Self { self.align = a; self }
}

impl Default for FlexLayout {
    fn default() -> Self { Self::new() }
}

impl LayoutEngine for FlexLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        if children.is_empty() {
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            };
        }

        // 溢出模式：使用简单流式堆叠
        if self.overflow_content {
            return overflow_layout(self, content_rect, children);
        }

        // 标准 FlexBox 模式
        let child_sizes: Vec<Size> = children.iter()
            .map(|c| c.preferred_size)
            .collect();

        let flex_children: Vec<FlexChild> = children.iter()
            .map(|c| FlexChild {
                flex_grow: c.flex_grow,
                flex_shrink: c.flex_shrink,
                ..FlexChild::default()
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            gap: self.gap,
            padding: uix_platform::EdgeInsets::zero(),
            container: content_rect,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);

        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
    }
}

/// 溢出模式：流式堆叠（不压缩，不分配剩余空间）。
fn overflow_layout(engine: &FlexLayout, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
    let is_row = matches!(engine.direction, FlexDirection::Row | FlexDirection::RowReverse);
    let is_reverse = matches!(engine.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);
    let count = children.len();
    let gap = engine.gap;

    let container_main = if is_row { content_rect.w } else { content_rect.h };
    let container_cross = if is_row { content_rect.h } else { content_rect.w };

    let main_sizes: Vec<f32> = children.iter()
        .map(|c| {
            let pref = if is_row { c.preferred_size.w } else { c.preferred_size.h };
            pref.max(0.0)
        })
        .collect();

    let total_main: f32 = main_sizes.iter().sum::<f32>() + gap * (count as f32 - 1.0).max(0.0);
    let total_size = if is_row {
        Size::new(total_main, container_cross)
    } else {
        Size::new(container_cross, total_main)
    };

    let mut cursor = if is_reverse { container_main - total_main } else { 0.0 };
    let mut positions = Vec::with_capacity(count);

    for i in 0..count {
        let main = main_sizes[i];
        let cross_size = if is_row {
            children[i].preferred_size.h
        } else {
            children[i].preferred_size.w
        };

        let child_cross = if engine.align == AlignItems::Stretch {
            container_cross
        } else {
            cross_size
        };

        let cross_offset = match engine.align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (container_cross - child_cross) / 2.0,
            AlignItems::End => container_cross - child_cross,
            AlignItems::Stretch => 0.0,
        };

        let (x, y, w, h) = if is_row {
            (content_rect.x + cursor, content_rect.y + cross_offset, main, child_cross)
        } else {
            (content_rect.x + cross_offset, content_rect.y + cursor, child_cross, main)
        };

        positions.push(Rect::new(x, y, w, h));

        if is_reverse {
            cursor -= main + gap;
        } else {
            cursor += main + gap;
        }
    }

    LayoutOutput { positions, total_size }
}

// ── Grid 布局引擎 ─────────────────────────────────────────────────

/// Grid 布局引擎组件。
///
/// 封装 GridInput 构建和 compute_grid_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct GridLayout {
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub col_gap: f32,
    pub row_gap: f32,
    pub align_items: AlignItems,
    pub justify_items: JustifyContent,
}

impl GridLayout {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: 0.0,
            row_gap: 0.0,
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
        }
    }

    pub fn with_columns(mut self, cols: Vec<GridTrack>) -> Self { self.columns = cols; self }
    pub fn with_rows(mut self, rows: Vec<GridTrack>) -> Self { self.rows = rows; self }
    pub fn with_gap(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.col_gap = col_gap;
        self.row_gap = row_gap;
        self
    }
    pub fn with_align(mut self, a: AlignItems) -> Self { self.align_items = a; self }
    pub fn with_justify(mut self, j: JustifyContent) -> Self { self.justify_items = j; self }
}

impl Default for GridLayout {
    fn default() -> Self { Self::new() }
}

impl LayoutEngine for GridLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        if self.columns.is_empty() || children.is_empty() {
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            };
        }

        // 自动计算行数
        let rows: Vec<GridTrack> = if self.rows.is_empty() {
            let n_cols = self.columns.len();
            let n_rows = (children.len() + n_cols - 1) / n_cols;
            vec![GridTrack::Auto; n_rows.max(1)]
        } else {
            self.rows.clone()
        };

        let grid_children: Vec<GridChild> = children.iter()
            .map(|c| GridChild {
                cell: c.grid_cell,
                col_span: c.grid_col_span,
                row_span: c.grid_row_span,
                preferred_size: c.preferred_size,
                align: None,
                justify: None,
            })
            .collect();

        let output = compute_grid_layout(&GridInput {
            container: content_rect,
            columns: self.columns.clone(),
            rows,
            col_gap: self.col_gap,
            row_gap: self.row_gap,
            padding: uix_platform::EdgeInsets::zero(),
            children: grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
        });

        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
    }
}

// ── 辅助：从 WidgetTree 构建 LayoutChild ──────────────────────────

/// 从 WidgetTree 节点构建统一的 LayoutChild。
pub fn child_from_tree(cid: WidgetId, tree: &WidgetTree) -> LayoutChild {
    let node = tree.get(cid);
    let pref = node.map(|c| c.preferred_size(None)).unwrap_or_default();
    let h = if pref.h > 0.0 { pref.h } else {
        node.map(|c| c.frame().h).unwrap_or(0.0)
    };

    let grow = node.map(|c| c.inner().flex_grow()).unwrap_or(0.0);
    let shrink = node.map(|c| c.inner().flex_shrink()).unwrap_or(1.0);

    LayoutChild {
        id: cid,
        preferred_size: Size::new(pref.w, h),
        flex_grow: grow,
        flex_shrink: shrink,
        grid_cell: 0,
        grid_col_span: 1,
        grid_row_span: 1,
    }
}
