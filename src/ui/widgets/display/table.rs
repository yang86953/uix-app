use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::{
    ComponentId, EventResult, LayoutChild, SemanticEvent, SnapshotFields, SnapshotTableColumn,
    SnapshotTableColumnGroup, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

mod config;
pub(crate) mod geometry;
mod header;

use config::{flatten_column_groups, merge_table_columns};
use geometry::{ColumnZone, TableColumnGeometry};

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    None,
    Asc,
    Desc,
}

/// 表格列的固定位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixed {
    Left,
    Right,
}

/// 表格列定义。
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
    pub filterable: bool,
    pub filters: Vec<(String, bool)>, // (label, active)
    pub fixed: Option<Fixed>,
}

impl TableColumn {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width: finite_nonnegative(width),
            sortable: false,
            sort_direction: SortDirection::None,
            filterable: false,
            filters: Vec::new(),
            fixed: None,
        }
    }
    pub fn sortable(mut self, v: bool) -> Self {
        self.sortable = v;
        self
    }
    pub fn filterable(mut self, v: bool) -> Self {
        self.filterable = v;
        self
    }
    /// 将列固定在表格视口左侧或右侧。
    pub fn fixed(mut self, fixed: Fixed) -> Self {
        self.fixed = Some(fixed);
        self
    }

    /// 把 typed 行数据投影为本列的稳定文本与快照值。
    pub fn bind<R>(self, accessor: impl Fn(&R) -> String + 'static) -> TableDataColumn<R> {
        TableDataColumn {
            column: self,
            accessor: Box::new(accessor),
            renderer: None,
        }
    }
}

/// 一组共享上层表头的列；`column` 用于声明不参与分组的单列。
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnGroup {
    pub title: Option<String>,
    pub columns: Vec<TableColumn>,
}

impl TableColumnGroup {
    pub fn new(title: impl Into<String>, columns: Vec<TableColumn>) -> Self {
        Self {
            title: Some(title.into()),
            columns,
        }
    }

    pub fn column(column: TableColumn) -> Self {
        Self {
            title: None,
            columns: vec![column],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnGroupRange {
    title: Option<String>,
    start: usize,
    len: usize,
}

/// 表格行数据。
pub type TableRow = Vec<String>;

/// typed 行数据的一列文本投影。
type TypedCellViewRenderer<R> = Box<dyn Fn(&R) -> crate::ui::view::ViewNode>;

pub struct TableDataColumn<R> {
    column: TableColumn,
    accessor: Box<dyn Fn(&R) -> String>,
    renderer: Option<TypedCellViewRenderer<R>>,
}

impl<R> TableDataColumn<R> {
    /// Render this column as an arbitrary View while retaining the bound text
    /// as its deterministic snapshot and non-View fallback value.
    pub fn render<V>(mut self, renderer: impl Fn(&R) -> V + 'static) -> Self
    where
        V: crate::ui::view::View,
    {
        self.renderer = Some(Box::new(move |row| {
            crate::ui::view::View::build(renderer(row))
        }));
        self
    }
}

/// typed 表格行键校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDataError {
    DuplicateRowKey(String),
}

impl fmt::Display for TableDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRowKey(key) => write!(formatter, "duplicate table row key: {key}"),
        }
    }
}

impl Error for TableDataError {}

/// 泛型行数据表格；构建 View 时投影到 Table 的文本与稳定 row key。
pub struct DataTable<R> {
    table: Table,
    rows: Vec<R>,
    row_keys: Vec<String>,
    columns: Vec<TableDataColumn<R>>,
}

/// 扩展行视图工厂；按当前行数据构建普通 View 子树。
pub type ExpandRenderer = Box<dyn Fn(&TableRow) -> crate::ui::view::ViewNode>;

/// Type-erased typed-row cell factory kept in the render-handler sidecar.
pub(crate) type TableCellRenderer = Box<dyn Fn(usize, usize) -> crate::ui::view::ViewNode>;

/// 变更事件。
#[derive(Debug, Clone)]
pub struct TableChange {
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
    pub page: usize,
    pub page_size: usize,
}

/// 声明式表格构建器；展开 View factory 保存在组件外。
pub struct TableBuilder {
    table: Table,
    expand_renderer: ExpandRenderer,
}

component! {
    pub struct Table {
        columns: Vec<TableColumn>,
        column_groups: Vec<ColumnGroupRange>,
        pub(crate) rows: Vec<TableRow>,
        row_keys: Vec<String>,
        view_columns: Vec<usize>,
        materialized_cell_range: Cell<Option<(usize, usize)>>,
        pub(crate) row_h: f32,
        header_h: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        selected_row: Cell<Option<usize>>,
        hover_row: Cell<Option<usize>>,
        /// 多选：选中行索引集合。
        checked_rows: Vec<usize>,
        /// 扩展行：当前展开的行索引。
        expanded_row: Cell<Option<usize>>,
        expanded_child_row: Cell<Option<usize>>,
        expandable: bool,
        expand_height: f32,
        sortable: bool,
        selection: bool,
        bordered: bool,
        /// 空状态文案。
        empty_text: String,
        /// 当前分页。
        current_page: Cell<usize>,
        page_size: usize,
        pending_change: RefCell<Option<String>>,
        virtual_scroll: bool,
        pub(crate) body_scroll: VirtualListScroll,
        horizontal_scroll: Cell<f32>,
        horizontal_scroll_requires_paint: Cell<bool>,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        layout_requested: Cell<bool>,
        focused: bool,
    }

    tab_index => (&self) -> i32 { i32::from(!self.rows.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        let w: f32 = self
            .columns
            .iter()
            .map(|column| finite_nonnegative(column.width))
            .sum::<f32>()
            + self.selection_width();
        let extra = if self.expandable && self.expanded_row.get().is_some() { self.expand_height } else { 0.0 };
        let body_h = self.rows.len() as f32 * self.row_h + extra;
        let h = self.total_header_height() + body_h;
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(w),
            self.fixed_height.unwrap_or(h.max(60.0)),
        ))
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        if !self.view_columns.is_empty() {
            self.horizontal_scroll_requires_paint.set(false);
            self.scroll_delta_strip.set((0.0, 0.0));
            return None;
        }
        if self.horizontal_scroll_requires_paint.replace(false) {
            self.scroll_delta_strip.set((0.0, 0.0));
            return None;
        }
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    scroll_composite_viewport => (&self, frame: Rect) -> Option<Rect> {
        let header_height = self.total_header_height() + 1.0;
        let body_height = (frame.h - header_height).max(0.0);
        (body_height > 0.0).then(|| {
            Rect::new(frame.x, frame.y + header_height, frame.w, body_height)
        })
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, .. } => {
                let viewport_h = self.body_viewport_height();
                let old_y = self.body_scroll.scroll_offset();
                let max = (self.body_content_height() - viewport_h).max(0.0);
                let next_y = (old_y - delta.y * 40.0).clamp(0.0, max);
                self.body_scroll.set_scroll_offset(next_y);
                let dy = next_y - old_y;

                let old_x = self.horizontal_scroll.get();
                let next_x = (old_x - delta.x * 40.0)
                    .clamp(0.0, self.horizontal_max_scroll());
                self.horizontal_scroll.set(next_x);
                let dx = next_x - old_x;
                if dx.abs() > 0.01 {
                    self.horizontal_scroll_requires_paint.set(true);
                }
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                }
                if dx.abs() > 0.01 || dy.abs() > 0.01 {
                    if !self.view_columns.is_empty() {
                        self.layout_requested.set(true);
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown {
                pos,
                button: crate::ui::MouseButton::Left,
                ..
            } => {
                if self.selection && pos.x < self.selection_width() {
                    if pos.y < self.total_header_height() {
                        if self.checked_rows.len() == self.rows.len() {
                            self.checked_rows.clear();
                        } else {
                            self.checked_rows = (0..self.rows.len()).collect();
                        }
                        return EventResult::Handled;
                    }
                    if let Some(row) = self.row_index_at_y(pos.y) {
                        if let Some(index) = self.checked_rows.iter().position(|&item| item == row) {
                            self.checked_rows.remove(index);
                        } else {
                            self.checked_rows.push(row);
                        }
                        return EventResult::Handled;
                    }
                }
                if self.leaf_header_contains(pos.y, self.column_at_x(pos.x)) {
                    if let Some(ci) = self.column_at_x(pos.x) {
                        let col = &self.columns[ci];
                        if self.sortable || col.sortable {
                            let new_dir = match col.sort_direction {
                                SortDirection::None => SortDirection::Asc,
                                SortDirection::Asc => SortDirection::Desc,
                                SortDirection::Desc => SortDirection::None,
                            };
                            for c in &mut self.columns { c.sort_direction = SortDirection::None; }
                            self.columns[ci].sort_direction = new_dir;
                            let change = TableChange {
                                sort_column: Some(ci),
                                sort_direction: new_dir,
                                page: self.current_page.get(),
                                page_size: self.page_size,
                            };
                            self.pending_change.replace(Some(change.payload()));
                            return EventResult::Handled;
                        }
                    }
                }
                if pos.y >= self.total_header_height() {
                    if let Some(row) = self.row_index_at_y(pos.y) {
                        if self.expandable && self.expand_toggle_hit(pos.x) {
                            let next = (self.expanded_row.get() != Some(row)).then_some(row);
                            self.expanded_row.set(next);
                        }
                        self.selected_row.set(Some(row));
                        return EventResult::Handled;
                    }
                }
                self.selected_row.set(None);
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if pos.y >= self.total_header_height() {
                    let row = self.row_index_at_y(pos.y);
                    self.hover_row.set(row.filter(|&r| r < self.rows.len()));
                } else {
                    self.hover_row.set(None);
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(frame));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let hover_bg = ctx.tokens().color_fill_quaternary();
        let sel_bg = ctx.tokens().color_primary_bg();
        let radius = ctx
            .tokens()
            .border_radius_sm()
            .min(frame.w.min(frame.h) * 0.5);
        let r = Some(Radius::uniform(radius));
        let mut y = frame.y;
        let sel = self.selected_row.get();
        let hover = self.hover_row.get();
        let expanded = self.expanded_row.get();
        let column_geometry = self.column_geometry(frame.x, frame.w);
        ctx.push_clip(frame);

        // 空状态
        if self.rows.is_empty() {
            let loc = crate::ui::locale::use_locale();
            let empty = if self.empty_text.is_empty() { loc.empty_data } else { &self.empty_text };
            let horizontal_inset = 16.0_f32.min(frame.w * 0.25);
            Self::paint_single_line(
                ctx,
                empty,
                Rect::new(
                    frame.x + horizontal_inset,
                    frame.y,
                    (frame.w - horizontal_inset * 2.0).max(0.0),
                    frame.h,
                ),
                text_sec,
                14.0,
            );
            if self.bordered {
                ctx.stroke_rect(frame, border, 1.0, r);
            }
            ctx.pop_clip();
            return;
        }

        header::paint(self, Rect::new(frame.x, y, frame.w, frame.h), ctx, &column_geometry);
        y += self.total_header_height();

        // 分隔线
        ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
        y += 1.0;

        // 数据行（虚拟滚动：仅绘制 viewport ± overscan）
        let body_top = y;
        let body_viewport_h = self.body_viewport_height();
        let (start, end) = self.visible_row_range(body_viewport_h);
        let body_clip = Rect::new(frame.x, body_top, frame.w, body_viewport_h);
        ctx.push_clip(body_clip);

        for actual_ri in start..end {
            let Some(row) = self.rows.get(actual_ri) else {
                continue;
            };
            let is_selected = sel == Some(actual_ri);
            let is_hovered = hover == Some(actual_ri);
            let is_checked = self.checked_rows.contains(&actual_ri);
            let is_expanded = expanded == Some(actual_ri);

            let expanded_offset = if expanded.is_some_and(|expanded_row| actual_ri > expanded_row)
            {
                self.expand_height
            } else {
                0.0
            };
            let row_y = body_top + actual_ri as f32 * self.row_h + expanded_offset
                - self.body_scroll.scroll_offset();
            if row_y + self.row_h < body_top || row_y > body_top + body_viewport_h {
                continue;
            }

            let row_bg = if is_selected { sel_bg } else if is_hovered { hover_bg } else if actual_ri.is_multiple_of(2) { bg } else { ctx.tokens().color_bg_container() };
            let row_rect = Rect::new(frame.x, row_y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            if self.selection {
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    if is_checked { "check-square" } else { "square" },
                    Rect::new(frame.x + 6.0, row_rect.y, 18.0, row_rect.h),
                    primary,
                    14.0,
                );
                if self.bordered {
                    ctx.fill_rect(
                        Rect::new(
                            frame.x + self.selection_width() - 1.0,
                            row_rect.y,
                            1.0,
                            row_rect.h,
                        ),
                        border,
                        None,
                    );
                }
            }

            for zone in [ColumnZone::Middle, ColumnZone::Left, ColumnZone::Right] {
                let Some(clip) = column_geometry.clip_for(zone, row_rect.y, row_rect.h) else {
                    continue;
                };
                ctx.canvas_2d().push_clip(clip);
                for laid_out in column_geometry
                    .columns
                    .iter()
                    .filter(|column| column.zone == zone)
                {
                    if !self.view_columns.contains(&laid_out.index) {
                        let cell = row
                            .get(laid_out.index)
                            .map(String::as_str)
                            .unwrap_or("");
                        let tc = if is_selected { primary } else { text_color };
                        let cell_frame = Rect::new(
                            laid_out.x,
                            row_rect.y,
                            laid_out.width,
                            row_rect.h,
                        );
                        if let Some(cell_frame) = cell_frame.intersect(&clip) {
                            let horizontal_inset = 8.0_f32.min(cell_frame.w * 0.25);
                            Self::paint_single_line(
                                ctx,
                                cell,
                                Rect::new(
                                    cell_frame.x + horizontal_inset,
                                    cell_frame.y,
                                    (cell_frame.w - horizontal_inset * 2.0).max(0.0),
                                    cell_frame.h,
                                ),
                                tc,
                                12.0,
                            );
                        }
                    }
                    if self.bordered {
                        ctx.fill_rect(
                            Rect::new(
                                laid_out.x + laid_out.width - 1.0,
                                row_rect.y,
                                1.0,
                                row_rect.h,
                            ),
                            border,
                            None,
                        );
                    }
                }
                ctx.canvas_2d().pop_clip();
            }

            // 扩展行箭头
            if self.expandable {
                let icon_size = 18.0_f32.min(row_rect.h).min(frame.w);
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    if is_expanded {
                        "chevron-up"
                    } else {
                        "chevron-down"
                    },
                    Rect::new(
                        frame.x + (frame.w - icon_size - 6.0).max(0.0),
                        row_rect.y + (row_rect.h - icon_size) * 0.5,
                        icon_size,
                        icon_size,
                    ),
                    text_sec,
                    10.0,
                );
            }

            if actual_ri + 1 < end || is_expanded {
                ctx.fill_rect(Rect::new(frame.x, row_y + self.row_h, frame.w, 1.0), border, None);
            }

            // 扩展行内容
            if is_expanded {
                let expand_rect = Rect::new(
                    frame.x,
                    row_y + self.row_h,
                    frame.w,
                    self.expand_height,
                );
                ctx.fill_rect(expand_rect, ctx.tokens().color_bg_container(), None);
            }
        }

        ctx.pop_clip();
        if self.bordered {
            ctx.stroke_rect(frame, border, 1.0, r);
        }
        if self.focused {
            let inset = 1.0_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                primary,
                2.0,
                Some(Radius::uniform(
                    radius.min((frame.w - inset * 2.0).min(frame.h - inset * 2.0) * 0.5),
                )),
            );
        }
        ctx.pop_clip();
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        let header_height = self.total_header_height();
        Some(Rect::new(
            frame.x,
            frame.y + header_height + 1.0,
            frame.w,
            (frame.h - header_height - 1.0).max(0.0),
        ))
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        self.last_frame.set(Some(frame));
        if !self.view_columns.is_empty() {
            let (start, end) = self
                .materialized_cell_range
                .get()
                .unwrap_or_else(|| self.visible_row_range(self.body_viewport_height()));
            let column_geometry = self.column_geometry(frame.x, frame.w);
            let body_top = frame.y + self.total_header_height() + 1.0;
            let mut positions = Vec::with_capacity(children.len());
            for (local_index, child) in children.iter().enumerate() {
                let column_count = self.view_columns.len();
                let row = start + local_index / column_count;
                if row >= end {
                    break;
                }
                let column_index = self.view_columns[local_index % column_count];
                let Some(column) = column_geometry
                    .columns
                    .iter()
                    .find(|column| column.index == column_index)
                else {
                    continue;
                };
                let expanded_offset = if self.expanded_row.get().is_some_and(|expanded| row > expanded)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + row as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                let cell = Rect::new(column.x, row_y, column.width, self.row_h);
                positions.push((
                    child.id,
                    column_geometry
                        .clip_for(column.zone, row_y, self.row_h)
                        .and_then(|zone_clip| cell.intersect(&zone_clip))
                        .unwrap_or_else(|| Rect::new(cell.x, cell.y, 0.0, 0.0)),
                ));
            }
            return positions;
        }
        let Some(expanded_row) = self.expanded_row.get() else {
            return Vec::new();
        };
        let Some(child) = children.first() else {
            return Vec::new();
        };
        let y = frame.y
            + self.total_header_height()
            + 1.0
            + (expanded_row + 1) as f32 * self.row_h;
        vec![(
            child.id,
            Rect::new(frame.x, y, frame.w, self.expand_height),
        )]
    }
}

impl TableChange {
    fn payload(&self) -> String {
        let sort_column = self
            .sort_column
            .map(|idx| idx.to_string())
            .unwrap_or_default();
        let sort_direction = match self.sort_direction {
            SortDirection::None => "none",
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        format!(
            "sort_column={};sort_direction={};page={};page_size={}",
            sort_column, sort_direction, self.page, self.page_size
        )
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            column_groups: Vec::new(),
            rows: Vec::new(),
            row_keys: Vec::new(),
            view_columns: Vec::new(),
            materialized_cell_range: Cell::new(None),
            row_h: 28.0,
            header_h: 32.0,
            fixed_width: None,
            fixed_height: None,
            selected_row: Cell::new(None),
            hover_row: Cell::new(None),
            checked_rows: Vec::new(),
            expanded_row: Cell::new(None),
            expanded_child_row: Cell::new(None),
            expandable: false,
            expand_height: 60.0,
            sortable: false,
            selection: false,
            bordered: false,
            empty_text: String::new(),
            current_page: Cell::new(0),
            page_size: 20,
            pending_change: RefCell::new(None),
            virtual_scroll: false,
            body_scroll: VirtualListScroll::new(),
            horizontal_scroll: Cell::new(0.0),
            horizontal_scroll_requires_paint: Cell::new(false),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            layout_requested: Cell::new(false),
            focused: false,
        }
    }

    fn normalized_columns(mut columns: Vec<TableColumn>) -> Vec<TableColumn> {
        for column in &mut columns {
            column.width = finite_nonnegative(column.width);
        }
        columns
    }

    fn normalized_row_height(height: f32) -> f32 {
        if height.is_finite() {
            height.max(1.0)
        } else {
            28.0
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            finite_nonnegative(frame.w),
            finite_nonnegative(frame.h),
        )
    }

    pub(super) fn paint_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::font::text_backend::estimate_text_metrics(value, f32::INFINITY, font_size)
                .max_line_width,
        )
    }

    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self {
        self.columns = Self::normalized_columns(cols);
        self.column_groups.clear();
        self
    }
    /// 使用分组定义替换当前列；`TableColumnGroup::column` 声明跨两层表头的单列。
    pub fn column_groups(mut self, groups: Vec<TableColumnGroup>) -> Self {
        (self.columns, self.column_groups) = flatten_column_groups(groups);
        self.columns = Self::normalized_columns(self.columns);
        self
    }
    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.row_keys = implicit_row_keys(rows.len());
        self.rows = rows;
        self
    }

    /// 以应用提供的唯一 key 创建泛型行数据表格。
    pub fn data<R>(
        rows: Vec<R>,
        row_key: impl Fn(&R) -> String,
    ) -> Result<DataTable<R>, TableDataError> {
        let mut seen = HashSet::with_capacity(rows.len());
        let mut row_keys = Vec::with_capacity(rows.len());
        for row in &rows {
            let key = row_key(row);
            if !seen.insert(key.clone()) {
                return Err(TableDataError::DuplicateRowKey(key));
            }
            row_keys.push(key);
        }
        Ok(DataTable {
            table: Self::new(),
            rows,
            row_keys,
            columns: Vec::new(),
        })
    }
    /// 是否让所有列头参与排序交互；列级 `TableColumn::sortable` 仍可单独启用。
    pub fn sortable(mut self, enabled: bool) -> Self {
        self.sortable = enabled;
        self
    }
    /// 是否显示行复选框并启用多选。
    pub fn selection(mut self, enabled: bool) -> Self {
        self.selection = enabled;
        if !enabled {
            self.checked_rows.clear();
        }
        self
    }
    /// 是否绘制外框与单元格纵向边界。
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.bordered = enabled;
        self
    }
    /// 设置表格视口尺寸；数据超出高度时仅表体滚动，表头保持可见。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.fixed_width = Some(finite_nonnegative(width));
        self.fixed_height = Some(finite_nonnegative(height));
        self
    }
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_h = Self::normalized_row_height(h);
        self
    }
    /// 是否仅遍历表体视口及 overscan 范围内的行。
    pub fn virtual_scroll(mut self, enabled: bool) -> Self {
        self.virtual_scroll = enabled;
        self
    }
    /// 设置虚拟滚动使用的固定行高；不会隐式开启虚拟滚动。
    pub fn virtual_row_height(mut self, height: f32) -> Self {
        self.row_h = Self::normalized_row_height(height);
        self
    }
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.get()
    }
    pub fn set_selected_row(&self, row: Option<usize>) {
        self.selected_row.set(row);
    }
    pub fn selected_row_key(&self) -> Option<&str> {
        self.selected_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .map(String::as_str)
    }
    pub fn expanded_row(&self) -> Option<usize> {
        self.expanded_row.get()
    }
    pub fn checked_rows(&self) -> &[usize] {
        &self.checked_rows
    }
    pub fn checked_row_keys(&self) -> Vec<&str> {
        self.checked_rows
            .iter()
            .filter_map(|row| self.row_keys.get(*row).map(String::as_str))
            .collect()
    }
    pub fn empty_text(mut self, t: impl Into<String>) -> Self {
        self.empty_text = t.into();
        self
    }
    /// 为展开行声明普通 View 子树；闭包接收当前 `TableRow`。
    pub fn expandable<V>(
        mut self,
        height: f32,
        renderer: impl Fn(&TableRow) -> V + 'static,
    ) -> TableBuilder
    where
        V: crate::ui::view::View,
    {
        self.expandable = true;
        self.expand_height = finite_nonnegative(height);
        TableBuilder {
            table: self,
            expand_renderer: Box::new(move |row| crate::ui::view::View::build(renderer(row))),
        }
    }
    pub fn page_size(mut self, n: usize) -> Self {
        self.page_size = n;
        self
    }

    fn selection_width(&self) -> f32 {
        if self.selection {
            32.0
        } else {
            0.0
        }
    }

    fn total_header_height(&self) -> f32 {
        if self.column_groups.is_empty() {
            self.header_h
        } else {
            self.header_h * 2.0
        }
    }

    fn leaf_header_contains(&self, y: f32, column: Option<usize>) -> bool {
        if y < 0.0 || y >= self.total_header_height() {
            return false;
        }
        if self.column_groups.is_empty() {
            return true;
        }
        let Some(column) = column else {
            return false;
        };
        let grouped = self
            .column_groups
            .iter()
            .find(|group| column >= group.start && column < group.start + group.len)
            .is_some_and(|group| group.title.is_some());
        !grouped || y >= self.header_h
    }

    fn column_geometry(&self, origin_x: f32, viewport_width: f32) -> TableColumnGeometry {
        TableColumnGeometry::new(
            &self.columns,
            origin_x,
            viewport_width,
            self.selection_width(),
            self.horizontal_scroll.get(),
        )
    }

    fn column_at_x(&self, x: f32) -> Option<usize> {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        self.column_geometry(0.0, width).column_at(x)
    }

    fn horizontal_max_scroll(&self) -> f32 {
        self.last_frame
            .get()
            .map(|frame| self.column_geometry(0.0, frame.w).max_scroll_x)
            .unwrap_or(0.0)
    }

    #[cfg(test)]
    pub(crate) fn horizontal_scroll_offset(&self) -> f32 {
        self.horizontal_scroll.get()
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| (finite_nonnegative(f.h) - self.total_header_height() - 1.0).max(0.0))
            .unwrap_or(300.0)
    }

    pub(crate) fn expanded_child_row(&self) -> Option<usize> {
        self.expanded_child_row.get()
    }

    pub(crate) fn mark_expanded_child_materialized(&self) {
        self.expanded_child_row.set(self.expanded_row.get());
    }

    fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let header_height = self.total_header_height();
        let viewport_height = self.body_viewport_height();
        if pos_y < header_height || pos_y >= header_height + 1.0 + viewport_height {
            return None;
        }
        let mut local_y = pos_y - header_height - 1.0 + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        if self.expandable {
            if let Some(expanded) = self.expanded_row.get() {
                let expand_start = (expanded + 1) as f32 * self.row_h;
                if local_y >= expand_start && local_y < expand_start + self.expand_height {
                    return None;
                }
                if local_y >= expand_start + self.expand_height {
                    local_y -= self.expand_height;
                }
            }
        }
        let row = (local_y / self.row_h) as usize;
        if row < self.rows.len() {
            Some(row)
        } else {
            None
        }
    }

    fn expand_toggle_hit(&self, pos_x: f32) -> bool {
        let width = self
            .last_frame
            .get()
            .map(|frame| frame.w)
            .unwrap_or_else(|| self.columns.iter().map(|column| column.width).sum());
        pos_x >= (width - 32.0).max(0.0)
    }

    fn body_content_height(&self) -> f32 {
        self.rows.len() as f32 * self.row_h
            + if self.expandable && self.expanded_row.get().is_some() {
                self.expand_height
            } else {
                0.0
            }
    }

    pub(crate) fn visible_row_range(&self, viewport_height: f32) -> (usize, usize) {
        if !self.virtual_scroll {
            return (0, self.rows.len());
        }
        let mut logical_offset = self.body_scroll.scroll_offset();
        if self.expandable {
            if let Some(expanded) = self.expanded_row.get() {
                let expand_start = (expanded + 1) as f32 * self.row_h;
                if logical_offset > expand_start {
                    logical_offset = (logical_offset - self.expand_height).max(expand_start);
                }
            }
        }
        crate::ui::foundation::virtual_scroll::virtual_list_index_range(
            self.rows.len(),
            self.row_h,
            logical_offset,
            viewport_height,
            2,
        )
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Table {
            columns: self
                .columns
                .iter()
                .map(SnapshotTableColumn::from_table_column)
                .collect(),
            column_groups: self
                .column_groups
                .iter()
                .map(|group| SnapshotTableColumnGroup {
                    title: group.title.clone(),
                    start: group.start,
                    len: group.len,
                })
                .collect(),
            rows: self.rows.clone(),
            row_keys: self.row_keys.clone(),
            view_columns: self.view_columns.clone(),
            row_h: self.row_h,
            header_h: self.header_h,
            expandable: self.expandable,
            expand_height: self.expand_height,
            sortable: self.sortable,
            selection: self.selection,
            bordered: self.bordered,
            selected_row: self.selected_row.get(),
            checked_rows: self.checked_rows.clone(),
            empty_text: self.empty_text.clone(),
            page_size: self.page_size,
            virtual_scroll: self.virtual_scroll,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let selected_key = self
            .selected_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .cloned();
        let checked_keys = self
            .checked_rows
            .iter()
            .filter_map(|row| self.row_keys.get(*row).cloned())
            .collect::<HashSet<_>>();
        let expanded_key = self
            .expanded_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .cloned();
        self.columns = merge_table_columns(self.columns.as_slice(), next.columns, next.sortable);
        for column in &mut self.columns {
            column.width = finite_nonnegative(column.width);
        }
        self.column_groups = next.column_groups;
        self.rows = next.rows;
        self.row_keys = next.row_keys;
        self.view_columns = next.view_columns;
        self.materialized_cell_range.set(None);
        self.row_h = Self::normalized_row_height(next.row_h);
        self.header_h = next.header_h;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.expandable = next.expandable;
        self.expand_height = finite_nonnegative(next.expand_height);
        self.sortable = next.sortable;
        self.selection = next.selection;
        self.bordered = next.bordered;
        self.empty_text = next.empty_text;
        self.page_size = next.page_size;
        self.virtual_scroll = next.virtual_scroll;
        if self.selection {
            self.checked_rows = self
                .row_keys
                .iter()
                .enumerate()
                .filter_map(|(index, key)| checked_keys.contains(key).then_some(index))
                .collect();
        } else {
            self.checked_rows.clear();
        }
        self.selected_row
            .set(selected_key.and_then(|key| self.row_keys.iter().position(|item| item == &key)));
        self.expanded_row
            .set(expanded_key.and_then(|key| self.row_keys.iter().position(|item| item == &key)));
        self.hover_row.set(None);
        self.expanded_child_row.set(None);
        let max = (self.body_content_height() - self.body_viewport_height()).max(0.0);
        self.body_scroll
            .set_scroll_offset(self.body_scroll.scroll_offset().min(max));
        self.horizontal_scroll.set(
            self.horizontal_scroll
                .get()
                .min(self.horizontal_max_scroll()),
        );
    }

    pub(crate) fn cell_view_range_for_frame(&self, frame: Rect) -> (usize, usize) {
        let viewport_height =
            (finite_nonnegative(frame.h) - self.total_header_height() - 1.0).max(0.0);
        self.visible_row_range(viewport_height)
    }

    pub(crate) fn view_columns(&self) -> &[usize] {
        &self.view_columns
    }

    pub(crate) fn row_keys(&self) -> &[String] {
        &self.row_keys
    }

    pub(crate) fn needs_cell_refresh(
        &self,
        range: (usize, usize),
        mounted_children: usize,
    ) -> bool {
        self.materialized_cell_range.get() != Some(range)
            || mounted_children
                != range
                    .1
                    .saturating_sub(range.0)
                    .saturating_mul(self.view_columns.len())
    }

    pub(crate) fn mark_cells_materialized(&self, range: (usize, usize)) {
        self.materialized_cell_range.set(Some(range));
    }
}

fn implicit_row_keys(row_count: usize) -> Vec<String> {
    (0..row_count).map(|index| index.to_string()).collect()
}

impl<R> DataTable<R> {
    pub fn columns(mut self, columns: Vec<TableDataColumn<R>>) -> Self {
        self.columns = columns;
        self
    }

    pub fn sortable(mut self, enabled: bool) -> Self {
        self.table.sortable = enabled;
        self
    }

    pub fn selection(mut self, enabled: bool) -> Self {
        self.table.selection = enabled;
        if !enabled {
            self.table.checked_rows.clear();
        }
        self
    }

    pub fn bordered(mut self, enabled: bool) -> Self {
        self.table.bordered = enabled;
        self
    }

    /// 设置表格视口尺寸；数据超出高度时仅表体滚动，表头保持可见。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.table.fixed_width = Some(finite_nonnegative(width));
        self.table.fixed_height = Some(finite_nonnegative(height));
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self
    }

    pub fn virtual_scroll(mut self, enabled: bool) -> Self {
        self.table.virtual_scroll = enabled;
        self
    }

    pub fn virtual_row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self
    }

    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.table.empty_text = text.into();
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }

    fn into_parts(self) -> (Table, Option<RenderHandlerRegistration>)
    where
        R: 'static,
    {
        let Self {
            mut table,
            rows,
            row_keys,
            columns,
        } = self;
        table.columns =
            Table::normalized_columns(columns.iter().map(|column| column.column.clone()).collect());
        table.column_groups.clear();
        table.rows = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|column| (column.accessor)(row))
                    .collect()
            })
            .collect();
        table.row_keys = row_keys;
        table.view_columns = columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| column.renderer.is_some().then_some(index))
            .collect();

        if table.view_columns.is_empty() {
            return (table, None);
        }

        let rows = Rc::new(rows);
        let renderers = columns
            .into_iter()
            .filter_map(|column| column.renderer)
            .collect::<Vec<_>>();
        let renderer: TableCellRenderer = Box::new(move |row, renderer_index| {
            let row = &rows[row];
            (renderers[renderer_index])(row)
        });
        (table, Some(RenderHandlerRegistration::TableCells(renderer)))
    }
}

impl<R: 'static> crate::ui::view::View for DataTable<R> {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handler) = self.into_parts();
        let mut node = crate::ui::view::View::build(table);
        if let Some(handler) = handler {
            node.render_handlers.push(handler);
        }
        node
    }
}

impl<R: 'static> crate::ui::IntoWidgetNode for DataTable<R> {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        let (table, handler) = self.into_parts();
        let mut node = crate::ui::IntoWidgetNode::into_node(table);
        if let Some(handler) = handler {
            node.render_handlers.push(handler);
        }
        node
    }
}

impl TableBuilder {
    fn into_parts(self) -> (Table, RenderHandlerRegistration) {
        (
            self.table,
            RenderHandlerRegistration::TableExpand(self.expand_renderer),
        )
    }

    pub fn columns(mut self, columns: Vec<TableColumn>) -> Self {
        self.table.columns = Table::normalized_columns(columns);
        self.table.column_groups.clear();
        self
    }

    /// 使用分组定义替换当前列；`TableColumnGroup::column` 声明跨两层表头的单列。
    pub fn column_groups(mut self, groups: Vec<TableColumnGroup>) -> Self {
        (self.table.columns, self.table.column_groups) = flatten_column_groups(groups);
        self.table.columns = Table::normalized_columns(self.table.columns);
        self
    }

    pub fn rows(mut self, rows: Vec<TableRow>) -> Self {
        self.table.row_keys = implicit_row_keys(rows.len());
        self.table.rows = rows;
        self
    }

    /// 是否让所有列头参与排序交互。
    pub fn sortable(mut self, enabled: bool) -> Self {
        self.table.sortable = enabled;
        self
    }

    /// 是否显示行复选框并启用多选。
    pub fn selection(mut self, enabled: bool) -> Self {
        self.table.selection = enabled;
        if !enabled {
            self.table.checked_rows.clear();
        }
        self
    }

    /// 是否绘制外框与单元格纵向边界。
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.table.bordered = enabled;
        self
    }

    /// 设置表格视口尺寸；数据超出高度时仅表体滚动，表头保持可见。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.table.fixed_width = Some(finite_nonnegative(width));
        self.table.fixed_height = Some(finite_nonnegative(height));
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self
    }

    /// 是否仅遍历表体视口及 overscan 范围内的行。
    pub fn virtual_scroll(mut self, enabled: bool) -> Self {
        self.table.virtual_scroll = enabled;
        self
    }

    /// 设置虚拟滚动使用的固定行高；不会隐式开启虚拟滚动。
    pub fn virtual_row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self
    }

    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.table.empty_text = text.into();
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }
}

impl crate::ui::IntoWidgetNode for TableBuilder {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        let (table, handler) = self.into_parts();
        crate::ui::core::widget::WidgetNode::leaf(Box::new(table))
            .with_render_handlers(vec![handler])
    }
}

impl crate::ui::view::View for TableBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handler) = self.into_parts();
        if table.rows.is_empty() {
            if let Some(empty) = crate::ui::config::render_empty_for::<Table>() {
                return empty;
            }
        }
        let mut node = crate::ui::view::ViewNode::leaf(table);
        node.render_handlers.push(handler);
        node
    }
}

impl crate::ui::view::View for Table {
    fn build(self) -> crate::ui::view::ViewNode {
        if self.rows.is_empty() {
            if let Some(empty) = crate::ui::config::render_empty_for::<Self>() {
                return empty;
            }
        }
        crate::ui::view::ViewNode::leaf(self)
    }
}

impl From<TableBuilder> for crate::ui::view::ViewNode {
    fn from(builder: TableBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}
