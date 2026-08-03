//! [`Table`] 组件实现（一）：构造与装配 — table 子模块。

use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use super::config::{flatten_column_groups, merge_table_columns};
use super::geometry::{ColumnZone, TableColumnGeometry};
use super::builder::TableBuilder;
use super::types::{
    finite_nonnegative, SortDirection, TableChange, TableColumn, TableColumnGroup, DataTable,
    TableDataError, TablePagination, TablePointerAction, TableRow, TableRowClickCallback,
    implicit_row_keys,
};
use super::Table;

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

    pub(crate) fn normalized_columns(mut columns: Vec<TableColumn>) -> Vec<TableColumn> {
        for column in &mut columns {
            column.width = finite_nonnegative(column.width);
        }
        columns
    }

    pub(crate) fn normalized_row_height(height: f32) -> f32 {
        if height.is_finite() {
            height.max(1.0)
        } else {
            28.0
        }
    }

    pub(crate) fn normalized_frame(frame: Rect) -> Rect {
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

    pub(crate) fn elide_single_line(
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

    pub(crate) fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    pub(crate) fn local_frame(&self) -> Rect {
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

    pub(crate) fn body_contains(&self, point: crate::core::Point) -> bool {
        let frame = self.local_frame();
        Rect::new(
            frame.x,
            self.total_header_height() + 1.0,
            frame.w,
            self.body_viewport_height(),
        )
        .contains(point)
    }

    pub(crate) fn action_at_point(&self, point: crate::core::Point) -> Option<TablePointerAction> {
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

    pub(crate) fn declared_row_span(&self, row: usize, column: usize) -> usize {
        self.rows
            .get(row)
            .and_then(|data| {
                self.columns
                    .get(column)
                    .and_then(|column_def| column_def.row_span.map(|span| span(data, column)))
            })
            .unwrap_or(1)
    }

    pub(crate) fn declared_col_span(&self, row: usize, column: usize) -> usize {
        self.rows
            .get(row)
            .and_then(|data| {
                self.columns
                    .get(column)
                    .and_then(|column_def| column_def.col_span.map(|span| span(data, column)))
            })
            .unwrap_or(1)
    }

    pub(crate) fn row_span(&self, row: usize, column: usize) -> usize {
        let declared = self.declared_row_span(row, column);
        if declared == 0 {
            0
        } else {
            declared.min(self.rows.len().saturating_sub(row).max(1))
        }
    }

    pub(crate) fn col_span(&self, row: usize, column: usize) -> usize {
        let declared = self.declared_col_span(row, column);
        if declared == 0 {
            0
        } else {
            declared.min(self.columns.len().saturating_sub(column).max(1))
        }
    }

    pub(crate) fn has_spans(&self) -> bool {
        self.columns
            .iter()
            .any(|column| column.row_span.is_some() || column.col_span.is_some())
    }

    pub(crate) fn cell_anchor(&self, row: usize, column: usize) -> Option<(usize, usize)> {
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

    pub(crate) fn is_cell_covered(&self, row: usize, column: usize) -> bool {
        self.cell_anchor(row, column) != Some((row, column))
    }

    pub(crate) fn span_width(&self, geometry: &TableColumnGeometry, column: usize, span: usize) -> f32 {
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

    pub(crate) fn span_height(&self, row: usize, span: usize) -> f32 {
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

    pub(crate) fn action_row(action: TablePointerAction) -> Option<usize> {
        match action {
            TablePointerAction::ToggleRow(row)
            | TablePointerAction::ToggleExpand(row)
            | TablePointerAction::SelectRow(row) => Some(row),
            TablePointerAction::ToggleAll
            | TablePointerAction::SortColumn(_)
            | TablePointerAction::ChangePage(_) => None,
        }
    }

    pub(crate) fn commit_pointer_action(&mut self, action: TablePointerAction) {
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

}