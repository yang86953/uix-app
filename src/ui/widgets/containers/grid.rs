//! Grid widget - CSS Grid-like layout container.

use std::borrow::Cow;

use crate::component;
use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::scene::PicturePolicy;
use crate::draw::Color;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::layout::engine::{
    child_from_tree_with_constraints, BoxModel, GridLayout, LayoutChild,
};
use crate::ui::layout::{AlignItems, GridTrack, JustifyContent};
use crate::ui::style::{apply_style as paint_style, ColorValue, DisplayMode, Style};
use crate::ui::{ComponentId, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

const RESPONSIVE_GRID_UNITS: usize = 24;

/// 自定义响应式断点无效时返回的 typed 错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointError {
    NonFinite,
    Negative,
    NotStrictlyAscending,
}

impl std::fmt::Display for BreakpointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NonFinite => "breakpoints must be finite",
            Self::Negative => "breakpoints must be nonnegative",
            Self::NotStrictlyAscending => "breakpoints must be strictly ascending from xs=0",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for BreakpointError {}

/// 响应式 Grid 的 logical 宽度断点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoints {
    sm: f32,
    md: f32,
    lg: f32,
    xl: f32,
    xxl: f32,
}

impl Breakpoints {
    /// 构造严格递增的自定义断点；`xs` 固定为 `0`。
    pub fn new(sm: f32, md: f32, lg: f32, xl: f32, xxl: f32) -> Result<Self, BreakpointError> {
        let values = [sm, md, lg, xl, xxl];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(BreakpointError::NonFinite);
        }
        if values.iter().any(|value| *value < 0.0) {
            return Err(BreakpointError::Negative);
        }
        if sm <= 0.0 || values.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(BreakpointError::NotStrictlyAscending);
        }
        Ok(Self {
            sm,
            md,
            lg,
            xl,
            xxl,
        })
    }

    /// Ant Design 的默认断点，单位为 logical px。
    pub const fn antd() -> Self {
        Self {
            sm: 576.0,
            md: 768.0,
            lg: 992.0,
            xl: 1200.0,
            xxl: 1600.0,
        }
    }

    pub const fn xs(self) -> f32 {
        0.0
    }

    pub const fn sm(self) -> f32 {
        self.sm
    }

    pub const fn md(self) -> f32 {
        self.md
    }

    pub const fn lg(self) -> f32 {
        self.lg
    }

    pub const fn xl(self) -> f32 {
        self.xl
    }

    pub const fn xxl(self) -> f32 {
        self.xxl
    }
}

impl Default for Breakpoints {
    fn default() -> Self {
        Self::antd()
    }
}

/// 响应式 Grid 中与源顺序子节点一一对应的列配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Col {
    span: u8,
    sm: Option<u8>,
    md: Option<u8>,
    lg: Option<u8>,
    xl: Option<u8>,
    xxl: Option<u8>,
    offset: u8,
    order: i32,
}

impl Col {
    pub const fn new() -> Self {
        Self {
            span: RESPONSIVE_GRID_UNITS as u8,
            sm: None,
            md: None,
            lg: None,
            xl: None,
            xxl: None,
            offset: 0,
            order: 0,
        }
    }

    pub fn span(mut self, span: u32) -> Self {
        self.span = Self::normalize_span(span);
        self
    }

    pub fn sm(mut self, span: u32) -> Self {
        self.sm = Some(Self::normalize_span(span));
        self
    }

    pub fn md(mut self, span: u32) -> Self {
        self.md = Some(Self::normalize_span(span));
        self
    }

    pub fn lg(mut self, span: u32) -> Self {
        self.lg = Some(Self::normalize_span(span));
        self
    }

    pub fn xl(mut self, span: u32) -> Self {
        self.xl = Some(Self::normalize_span(span));
        self
    }

    pub fn xxl(mut self, span: u32) -> Self {
        self.xxl = Some(Self::normalize_span(span));
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = offset.min((RESPONSIVE_GRID_UNITS - 1) as u32) as u8;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub const fn base_span(self) -> u8 {
        self.span
    }

    pub const fn base_offset(self) -> u8 {
        self.offset
    }

    pub const fn visual_order(self) -> i32 {
        self.order
    }

    fn span_at(self, width: f32, breakpoints: Breakpoints) -> usize {
        let mut span = self.span;
        for (threshold, override_span) in [
            (breakpoints.sm, self.sm),
            (breakpoints.md, self.md),
            (breakpoints.lg, self.lg),
            (breakpoints.xl, self.xl),
            (breakpoints.xxl, self.xxl),
        ] {
            if width >= threshold {
                if let Some(value) = override_span {
                    span = value;
                }
            }
        }
        span as usize
    }

    fn effective_offset(self, span: usize) -> usize {
        (self.offset as usize).min(RESPONSIVE_GRID_UNITS - span)
    }

    fn normalize_span(span: u32) -> u8 {
        span.clamp(1, RESPONSIVE_GRID_UNITS as u32) as u8
    }
}

impl Default for Col {
    fn default() -> Self {
        Self::new()
    }
}

component! {
    /// Grid container widget.
    pub struct Grid {
        pub style: Style,
        breakpoints: Option<Breakpoints>,
        cols: Vec<Col>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.style.width.unwrap_or(0.0),
            self.style.height.unwrap_or(0.0),
        ))
    }

    layout_margin => (&self) -> EdgeInsets { self.style.margin }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    align_self => (&self) -> Option<AlignItems> { self.style.align_self }

    grid_cell => (&self) -> Option<usize> { self.style.grid_cell }

    grid_column_span => (&self) -> u32 { self.style.grid_column_span }

    grid_row_span => (&self) -> u32 { self.style.grid_row_span }

    picture_policy => (&self) -> PicturePolicy { PicturePolicy::Eligible }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let visual = Rect::new(
            frame.x + self.style.margin.left,
            frame.y + self.style.margin.top,
            (frame.w - self.style.margin.horizontal()).max(0.0),
            (frame.h - self.style.margin.vertical()).max(0.0),
        );
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }
        paint_style(ctx, visual, &self.style);
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            return Vec::new();
        }

        let content_rect = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        }
        .content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 {
            return Vec::new();
        }

        let constraints = Constraints::loose(Size::new(content_rect.w, content_rect.h));
        children
            .iter()
            .copied()
            .map(|cid| child_from_tree_with_constraints(cid, tree, constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if self.style.grid_template_columns.is_empty() || children.is_empty() {
            return Vec::new();
        }

        let box_model = BoxModel {
            margin: self.style.margin,
            border_width: self.style.border_width,
            padding: self.style.padding,
        };
        let content_rect = box_model.content_rect(frame);
        if content_rect.w <= 0.0 || content_rect.h <= 0.0 {
            return Vec::new();
        }

        let engine = GridLayout {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: self.effective_col_gap(),
            row_gap: self.effective_row_gap(),
            align_items: self.style.align_items,
            justify_items: self.style.justify_content,
        };

        let responsive_children = self.responsive_children(content_rect.w, children);
        let output = engine.layout_with_tracks(
            content_rect,
            &self.style.grid_template_columns,
            &self.style.grid_template_rows,
            &responsive_children,
        );

        responsive_children
            .iter()
            .zip(output.positions)
            .map(|(child, rect)| (child.id, rect))
            .collect()
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for Grid {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Grid {
            style: self.style.clone(),
            breakpoints: self.breakpoints,
            cols: self.cols.clone(),
        }
    }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            style: Style::default().with_display(DisplayMode::Grid),
            breakpoints: None,
            cols: Vec::new(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.style = next.style;
        self.breakpoints = next.breakpoints;
        self.cols = next.cols;
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style.with_display(DisplayMode::Grid);
        self.ensure_responsive_tracks();
        self
    }

    pub fn columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.style.grid_template_columns = cols;
        self.breakpoints = None;
        self.cols.clear();
        self
    }

    /// 创建使用 24 单元栅格和 Ant Design 默认断点的响应式 Grid。
    pub fn responsive() -> Self {
        let mut grid = Self::new();
        grid.breakpoints = Some(Breakpoints::antd());
        grid.ensure_responsive_tracks();
        grid
    }

    /// 设置断点并启用响应式布局。
    pub fn breakpoints(mut self, breakpoints: Breakpoints) -> Self {
        self.breakpoints = Some(breakpoints);
        self.ensure_responsive_tracks();
        self
    }

    /// 设置与源顺序子节点对应的列配置并启用响应式布局。
    pub fn cols(mut self, cols: Vec<Col>) -> Self {
        self.cols = cols;
        self.breakpoints.get_or_insert_with(Breakpoints::antd);
        self.ensure_responsive_tracks();
        self
    }

    pub fn is_responsive(&self) -> bool {
        self.breakpoints.is_some()
    }

    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.style.grid_template_rows = rows;
        self
    }

    pub fn col_gap(mut self, gap: f32) -> Self {
        self.style.grid_column_gap = gap;
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.grid_row_gap = gap;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self.style.grid_column_gap = gap;
        self.style.grid_row_gap = gap;
        self
    }

    pub fn pad(mut self, padding: EdgeInsets) -> Self {
        self.style.padding = padding;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(ColorValue::Custom(color));
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.border_color = Some(ColorValue::Custom(color));
        self.style.border_width = EdgeInsets::uniform(width);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = radius;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.style.width = Some(width);
        self.style.height = Some(height);
        self
    }

    pub fn align(mut self, align: AlignItems) -> Self {
        self.style.align_items = align;
        self
    }

    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.style.justify_content = justify;
        self
    }

    pub fn apply_style(&mut self, style: &Style) {
        self.style = self.style.clone().apply(style.clone());
        self.style.display = DisplayMode::Grid;
        self.ensure_responsive_tracks();
    }

    pub fn two_columns() -> Self {
        Self::new().columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)])
    }

    pub fn three_columns() -> Self {
        Self::new().columns(vec![
            GridTrack::Fr(1.0),
            GridTrack::Fr(1.0),
            GridTrack::Fr(1.0),
        ])
    }

    fn effective_col_gap(&self) -> f32 {
        if self.style.grid_column_gap != 0.0 {
            self.style.grid_column_gap
        } else {
            self.style.gap
        }
    }

    fn effective_row_gap(&self) -> f32 {
        if self.style.grid_row_gap != 0.0 {
            self.style.grid_row_gap
        } else {
            self.style.gap
        }
    }

    fn ensure_responsive_tracks(&mut self) {
        if self.breakpoints.is_some() {
            self.style.grid_template_columns = vec![GridTrack::Fr(1.0); RESPONSIVE_GRID_UNITS];
        }
    }

    fn responsive_children<'a>(
        &self,
        available_width: f32,
        children: &'a [LayoutChild],
    ) -> Cow<'a, [LayoutChild]> {
        let Some(breakpoints) = self.breakpoints else {
            return Cow::Borrowed(children);
        };

        let mut configured = children.to_vec();
        let mut visual_order: Vec<usize> = (0..configured.len()).collect();
        visual_order.sort_by_key(|index| {
            (
                self.cols.get(*index).copied().unwrap_or_default().order,
                *index,
            )
        });

        let mut next_cell = 0usize;
        for index in visual_order {
            let col = self.cols.get(index).copied().unwrap_or_default();
            let span = col.span_at(available_width, breakpoints);
            let offset = col.effective_offset(span);
            let current_column = next_cell % RESPONSIVE_GRID_UNITS;
            if current_column + offset + span > RESPONSIVE_GRID_UNITS {
                next_cell = next_cell.div_ceil(RESPONSIVE_GRID_UNITS) * RESPONSIVE_GRID_UNITS;
            }
            let cell = next_cell + offset;
            configured[index].grid_cell = Some(cell);
            configured[index].grid_column_span = span as u32;
            next_cell = cell + span;
        }
        Cow::Owned(configured)
    }
}
