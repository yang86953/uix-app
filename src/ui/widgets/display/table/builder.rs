//! Table 构建器（静态与数据驱动）— table 子模块。
//!
//! [`TableBuilder`] 与 [`DataTable`] 的声明式装配面：列、行、分页、选择、
//! 虚拟滚动与回调，最终转换为 [`super::Table`] 组件节点。

use std::rc::Rc;

use crate::ui::render_handler::RenderHandlerRegistration;

use super::Table;
use super::config::flatten_column_groups;
use super::types::{
    DataTable, ExpandRenderer, TableCellRenderer, TableColumn, TableColumnGroup, TableDataColumn,
    TableEmptyRenderer, TablePagination, TableRow, finite_nonnegative, implicit_row_keys,
    resolve_table_empty_view,
};

/// 保存表格组件及可选展开行、空态 View 工厂的声明构建器。
pub struct TableBuilder {
    pub(crate) table: Table,
    pub(crate) expand_renderer: Option<ExpandRenderer>,
    pub(crate) empty_renderer: Option<TableEmptyRenderer>,
}

impl<R> DataTable<R> {
    /// 设置把数据记录映射为表格单元格的列声明。
    pub fn columns(mut self, columns: Vec<TableDataColumn<R>>) -> Self {
        self.columns = columns;
        self
    }

    /// 设置是否允许列头触发排序。
    pub fn sortable(mut self, enabled: bool) -> Self {
        self.table.sortable = enabled;
        self
    }

    /// 设置是否显示行复选框并启用多选。
    pub fn selection(mut self, enabled: bool) -> Self {
        self.table.selection = enabled;
        if !enabled {
            self.table.checked_rows.clear();
        }
        self
    }

    /// 设置是否绘制表格外框和单元格纵向边界。
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.table.bordered = enabled;
        self
    }

    /// 设置是否呈现加载状态。
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

    /// 设置表体行高，并将非法或过小数值归一化。
    pub fn row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self.table.row_h_authored = true;
        self
    }

    /// 设置是否只物化表体视口及 overscan 范围内的行。
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

    /// 设置虚拟滚动使用的固定行高；不会隐式开启虚拟滚动。
    pub fn virtual_row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self.table.row_h_authored = true;
        self
    }

    /// 设置没有数据且未加载时显示的默认空态文本。
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

    /// 设置本地分页时每页展示的行数。
    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }

    /// 配置由应用层维护数据并响应页码变化的远程分页。
    pub fn pagination<F>(mut self, pagination: TablePagination<F>) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.table = self.table.pagination(pagination);
        self
    }

    pub(crate) fn into_parts(
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
    fn into_node(self) -> crate::ui::widget_runtime::widget::WidgetNode {
        let (table, handler, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return crate::ui::IntoWidgetNode::into_node(empty);
        }
        // 泛型表格与普通 Table 统一经过同目录 UIX 根，避免命令式转换绕过视觉声明。
        let mut node = super::build_table_uix_root(table);
        if let Some(handler) = handler {
            node.render_handlers.push(handler);
        }
        crate::ui::IntoWidgetNode::into_node(node)
    }
}

impl TableBuilder {
    pub(crate) fn into_parts(
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

    /// 替换普通表格列，并清除已有分组表头配置。
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

    /// 替换表格行，并按当前顺序生成隐式行 key。
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

    /// 设置是否呈现加载状态。
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

    /// 设置表体行高，并将非法或过小数值归一化。
    pub fn row_height(mut self, height: f32) -> Self {
        self.table.row_h = Table::normalized_row_height(height);
        self.table.row_h_authored = true;
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
        self.table.row_h_authored = true;
        self
    }

    /// 设置没有数据且未加载时显示的默认空态文本。
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.table.empty_text = text.into();
        self
    }

    /// 设置本地分页时每页展示的行数。
    pub fn page_size(mut self, size: usize) -> Self {
        self.table.page_size = size;
        self
    }

    /// 配置由应用层维护数据并响应页码变化的远程分页。
    pub fn pagination<F>(mut self, pagination: TablePagination<F>) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.table = self.table.pagination(pagination);
        self
    }
}

impl crate::ui::IntoWidgetNode for TableBuilder {
    fn into_node(self) -> crate::ui::widget_runtime::widget::WidgetNode {
        let (table, handlers, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return crate::ui::IntoWidgetNode::into_node(empty);
        }
        let mut view = super::build_table_uix_root(table);
        view.render_handlers.extend(handlers);
        crate::ui::IntoWidgetNode::into_node(view)
    }
}

impl crate::ui::view::View for TableBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        let (table, handlers, empty_renderer) = self.into_parts();
        if let Some(empty) = resolve_table_empty_view(&table, empty_renderer.as_ref()) {
            return empty;
        }
        let mut node = super::build_table_uix_root(table);
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
        super::build_table_uix_root(self)
    }
}
