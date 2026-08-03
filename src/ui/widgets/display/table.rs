use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, SemanticEvent, SnapshotFields,
    SnapshotTableColumn, SnapshotTableColumnGroup, SystemEvent, WidgetTree,
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

const COLUMN_RESIZE_HANDLE_HALF_WIDTH: f32 = 4.0;
const MIN_RESIZABLE_COLUMN_WIDTH: f32 = 32.0;
const TABLE_PAGINATION_HEIGHT: f32 = 40.0;
const TABLE_PAGINATION_ITEM_SIZE: f32 = 28.0;
const TABLE_PAGINATION_GAP: f32 = 4.0;
const TABLE_PAGINATION_LABEL_WIDTH: f32 = 80.0;
const TABLE_PAGINATION_INSET: f32 = 8.0;

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
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_direction: SortDirection,
    pub filterable: bool,
    pub filters: Vec<(String, bool)>, // (label, active)
    pub fixed: Option<Fixed>,
    pub resizable: bool,
    /// 合并单元格时返回当前单元格占用的行数；返回 0 表示由上方单元格覆盖。
    pub row_span: Option<fn(&TableRow, usize) -> usize>,
    /// 合并单元格时返回当前单元格占用的列数。
    pub col_span: Option<fn(&TableRow, usize) -> usize>,
}

impl PartialEq for TableColumn {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.width == other.width
            && self.sortable == other.sortable
            && self.sort_direction == other.sort_direction
            && self.filterable == other.filterable
            && self.filters == other.filters
            && self.fixed == other.fixed
            && self.resizable == other.resizable
            // Function pointers do not have stable identity across codegen units;
            // the callback's presence is the meaningful part of the declaration.
            && self.row_span.is_some() == other.row_span.is_some()
            && self.col_span.is_some() == other.col_span.is_some()
    }
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
            resizable: false,
            row_span: None,
            col_span: None,
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
    /// 允许从表头右边缘拖拽调整列宽。
    pub fn resizable(mut self, enabled: bool) -> Self {
        self.resizable = enabled;
        self
    }
    /// 将列固定在表格视口左侧或右侧。
    pub fn fixed(mut self, fixed: Fixed) -> Self {
        self.fixed = Some(fixed);
        self
    }

    /// 设置单元格的行合并函数。
    pub fn row_span(mut self, span: fn(&TableRow, usize) -> usize) -> Self {
        self.row_span = Some(span);
        self
    }

    /// 设置单元格的列合并函数。
    pub fn col_span(mut self, span: fn(&TableRow, usize) -> usize) -> Self {
        self.col_span = Some(span);
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
    empty_renderer: Option<TableEmptyRenderer>,
}

/// 扩展行视图工厂；按当前行数据构建普通 View 子树。
pub type ExpandRenderer = Box<dyn Fn(&TableRow) -> crate::ui::view::ViewNode>;

/// 空态 View 工厂；仅在 build 边界消费，接收当前列数，不进入 Table 组件。
pub type TableEmptyRenderer = Box<dyn Fn(usize) -> crate::ui::view::ViewNode>;

/// Type-erased typed-row cell factory kept in the render-handler sidecar.
pub(crate) type TableCellRenderer = Box<dyn Fn(usize, usize) -> crate::ui::view::ViewNode>;

/// 空行且非加载时解析空态 View：表级 `.empty` 优先于 ConfigProvider。
fn resolve_table_empty_view(
    table: &Table,
    empty_renderer: Option<&TableEmptyRenderer>,
) -> Option<crate::ui::view::ViewNode> {
    if !table.rows.is_empty() || table.loading {
        return None;
    }
    if let Some(renderer) = empty_renderer {
        return Some(renderer(table.columns.len()));
    }
    crate::ui::component::config::render_empty_for::<Table>()
}

/// 变更事件。
#[derive(Debug, Clone)]
pub struct TableChange {
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
    pub page: usize,
    pub page_size: usize,
}

/// 远程分页配置。回调由表格在页码变化时调用，数据本身仍由应用层维护。
pub struct TablePagination<F = fn(usize)> {
    pub current: usize,
    pub total: usize,
    pub page_size: usize,
    pub on_change: F,
}

type TablePaginationCallback = Rc<dyn Fn(usize)>;
type TableRowClickCallback = Rc<dyn Fn(&TableRow, usize)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TablePointerAction {
    ToggleAll,
    ToggleRow(usize),
    SortColumn(usize),
    ToggleExpand(usize),
    SelectRow(usize),
    ChangePage(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TableResizeDrag {
    column: usize,
    start_x: f32,
    start_width: f32,
}

/// 声明式表格构建器；展开 / 空态 View factory 保存在组件外。
pub struct TableBuilder {
    table: Table,
    expand_renderer: Option<ExpandRenderer>,
    empty_renderer: Option<TableEmptyRenderer>,
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
        loading: bool,
        loading_phase: f32,
        loading_dirty: bool,
        /// 空状态文案。
        empty_text: String,
        /// 当前分页。
        current_page: Cell<usize>,
        page_size: usize,
        pagination: Option<(usize, usize, usize, TablePaginationCallback)>,
        row_click: Option<TableRowClickCallback>,
        pending_change: RefCell<Option<String>>,
        virtual_scroll: bool,
        pub(crate) body_scroll: VirtualListScroll,
        horizontal_scroll: Cell<f32>,
        horizontal_scroll_requires_paint: Cell<bool>,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        layout_requested: Cell<bool>,
        focused: bool,
        pressed_action: Cell<Option<TablePointerAction>>,
        resize_drag: Cell<Option<TableResizeDrag>>,
        hover_resize_column: Cell<Option<usize>>,
    }

    tab_index => (&self) -> i32 {
        i32::from(!self.loading && (!self.rows.is_empty() || self.pagination.is_some()))
    }

    measure => (&self, constraints: Constraints) -> Size {
        let w: f32 = self
            .columns
            .iter()
            .map(|column| finite_nonnegative(column.width))
            .sum::<f32>()
            + self.selection_width();
        let extra = if self.expandable && self.expanded_row.get().is_some() { self.expand_height } else { 0.0 };
        let body_h = self.rows.len() as f32 * self.row_h + extra;
        let body_separator = if !self.rows.is_empty() || self.loading {
            1.0
        } else {
            0.0
        };
        let h = self.total_header_height()
            + body_separator
            + body_h
            + self.pagination_height();
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(w),
            self.fixed_height.unwrap_or(h.max(60.0)),
        ))
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
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
        let body_height = (frame.h - header_height - self.pagination_height()).max(0.0);
        (body_height > 0.0).then(|| {
            Rect::new(frame.x, frame.y + header_height, frame.w, body_height)
        })
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.loading {
            let changed = self.pressed_action.replace(None).is_some()
                | self.hover_row.replace(None).is_some()
                | self.hover_resize_column.replace(None).is_some()
                | self.cancel_column_resize();
            if matches!(event, SystemEvent::FocusOut) {
                self.focused = false;
            }
            let blocks_interaction = match event {
                SystemEvent::PointerDown { pos, .. }
                | SystemEvent::PointerUp { pos, .. }
                | SystemEvent::PointerMove { pos, .. }
                | SystemEvent::Wheel { pos, .. } => self.local_frame().contains(*pos),
                SystemEvent::KeyDown { .. } => self.focused,
                _ => false,
            };
            return if changed || blocks_interaction || matches!(event, SystemEvent::FocusOut) {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            };
        }
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed_action.set(None);
                self.cancel_column_resize();
                self.hover_resize_column.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.focused && self.pagination.is_some() => {
                let current = self.pagination_current();
                let next = match key {
                    KeyCode::Left | KeyCode::PageUp => current.saturating_sub(1).max(1),
                    KeyCode::Right | KeyCode::PageDown => {
                        current.saturating_add(1).min(self.pagination_total_pages())
                    }
                    KeyCode::Home => 1,
                    KeyCode::End => self.pagination_total_pages(),
                    _ => return EventResult::NotHandled,
                };
                self.commit_page_change(next);
                EventResult::Handled
            }
            SystemEvent::Wheel { pos, delta } => {
                if !self.body_contains(*pos) {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.body_viewport_height();
                let old_y = self.body_scroll.scroll_offset();
                let max = (self.body_content_height() - viewport_h).max(0.0);
                let next_y = (old_y + delta.y * 40.0).clamp(0.0, max);
                self.body_scroll.set_scroll_offset(next_y);
                let dy = next_y - old_y;

                let old_x = self.horizontal_scroll.get();
                let next_x = (old_x + delta.x * 40.0)
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
                    if dx.abs() > 0.01 && !self.view_columns.is_empty() {
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
                if let Some(column) = self.resize_handle_at_point(*pos) {
                    let start_width = self.columns[column].width;
                    self.resize_drag.set(Some(TableResizeDrag {
                        column,
                        start_x: pos.x,
                        start_width,
                    }));
                    self.hover_resize_column.set(Some(column));
                    self.pressed_action.set(None);
                    return EventResult::Handled;
                }
                if let Some(action) = self.action_at_point(*pos) {
                    self.pressed_action.set(Some(action));
                    self.hover_row.set(Self::action_row(action));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp {
                pos,
                button: crate::ui::MouseButton::Left,
                ..
            } => {
                if let Some(drag) = self.resize_drag.replace(None) {
                    let valid_release = self.local_frame().contains(*pos)
                        && self.leaf_header_contains(pos.y, Some(drag.column));
                    if !valid_release {
                        self.restore_column_width(drag);
                    } else {
                        self.clamp_horizontal_scroll();
                    }
                    self.hover_resize_column
                        .set(valid_release.then_some(drag.column));
                    return EventResult::Handled;
                }
                let Some(pressed) = self.pressed_action.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.action_at_point(*pos);
                self.hover_row.set(released.and_then(Self::action_row));
                if released == Some(pressed) {
                    self.commit_pointer_action(pressed);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(drag) = self.resize_drag.get() {
                    if !self.local_frame().contains(*pos)
                        || !self.leaf_header_contains(pos.y, Some(drag.column))
                    {
                        self.cancel_column_resize();
                        self.hover_resize_column.set(None);
                    } else {
                        self.resize_column_to_pointer(drag, pos.x);
                        self.hover_resize_column.set(Some(drag.column));
                    }
                    return EventResult::Handled;
                }
                let resize_column = self.resize_handle_at_point(*pos);
                let resize_changed = self.hover_resize_column.replace(resize_column) != resize_column;
                let next = resize_column
                    .is_none()
                    .then(|| self.action_at_point(*pos).and_then(Self::action_row))
                    .flatten();
                if resize_changed | (self.hover_row.replace(next) != next) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hover_row.replace(None).is_some()
                    | self.pressed_action.replace(None).is_some()
                    | self.hover_resize_column.replace(None).is_some()
                    | self.cancel_column_resize();
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
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

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.resize_drag.get().is_some() }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
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
        if self.rows.is_empty() && !self.loading {
            let loc = crate::ui::component::locale::use_locale();
            let empty = if self.empty_text.is_empty() { loc.empty_data } else { &self.empty_text };
            let horizontal_inset = 16.0_f32.min(frame.w * 0.25);
            Self::paint_single_line(
                ctx,
                empty,
                Rect::new(
                    frame.x + horizontal_inset,
                    frame.y,
                    (frame.w - horizontal_inset * 2.0).max(0.0),
                    (frame.h - self.pagination_height()).max(0.0),
                ),
                text_sec,
                14.0,
            );
            if self.bordered {
                ctx.stroke_rect(frame, border, 1.0, r);
            }
            self.paint_pagination(frame, ctx);
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
            let is_pressed = self
                .pressed_action
                .get()
                .and_then(Self::action_row)
                == Some(actual_ri)
                && is_hovered;

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

            let row_bg = if is_pressed {
                ctx.tokens().color_fill_secondary()
            } else if is_selected {
                sel_bg
            } else if is_hovered {
                hover_bg
            } else if actual_ri.is_multiple_of(2) {
                bg
            } else {
                ctx.tokens().color_bg_container()
            };
            let row_rect = Rect::new(frame.x, row_y, frame.w, self.row_h);
            ctx.fill_rect(row_rect, row_bg, None);

            if self.selection {
                crate::ui::widgets::Icon::paint_in_frame(
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
                ctx.push_clip(clip);
                for laid_out in column_geometry
                    .columns
                    .iter()
                    .filter(|column| column.zone == zone)
                {
                    if self.is_cell_covered(actual_ri, laid_out.index) {
                        continue;
                    }
                    let row_span = self.row_span(actual_ri, laid_out.index);
                    let col_span = self.col_span(actual_ri, laid_out.index);
                    let cell_width = self.span_width(&column_geometry, laid_out.index, col_span);
                    let cell_height = self.span_height(actual_ri, row_span);
                    if !self.view_columns.contains(&laid_out.index) {
                        let cell = row
                            .get(laid_out.index)
                            .map(String::as_str)
                            .unwrap_or("");
                        let tc = if is_selected { primary } else { text_color };
                        let cell_frame = Rect::new(
                            laid_out.x,
                            row_rect.y,
                            cell_width,
                            cell_height,
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
                                laid_out.x + cell_width - 1.0,
                                row_rect.y,
                                1.0,
                                cell_height,
                            ),
                            border,
                            None,
                        );
                    }
                }
                ctx.pop_clip();
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

        // Row backgrounds and separators are painted one physical row at a time. Repaint merged
        // anchors after those rows so a row-spanning cell keeps one background, border and text
        // rectangle instead of being split by later rows.
        if self.has_spans() {
            let mut merged_anchors = HashSet::new();
            for visible_row in start..end {
                for column in 0..self.columns.len() {
                    if let Some(anchor) = self.cell_anchor(visible_row, column) {
                        let row_span = self.row_span(anchor.0, anchor.1);
                        let col_span = self.col_span(anchor.0, anchor.1);
                        if row_span > 1 || col_span > 1 {
                            merged_anchors.insert(anchor);
                        }
                    }
                }
            }
            let mut merged_anchors = merged_anchors.into_iter().collect::<Vec<_>>();
            merged_anchors.sort_unstable();
            for (row_index, column_index) in merged_anchors {
                let Some(row) = self.rows.get(row_index) else {
                    continue;
                };
                let Some(laid_out) = column_geometry
                    .columns
                    .iter()
                    .find(|column| column.index == column_index)
                else {
                    continue;
                };
                let row_span = self.row_span(row_index, column_index);
                let col_span = self.col_span(row_index, column_index);
                let cell_width = self.span_width(&column_geometry, column_index, col_span);
                let cell_height = self.span_height(row_index, row_span);
                let expanded_offset = if expanded.is_some_and(|expanded_row| row_index > expanded_row)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + row_index as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                let cell_frame = Rect::new(laid_out.x, row_y, cell_width, cell_height);
                if cell_frame.intersect(&body_clip).is_none() {
                    continue;
                }
                let Some(zone_clip) =
                    column_geometry.clip_for(laid_out.zone, cell_frame.y, cell_frame.h)
                else {
                    continue;
                };
                let is_selected = sel == Some(row_index);
                let is_hovered = hover == Some(row_index);
                let is_pressed = self
                    .pressed_action
                    .get()
                    .and_then(Self::action_row)
                    == Some(row_index)
                    && is_hovered;
                let cell_bg = if is_pressed {
                    ctx.tokens().color_fill_secondary()
                } else if is_selected {
                    sel_bg
                } else if is_hovered {
                    hover_bg
                } else if row_index.is_multiple_of(2) {
                    bg
                } else {
                    ctx.tokens().color_bg_container()
                };

                ctx.push_clip(zone_clip);
                ctx.fill_rect(cell_frame, cell_bg, None);
                if !self.view_columns.contains(&column_index) {
                    let cell = row.get(column_index).map(String::as_str).unwrap_or("");
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
                        if is_selected { primary } else { text_color },
                        12.0,
                    );
                }
                if self.bordered {
                    ctx.stroke_rect(cell_frame, border, 1.0, None);
                } else {
                    ctx.fill_rect(
                        Rect::new(
                            cell_frame.x,
                            cell_frame.y + cell_frame.h,
                            cell_frame.w,
                            1.0,
                        ),
                        border,
                        None,
                    );
                }
                ctx.pop_clip();
            }
        }

        // Expansion affordances belong to physical rows. Paint them after merged cells so a
        // colspan reaching the trailing edge cannot obscure the icon.
        if self.expandable {
            for actual_ri in start..end {
                let expanded_offset = if expanded.is_some_and(|expanded_row| actual_ri > expanded_row)
                {
                    self.expand_height
                } else {
                    0.0
                };
                let row_y = body_top + actual_ri as f32 * self.row_h + expanded_offset
                    - self.body_scroll.scroll_offset();
                let icon_size = 18.0_f32.min(self.row_h).min(frame.w);
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if expanded == Some(actual_ri) {
                        "chevron-up"
                    } else {
                        "chevron-down"
                    },
                    Rect::new(
                        frame.x + (frame.w - icon_size - 6.0).max(0.0),
                        row_y + (self.row_h - icon_size) * 0.5,
                        icon_size,
                        icon_size,
                    ),
                    text_sec,
                    10.0,
                );
            }
        }

        ctx.pop_clip();
        self.paint_pagination(frame, ctx);
        if self.loading {
            self.paint_loading_overlay(frame, ctx);
        }
        if self.bordered {
            ctx.stroke_rect(frame, border, 1.0, r);
        }
        if self.focused && tree.keyboard_focus_visible() {
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

    update_animation => (&mut self, dt: f64) -> bool {
        self.loading_dirty = false;
        if !self.loading {
            return false;
        }
        let before = self.loading_phase;
        self.loading_phase = (self.loading_phase
            + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
            .rem_euclid(std::f32::consts::TAU);
        self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.loading_dirty {
            self.loading_spinner_bounds(Self::normalized_frame(frame))
        } else {
            Rect::zero()
        }
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        let header_height = self.total_header_height();
        Some(Rect::new(
            frame.x,
            frame.y + header_height + 1.0,
            frame.w,
            (frame.h - header_height - 1.0 - self.pagination_height()).max(0.0),
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
                // View children stay in content coordinates. The compositor and
                // hit-test path apply the viewport scroll offset, so unchanged
                // cells do not need new frames for every vertical wheel event.
                let row_y = body_top + row as f32 * self.row_h + expanded_offset;
                if self.cell_anchor(row, column_index) != Some((row, column_index)) {
                    positions.push((child.id, Rect::new(column.x, row_y, 0.0, 0.0)));
                    continue;
                }
                let row_span = self.row_span(row, column_index);
                let col_span = self.col_span(row, column_index);
                let cell_width = self.span_width(&column_geometry, column_index, col_span);
                let cell_height = self.span_height(row, row_span);
                let cell = Rect::new(column.x, row_y, cell_width, cell_height);
                positions.push((
                    child.id,
                    column_geometry
                        .clip_for(column.zone, row_y, cell_height)
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
            loading: false,
            loading_phase: 0.0,
            loading_dirty: false,
            empty_text: String::new(),
            current_page: Cell::new(0),
            page_size: 20,
            pagination: None,
            row_click: None,
            pending_change: RefCell::new(None),
            virtual_scroll: false,
            body_scroll: VirtualListScroll::new(),
            horizontal_scroll: Cell::new(0.0),
            horizontal_scroll_requires_paint: Cell::new(false),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            layout_requested: Cell::new(false),
            focused: false,
            pressed_action: Cell::new(None),
            resize_drag: Cell::new(None),
            hover_resize_column: Cell::new(None),
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
        ctx: &mut PaintContext,
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
        ctx: &mut PaintContext,
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

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    fn local_frame(&self) -> Rect {
        let size = self.last_frame.get().map_or_else(
            || {
                let content_width = self
                    .columns
                    .iter()
                    .map(|column| finite_nonnegative(column.width))
                    .sum::<f32>()
                    + self.selection_width();
                Size::new(
                    self.fixed_width.unwrap_or(content_width.max(400.0)),
                    self.total_header_height()
                        + self.rows.len() as f32 * self.row_h
                        + 1.0
                        + self.pagination_height(),
                )
            },
            |frame| Size::new(finite_nonnegative(frame.w), finite_nonnegative(frame.h)),
        );
        Rect::new(0.0, 0.0, size.w, size.h)
    }

    fn body_contains(&self, point: crate::core::Point) -> bool {
        let frame = self.local_frame();
        Rect::new(
            frame.x,
            self.total_header_height() + 1.0,
            frame.w,
            self.body_viewport_height(),
        )
        .contains(point)
    }

    fn action_at_point(&self, point: crate::core::Point) -> Option<TablePointerAction> {
        if !self.local_frame().contains(point) {
            return None;
        }
        if let Some(page) = self.pagination_page_at_point(point) {
            return Some(TablePointerAction::ChangePage(page));
        }
        if self.selection && point.x < self.selection_width() {
            if point.y < self.total_header_height() {
                return (!self.rows.is_empty()).then_some(TablePointerAction::ToggleAll);
            }
            return self
                .row_index_at_y(point.y)
                .map(TablePointerAction::ToggleRow);
        }

        let column = self.column_at_x(point.x);
        if self.leaf_header_contains(point.y, column) {
            if let Some(index) = column.filter(|index| {
                self.sortable
                    || self
                        .columns
                        .get(*index)
                        .is_some_and(|column| column.sortable)
            }) {
                return Some(TablePointerAction::SortColumn(index));
            }
        }

        let row = self.row_index_at_y(point.y)?;
        if self.expandable && self.expand_toggle_hit(point.x) {
            Some(TablePointerAction::ToggleExpand(row))
        } else {
            let row = column
                .and_then(|column| self.cell_anchor(row, column))
                .map_or(row, |(anchor_row, _)| anchor_row);
            Some(TablePointerAction::SelectRow(row))
        }
    }

    fn declared_row_span(&self, row: usize, column: usize) -> usize {
        self.rows
            .get(row)
            .and_then(|data| {
                self.columns
                    .get(column)
                    .and_then(|column_def| column_def.row_span.map(|span| span(data, column)))
            })
            .unwrap_or(1)
    }

    fn declared_col_span(&self, row: usize, column: usize) -> usize {
        self.rows
            .get(row)
            .and_then(|data| {
                self.columns
                    .get(column)
                    .and_then(|column_def| column_def.col_span.map(|span| span(data, column)))
            })
            .unwrap_or(1)
    }

    fn row_span(&self, row: usize, column: usize) -> usize {
        let declared = self.declared_row_span(row, column);
        if declared == 0 {
            0
        } else {
            declared.min(self.rows.len().saturating_sub(row).max(1))
        }
    }

    fn col_span(&self, row: usize, column: usize) -> usize {
        let declared = self.declared_col_span(row, column);
        if declared == 0 {
            0
        } else {
            declared.min(self.columns.len().saturating_sub(column).max(1))
        }
    }

    fn has_spans(&self) -> bool {
        self.columns
            .iter()
            .any(|column| column.row_span.is_some() || column.col_span.is_some())
    }

    fn cell_anchor(&self, row: usize, column: usize) -> Option<(usize, usize)> {
        if row >= self.rows.len() || column >= self.columns.len() {
            return None;
        }
        if !self.has_spans() {
            return Some((row, column));
        }
        for anchor_row in 0..=row {
            for anchor_column in 0..=column {
                let row_span = self.row_span(anchor_row, anchor_column);
                let col_span = self.col_span(anchor_row, anchor_column);
                if row_span > 0
                    && col_span > 0
                    && row < anchor_row.saturating_add(row_span)
                    && column < anchor_column.saturating_add(col_span)
                {
                    return Some((anchor_row, anchor_column));
                }
            }
        }
        None
    }

    fn is_cell_covered(&self, row: usize, column: usize) -> bool {
        self.cell_anchor(row, column) != Some((row, column))
    }

    fn span_width(&self, geometry: &TableColumnGeometry, column: usize, span: usize) -> f32 {
        geometry
            .columns
            .iter()
            .filter(|laid_out| {
                laid_out.index >= column && laid_out.index < column.saturating_add(span)
            })
            .map(|laid_out| laid_out.width)
            .sum::<f32>()
            .max(self.columns.get(column).map_or(0.0, |column| column.width))
    }

    fn span_height(&self, row: usize, span: usize) -> f32 {
        let mut height = self.row_h * span as f32;
        if self
            .expanded_row
            .get()
            .is_some_and(|expanded| expanded >= row && expanded < row.saturating_add(span))
        {
            height += self.expand_height;
        }
        height
    }

    fn action_row(action: TablePointerAction) -> Option<usize> {
        match action {
            TablePointerAction::ToggleRow(row)
            | TablePointerAction::ToggleExpand(row)
            | TablePointerAction::SelectRow(row) => Some(row),
            TablePointerAction::ToggleAll
            | TablePointerAction::SortColumn(_)
            | TablePointerAction::ChangePage(_) => None,
        }
    }

    fn commit_pointer_action(&mut self, action: TablePointerAction) {
        match action {
            TablePointerAction::ToggleAll => {
                if self.checked_rows.len() == self.rows.len() {
                    self.checked_rows.clear();
                } else {
                    self.checked_rows = (0..self.rows.len()).collect();
                }
            }
            TablePointerAction::ToggleRow(row) => {
                if let Some(index) = self.checked_rows.iter().position(|&item| item == row) {
                    self.checked_rows.remove(index);
                } else {
                    self.checked_rows.push(row);
                }
            }
            TablePointerAction::SortColumn(index) => {
                let Some(column) = self.columns.get(index) else {
                    return;
                };
                let sort_direction = match column.sort_direction {
                    SortDirection::None => SortDirection::Asc,
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::None,
                };
                for column in &mut self.columns {
                    column.sort_direction = SortDirection::None;
                }
                self.columns[index].sort_direction = sort_direction;
                self.pending_change.replace(Some(
                    TableChange {
                        sort_column: Some(index),
                        sort_direction,
                        page: self.current_page.get(),
                        page_size: self.page_size,
                    }
                    .payload(),
                ));
            }
            TablePointerAction::ToggleExpand(row) => {
                let next = (self.expanded_row.get() != Some(row)).then_some(row);
                self.expanded_row.set(next);
                self.selected_row.set(Some(row));
                self.layout_requested.set(true);
            }
            TablePointerAction::SelectRow(row) => {
                self.selected_row.set(Some(row));
                if let (Some(callback), Some(data)) = (self.row_click.as_ref(), self.rows.get(row))
                {
                    callback(data, row);
                }
            }
            TablePointerAction::ChangePage(page) => self.commit_page_change(page),
        }
    }

    pub(super) fn header_selection_pressed(&self) -> bool {
        self.pressed_action.get() == Some(TablePointerAction::ToggleAll)
    }

    pub(super) fn pressed_sort_column(&self) -> Option<usize> {
        match self.pressed_action.get() {
            Some(TablePointerAction::SortColumn(index)) => Some(index),
            _ => None,
        }
    }

    pub(super) fn resize_indicator_column(&self) -> Option<usize> {
        self.resize_drag
            .get()
            .map(|drag| drag.column)
            .or(self.hover_resize_column.get())
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
            empty_renderer: None,
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
    /// 在表体上方显示加载遮罩，并暂时阻止表格交互。
    pub fn loading(mut self, enabled: bool) -> Self {
        self.loading = enabled;
        if !enabled {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
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

    /// 声明式虚拟化开关（E-02）：等价于 `virtual_scroll`。
    pub fn virtualized(mut self, enabled: bool) -> Self {
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
    /// 数据为空且非加载时，用自定义 View 替换默认空态；工厂接收当前列数。
    ///
    /// 闭包只在 build 边界消费，不进入 Table 组件；优先于 ConfigProvider 空态。
    pub fn empty<V>(self, renderer: impl Fn(usize) -> V + 'static) -> TableBuilder
    where
        V: crate::ui::view::View,
    {
        TableBuilder {
            table: self,
            expand_renderer: None,
            empty_renderer: Some(Box::new(move |column_count| {
                crate::ui::view::View::build(renderer(column_count))
            })),
        }
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
            expand_renderer: Some(Box::new(move |row| {
                crate::ui::view::View::build(renderer(row))
            })),
            empty_renderer: None,
        }
    }
    pub fn page_size(mut self, n: usize) -> Self {
        self.page_size = n;
        self
    }

    /// 配置由应用层驱动的远程分页。
    pub fn pagination<F>(mut self, pagination: TablePagination<F>) -> Self
    where
        F: Fn(usize) + 'static,
    {
        let page_size = pagination.page_size.max(1);
        let total_pages = pagination.total.div_ceil(page_size).max(1);
        let current = pagination.current.clamp(1, total_pages);
        self.current_page.set(current - 1);
        self.page_size = page_size;
        self.pagination = Some((
            current,
            pagination.total,
            page_size,
            Rc::new(pagination.on_change),
        ));
        self
    }

    /// 在行主体按下并释放于同一行时调用；复选框与展开按钮不会触发该回调。
    pub fn on_row_click<F>(mut self, callback: F) -> Self
    where
        F: Fn(&TableRow, usize) + 'static,
    {
        self.row_click = Some(Rc::new(callback));
        self
    }

    fn selection_width(&self) -> f32 {
        if self.selection {
            32.0
        } else {
            0.0
        }
    }

    fn loading_body_rect(&self, frame: Rect) -> Rect {
        let header_height = (self.total_header_height() + 1.0).min(frame.h);
        Rect::new(
            frame.x,
            frame.y + header_height,
            frame.w,
            (frame.h - header_height).max(0.0),
        )
    }

    fn loading_spinner_bounds(&self, frame: Rect) -> Rect {
        let body = self.loading_body_rect(frame);
        let radius = 10.0_f32.min(body.w.min(body.h) * 0.3);
        let dot_radius = radius * 0.18;
        let extent = radius + dot_radius + 1.0;
        Rect::new(
            body.x + body.w * 0.5 - extent,
            body.y + body.h * 0.5 - extent,
            extent * 2.0,
            extent * 2.0,
        )
    }

    fn paint_loading_overlay(&self, frame: Rect, ctx: &mut PaintContext) {
        let body = self.loading_body_rect(frame);
        if body.w <= 0.0 || body.h <= 0.0 {
            return;
        }
        ctx.fill_rect(body, ctx.tokens().color_text().with_alpha(30), None);
        let radius = 10.0_f32.min(body.w.min(body.h) * 0.3);
        if radius <= 0.0 {
            return;
        }
        let cx = body.x + body.w * 0.5;
        let cy = body.y + body.h * 0.5;
        let dot_radius = radius * 0.18;
        let primary = ctx.tokens().color_primary();
        for index in 0..8 {
            let angle = self.loading_phase + index as f32 * std::f32::consts::TAU / 8.0;
            let opacity = 0.25 + index as f32 / 8.0 * 0.75;
            ctx.fill_circle(
                cx + angle.cos() * radius,
                cy + angle.sin() * radius,
                dot_radius,
                primary.with_alpha((primary.a as f32 * opacity) as u8),
            );
        }
    }

    fn total_header_height(&self) -> f32 {
        if self.column_groups.is_empty() {
            self.header_h
        } else {
            self.header_h * 2.0
        }
    }

    fn pagination_height(&self) -> f32 {
        if self.pagination.is_some() {
            TABLE_PAGINATION_HEIGHT
        } else {
            0.0
        }
    }

    fn pagination_total_pages(&self) -> usize {
        self.pagination
            .as_ref()
            .map_or(1, |(_, total, page_size, _)| {
                total.div_ceil((*page_size).max(1)).max(1)
            })
    }

    fn pagination_current(&self) -> usize {
        self.pagination.as_ref().map_or(1, |(current, _, _, _)| {
            (*current).clamp(1, self.pagination_total_pages())
        })
    }

    fn pagination_controls(&self, frame: Rect) -> Option<(Rect, Rect, Rect)> {
        self.pagination.as_ref()?;
        let footer_height = TABLE_PAGINATION_HEIGHT.min(frame.h.max(0.0));
        let inset = TABLE_PAGINATION_INSET.min(frame.w.max(0.0) * 0.1);
        let available = (frame.w - inset * 2.0).max(0.0);
        let gap = TABLE_PAGINATION_GAP.min(available * 0.05);
        let label_width = TABLE_PAGINATION_LABEL_WIDTH.min(available * 0.5);
        let item_size = TABLE_PAGINATION_ITEM_SIZE
            .min(((available - label_width - gap * 2.0) * 0.5).max(0.0))
            .min(footer_height);
        let controls_width = item_size * 2.0 + label_width + gap * 2.0;
        let x = frame.x + (frame.w - inset - controls_width).max(0.0);
        let footer_y = frame.y + frame.h - footer_height;
        let y = footer_y + (footer_height - item_size) * 0.5;
        let previous = Rect::new(x, y, item_size, item_size);
        let label = Rect::new(previous.x + previous.w + gap, y, label_width, item_size);
        let next = Rect::new(label.x + label.w + gap, y, item_size, item_size);
        Some((previous, label, next))
    }

    fn pagination_page_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let (previous, _, next) = self.pagination_controls(self.local_frame())?;
        let current = self.pagination_current();
        if previous.contains(point) {
            Some(current.saturating_sub(1).max(1))
        } else if next.contains(point) {
            Some(current.saturating_add(1).min(self.pagination_total_pages()))
        } else {
            None
        }
    }

    fn commit_page_change(&mut self, page: usize) {
        let Some((current, _, page_size, callback)) = self.pagination.as_ref() else {
            return;
        };
        let page = page.clamp(1, self.pagination_total_pages());
        if page == *current {
            return;
        }
        self.current_page.set(page - 1);
        callback(page);
        self.pending_change.replace(Some(
            TableChange {
                sort_column: None,
                sort_direction: SortDirection::None,
                page: page - 1,
                page_size: *page_size,
            }
            .payload(),
        ));
    }

    fn paint_pagination(&self, frame: Rect, ctx: &mut PaintContext) {
        let Some((previous, label, next)) = self.pagination_controls(frame) else {
            return;
        };
        let current = self.pagination_current();
        let total_pages = self.pagination_total_pages();
        let footer = Rect::new(
            frame.x,
            frame.y + (frame.h - TABLE_PAGINATION_HEIGHT).max(0.0),
            frame.w,
            TABLE_PAGINATION_HEIGHT.min(frame.h),
        );
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let secondary = ctx.tokens().color_text_secondary();
        let bg = ctx.tokens().color_bg_container();
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.fill_rect(footer, bg, None);
        ctx.fill_rect(Rect::new(footer.x, footer.y, footer.w, 1.0), border, None);
        for button in [previous, next] {
            ctx.fill_rect(button, bg, radius);
            ctx.stroke_rect(button, border, 1.0, radius);
        }
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            "chevron-left",
            previous,
            if current <= 1 { secondary } else { text },
            14.0,
        );
        ctx.text_center(&format!("{current} / {total_pages}"), label, text, 13.0);
        crate::ui::widgets::Icon::paint_in_frame(
            ctx,
            "chevron-right",
            next,
            if current >= total_pages {
                secondary
            } else {
                text
            },
            14.0,
        );
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

    fn resize_handle_at_point(&self, point: crate::core::Point) -> Option<usize> {
        let frame = self.local_frame();
        if !frame.contains(point) {
            return None;
        }
        let geometry = self.column_geometry(0.0, frame.w);
        geometry
            .columns
            .iter()
            .filter(|laid_out| {
                self.columns
                    .get(laid_out.index)
                    .is_some_and(|column| column.resizable)
                    && self.leaf_header_contains(point.y, Some(laid_out.index))
                    && geometry
                        .clip_for(laid_out.zone, point.y, 1.0)
                        .is_some_and(|clip| {
                            let edge = laid_out.x + laid_out.width;
                            edge >= clip.x
                                && edge <= clip.x + clip.w
                                && (point.x - edge).abs() <= COLUMN_RESIZE_HANDLE_HALF_WIDTH
                        })
            })
            .min_by(|left, right| {
                let left_distance = (point.x - (left.x + left.width)).abs();
                let right_distance = (point.x - (right.x + right.width)).abs();
                left_distance.total_cmp(&right_distance)
            })
            .map(|column| column.index)
    }

    fn resize_column_to_pointer(&mut self, drag: TableResizeDrag, pointer_x: f32) {
        let Some(column) = self.columns.get_mut(drag.column) else {
            self.resize_drag.set(None);
            return;
        };
        let width = (drag.start_width + pointer_x - drag.start_x).max(MIN_RESIZABLE_COLUMN_WIDTH);
        if (column.width - width).abs() <= 0.01 {
            return;
        }
        column.width = width;
        self.horizontal_scroll_requires_paint.set(true);
        self.layout_requested.set(true);
    }

    fn restore_column_width(&mut self, drag: TableResizeDrag) {
        let Some(column) = self.columns.get_mut(drag.column) else {
            return;
        };
        if (column.width - drag.start_width).abs() <= 0.01 {
            return;
        }
        column.width = drag.start_width;
        self.clamp_horizontal_scroll();
        self.horizontal_scroll_requires_paint.set(true);
        self.layout_requested.set(true);
    }

    fn cancel_column_resize(&mut self) -> bool {
        let Some(drag) = self.resize_drag.replace(None) else {
            return false;
        };
        self.restore_column_width(drag);
        true
    }

    fn clamp_horizontal_scroll(&self) {
        self.horizontal_scroll.set(
            self.horizontal_scroll
                .get()
                .min(self.horizontal_max_scroll()),
        );
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
            .map(|f| {
                (finite_nonnegative(f.h)
                    - self.total_header_height()
                    - 1.0
                    - self.pagination_height())
                .max(0.0)
            })
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
        crate::ui::virtualization::virtual_scroll::virtual_list_index_range(
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
            loading: self.loading,
            selected_row: self.selected_row.get(),
            checked_rows: self.checked_rows.clone(),
            empty_text: self.empty_text.clone(),
            current_page: self.pagination.as_ref().map(|(current, _, _, _)| *current),
            total: self.pagination.as_ref().map(|(_, total, _, _)| *total),
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
        self.current_page.set(next.current_page.get());
        self.pagination = next.pagination;
        self.row_click = next.row_click;
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
        self.pressed_action.set(None);
        self.hover_resize_column.set(None);
        if self.resize_drag.get().is_some_and(|drag| {
            !self
                .columns
                .get(drag.column)
                .is_some_and(|column| column.resizable)
        }) {
            self.resize_drag.set(None);
        }
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
        let viewport_height = (finite_nonnegative(frame.h)
            - self.total_header_height()
            - 1.0
            - self.pagination_height())
        .max(0.0);
        let (visible_start, end) = self.visible_row_range(viewport_height);
        let mut start = visible_start;
        if self.has_spans() {
            for row in visible_start..end {
                for &column in &self.view_columns {
                    if let Some((anchor_row, _)) = self.cell_anchor(row, column) {
                        start = start.min(anchor_row);
                    }
                }
            }
        }
        (start, end)
    }

    #[cfg(test)]
    pub(crate) fn pagination_controls_for_test(&self, frame: Rect) -> Option<(Rect, Rect, Rect)> {
        self.pagination_controls(frame)
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

    pub fn loading(mut self, enabled: bool) -> Self {
        self.table = self.table.loading(enabled);
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

    /// 声明式虚拟化开关（E-02）：等价于 `virtual_scroll`，大数据集只物化
    /// 可视区及 overscan 范围内的行；`false` 时全量物化。
    pub fn virtualized(mut self, enabled: bool) -> Self {
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

    /// 数据为空且非加载时，用自定义 View 替换默认空态；工厂接收当前列数。
    pub fn empty<V>(mut self, renderer: impl Fn(usize) -> V + 'static) -> Self
    where
        V: crate::ui::view::View,
    {
        self.empty_renderer = Some(Box::new(move |column_count| {
            crate::ui::view::View::build(renderer(column_count))
        }));
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }

    pub fn pagination<F>(mut self, pagination: TablePagination<F>) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.table = self.table.pagination(pagination);
        self
    }

    fn into_parts(
        self,
    ) -> (
        Table,
        Option<RenderHandlerRegistration>,
        Option<TableEmptyRenderer>,
    )
    where
        R: 'static,
    {
        let Self {
            mut table,
            rows,
            row_keys,
            columns,
            empty_renderer,
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
            return (table, None, empty_renderer);
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
        (
            table,
            Some(RenderHandlerRegistration::TableCells(renderer)),
            empty_renderer,
        )
    }
}

impl<R: 'static> crate::ui::view::View for DataTable<R> {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handler, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return empty;
        }
        let mut node = crate::ui::view::View::build(table);
        if let Some(handler) = handler {
            node.render_handlers.push(handler);
        }
        node
    }
}

impl<R: 'static> crate::ui::IntoWidgetNode for DataTable<R> {
    fn into_node(self) -> crate::ui::component::widget::WidgetNode {
        let (table, handler, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return crate::ui::IntoWidgetNode::into_node(empty);
        }
        let mut node = crate::ui::IntoWidgetNode::into_node(table);
        if let Some(handler) = handler {
            node.render_handlers.push(handler);
        }
        node
    }
}

impl TableBuilder {
    fn into_parts(
        self,
    ) -> (
        Table,
        Vec<RenderHandlerRegistration>,
        Option<TableEmptyRenderer>,
    ) {
        let mut handlers = Vec::new();
        if let Some(expand) = self.expand_renderer {
            handlers.push(RenderHandlerRegistration::TableExpand(expand));
        }
        (self.table, handlers, self.empty_renderer)
    }

    /// 数据为空且非加载时，用自定义 View 替换默认空态；工厂接收当前列数。
    pub fn empty<V>(mut self, renderer: impl Fn(usize) -> V + 'static) -> Self
    where
        V: crate::ui::view::View,
    {
        self.empty_renderer = Some(Box::new(move |column_count| {
            crate::ui::view::View::build(renderer(column_count))
        }));
        self
    }

    /// 为展开行声明普通 View 子树；可与 `.empty` 组合。
    pub fn expandable<V>(mut self, height: f32, renderer: impl Fn(&TableRow) -> V + 'static) -> Self
    where
        V: crate::ui::view::View,
    {
        self.table.expandable = true;
        self.table.expand_height = finite_nonnegative(height);
        self.expand_renderer = Some(Box::new(move |row| {
            crate::ui::view::View::build(renderer(row))
        }));
        self
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

    pub fn loading(mut self, enabled: bool) -> Self {
        self.table = self.table.loading(enabled);
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

    /// 声明式虚拟化开关（E-02）：等价于 `virtual_scroll`。
    pub fn virtualized(mut self, enabled: bool) -> Self {
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

    pub fn pagination<F>(mut self, pagination: TablePagination<F>) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.table = self.table.pagination(pagination);
        self
    }
}

impl crate::ui::IntoWidgetNode for TableBuilder {
    fn into_node(self) -> crate::ui::component::widget::WidgetNode {
        let (table, handlers, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return crate::ui::IntoWidgetNode::into_node(empty);
        }
        crate::ui::component::widget::WidgetNode::leaf(Box::new(table))
            .with_render_handlers(handlers)
    }
}

impl crate::ui::view::View for TableBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handlers, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return empty;
        }
        let mut node = crate::ui::view::ViewNode::leaf(table);
        node.render_handlers.extend(handlers);
        node
    }
}

impl From<TableBuilder> for crate::ui::view::ViewNode {
    fn from(builder: TableBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}

impl crate::ui::view::View for Table {
    fn build(self) -> crate::ui::view::ViewNode {
        if let Some(empty) = resolve_table_empty_view(&self, None) {
            return empty;
        }
        crate::ui::view::ViewNode::leaf(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct UserRow {
        id: u64,
        name: String,
    }

    fn data_table(rows: Vec<UserRow>) -> DataTable<UserRow> {
        let Ok(table) = Table::data(rows.clone(), |row| row.id.to_string()) else {
            panic!("Table::data must accept a stable id extractor");
        };
        table
            .columns(vec![
                TableColumn::new("ID", 80.0).bind(|r: &UserRow| r.id.to_string())
            ])
            .row_height(32.0)
    }

    #[test]
    fn virtualized_true_materializes_viewport_only() {
        let rows = (0..1000)
            .map(|id| UserRow {
                id,
                name: format!("row-{id}"),
            })
            .collect::<Vec<_>>();
        let data = data_table(rows).virtualized(true);
        let (table, _, _) = data.into_parts();

        // 1000 行 + 32px 行高 + 320px 视口：只物化可视区与 overscan。
        let (start, end) = table.visible_row_range(320.0);
        assert!(
            end - start <= 30,
            "虚拟化应限制物化行数，实际 {}",
            end - start
        );
        assert!(end > start);
        assert_eq!(table.row_keys().len(), 1000, "行身份仍保留全量");
    }

    #[test]
    fn virtualized_false_materializes_all_rows() {
        let rows = (0..1000)
            .map(|id| UserRow {
                id,
                name: format!("row-{id}"),
            })
            .collect::<Vec<_>>();
        let data = data_table(rows).virtualized(false);
        let (table, _, _) = data.into_parts();

        assert_eq!(table.visible_row_range(320.0), (0, 1000));
    }

    #[test]
    fn virtualized_is_alias_of_virtual_scroll() {
        let rows = (0..100)
            .map(|id| UserRow {
                id,
                name: format!("row-{id}"),
            })
            .collect::<Vec<_>>();
        let on = data_table(rows.clone()).virtualized(true);
        let (table_on, _, _) = on.into_parts();
        assert!(table_on.virtual_scroll);

        let off = data_table(rows).virtualized(false);
        let (table_off, _, _) = off.into_parts();
        assert!(!table_off.virtual_scroll);
    }
}
