//! [`Table`] 组件实现（一）：构造与装配 — table 子模块。

use crate::core::{Rect, Size};
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::widget_runtime::paint_context::PaintContext;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use super::Table;
use super::builder::TableBuilder;
use super::config::flatten_column_groups;
use super::presentation::{TABLE_VISUAL, TABLE_VISUAL_REF, TablePaginationLabelCache};
use super::types::{
    DataTable, SortDirection, TableChange, TableColumn, TableColumnGroup, TableDataError,
    TablePagination, TablePointerAction, TableRow, finite_nonnegative, implicit_row_keys,
};

impl Table {
    /// 创建默认行高、无边框且未启用排序、选择或虚拟滚动的空表格。
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            column_groups: Vec::new(),
            rows: Vec::new(),
            row_keys: Vec::new(),
            view_columns: Vec::new(),
            materialized_cell_range: Cell::new(None),
            row_h: TABLE_VISUAL.geometry.row_height,
            row_h_authored: false,
            header_h: TABLE_VISUAL.geometry.header_height,
            fixed_width: None,
            fixed_height: None,
            selected_row: Cell::new(None),
            hover_row: Cell::new(None),
            checked_rows: Vec::new(),
            expanded_row: Cell::new(None),
            expanded_child_row: Cell::new(None),
            expandable: false,
            expand_height: TABLE_VISUAL.geometry.expand_height,
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
            visual: TABLE_VISUAL_REF,
            pagination_label_cache: RefCell::new(TablePaginationLabelCache::default()),
            column_geometry_cache: RefCell::new(Default::default()),
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
            height.max(TABLE_VISUAL.geometry.min_row_height)
        } else {
            TABLE_VISUAL.geometry.row_height
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
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
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
                    self.fixed_width
                        .unwrap_or(content_width.max(self.visual.geometry.default_width)),
                    self.total_header_height()
                        + self.rows.len() as f32 * self.row_h
                        + self.visual.geometry.body_separator
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
            self.total_header_height() + self.visual.geometry.body_separator,
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
        // 先解析当前横坐标上按绘制层级位于最上方的可见列。
        let column = self.column_at_x(point.x);
        // 提前解析表体物理行，让最后绘制的展开控件先于其下方内容响应。
        let row = self.row_index_at_y(point.y);
        // 展开控件位于表体最上层，其尾部交互区覆盖选择列或普通列时优先。
        if self.expandable && self.expand_toggle_hit(point.x) {
            // 只有真实数据行才能产生展开动作，展开内容区保持不可切换。
            if let Some(row) = row {
                // 返回与最后绘制的物理行展开箭头一致的动作。
                return Some(TablePointerAction::ToggleExpand(row));
            }
        }
        // 只有选择区未被更高绘制层的列覆盖时才响应复选框动作。
        if self.selection && point.x < self.selection_width() && column.is_none() {
            if point.y < self.total_header_height() {
                return (!self.rows.is_empty()).then_some(TablePointerAction::ToggleAll);
            }
            return self
                .row_index_at_y(point.y)
                .map(TablePointerAction::ToggleRow);
        }

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

        // 表头与非数据区域结束后要求命中真实数据行。
        let row = row?;
        // 普通单元格动作按合并单元格锚点回落到所属物理行。
        let row = column
            // 将可见列与当前物理行解析为合并单元格锚点。
            .and_then(|column| self.cell_anchor(row, column))
            // 没有合并单元格时保留当前物理行。
            .map_or(row, |(anchor_row, _)| anchor_row);
        // 返回普通行选择动作。
        Some(TablePointerAction::SelectRow(row))
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
        // 越过表格边界的查询没有对应逻辑单元格。
        if row >= self.rows.len() || column >= self.columns.len() {
            return None;
        }
        // 没有任何跨度时直接返回当前物理单元格。
        if !self.has_spans() {
            return Some((row, column));
        }
        // 以目标列宽建立已被更早锚点覆盖的扫描位图。
        let scan_width = column.saturating_add(1);
        // 只为目标左上矩形分配必要的覆盖状态。
        let mut covered = vec![false; scan_width.saturating_mul(row.saturating_add(1))];
        // 按行主序扫描所有可能的逻辑锚点。
        for anchor_row in 0..=row {
            // 只需扫描目标列之前的逻辑列。
            for anchor_column in 0..=column {
                // 读取当前候选在覆盖位图中的状态。
                let covered_index = anchor_row * scan_width + anchor_column;
                // 已被更早有效锚点覆盖的物理单元格不能再次声明新锚点。
                if covered[covered_index] {
                    continue;
                }
                // 读取当前未覆盖候选的有效行跨度。
                let row_span = self.row_span(anchor_row, anchor_column);
                // 读取当前未覆盖候选的有效列跨度。
                let col_span = self.col_span(anchor_row, anchor_column);
                // 零跨度候选不占用任何物理单元格。
                if row_span == 0 || col_span == 0 {
                    continue;
                }
                // 当前候选覆盖目标时，它就是目标的唯一有效锚点。
                if row < anchor_row.saturating_add(row_span)
                    && column < anchor_column.saturating_add(col_span)
                {
                    return Some((anchor_row, anchor_column));
                }
                // 计算当前候选在扫描矩形内的行覆盖排他末端。
                let covered_row_end = anchor_row
                    .saturating_add(row_span)
                    .min(row.saturating_add(1));
                // 计算当前候选在扫描矩形内的列覆盖排他末端。
                let covered_column_end = anchor_column
                    .saturating_add(col_span)
                    .min(column.saturating_add(1));
                // 将当前有效锚点覆盖的后续物理单元标记为不可再锚定。
                for covered_row in anchor_row..covered_row_end {
                    // 逐列标记当前锚点的物理覆盖范围。
                    for covered_column in anchor_column..covered_column_end {
                        // 保存该物理单元格已经属于更早锚点。
                        covered[covered_row * scan_width + covered_column] = true;
                    }
                }
            }
        }
        // 目标不在任何有效跨度内时保留无锚点结果。
        None
    }

    pub(crate) fn is_cell_covered(&self, row: usize, column: usize) -> bool {
        self.cell_anchor(row, column) != Some((row, column))
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

    /// 替换未分组列、规范化列宽并清除列分组定义。
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
    /// 替换全部行，并按新顺序生成隐式行键。
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
    /// 设置固定行高；有限值至少为一，非有限值恢复为默认行高。
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_h = Self::normalized_row_height(h);
        self.row_h_authored = true;
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
        self.row_h_authored = true;
        self
    }
    /// 返回当前选中行的索引。
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.get()
    }
    /// 设置当前选中行索引，不对索引范围作验证。
    pub fn set_selected_row(&self, row: Option<usize>) {
        self.selected_row.set(row);
    }
    /// 返回当前选中行的稳定键；未选中或索引无效时返回 `None`。
    pub fn selected_row_key(&self) -> Option<&str> {
        self.selected_row
            .get()
            .and_then(|row| self.row_keys.get(row))
            .map(String::as_str)
    }
    /// 返回当前展开行的索引。
    pub fn expanded_row(&self) -> Option<usize> {
        self.expanded_row.get()
    }
    /// 返回当前勾选行的索引列表。
    pub fn checked_rows(&self) -> &[usize] {
        &self.checked_rows
    }
    /// 返回当前有效勾选行对应的稳定键。
    pub fn checked_row_keys(&self) -> Vec<&str> {
        self.checked_rows
            .iter()
            .filter_map(|row| self.row_keys.get(*row).map(String::as_str))
            .collect()
    }
    /// 设置无数据时显示的默认文本。
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
    /// 设置本地分页每页行数。
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
