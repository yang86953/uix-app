//! Table 公开类型与常量 — table 子模块。
//!
//! 列配置、排序/分页契约、数据表格与回调类型；不包含任何组件实现。
//! [`Table`]（widget! 宏）定义在父模块，[`TableBuilder`] 在 [`super::builder`]。

use std::error::Error;
use std::fmt;
use std::rc::Rc;

use super::Table;

// 表模块唯一位宽敏感收敛实现；委托浮层共享纯函数保持单一事实源。
pub(crate) fn finite_nonnegative(value: f32) -> f32 {
    crate::ui::widgets::overlay::finite_nonnegative(value)
}

/// 排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// 当前列不参与排序。
    None,
    /// 按列值升序排列。
    Asc,
    /// 按列值降序排列。
    Desc,
}

/// 表格列的固定位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixed {
    /// 将列固定在表格视口左侧。
    Left,
    /// 将列固定在表格视口右侧。
    Right,
}

/// 表格列定义。
#[derive(Debug, Clone)]
pub struct TableColumn {
    /// 显示在表头中的列标题。
    pub title: String,
    /// 列的初始布局宽度。
    pub width: f32,
    /// 是否允许用户通过表头切换排序。
    pub sortable: bool,
    /// 当前应用于该列的排序方向。
    pub sort_direction: SortDirection,
    /// 是否为该列启用筛选入口。
    pub filterable: bool,
    /// 由筛选标签与激活状态组成的候选项。
    pub filters: Vec<(String, bool)>,
    /// 列在水平滚动视口中的可选固定位置。
    pub fixed: Option<Fixed>,
    /// 是否允许从表头边缘拖拽调整列宽。
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
    /// 使用标题与有限非负宽度创建默认列定义。
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
    /// 设置是否允许用户通过表头切换排序。
    pub fn sortable(mut self, v: bool) -> Self {
        self.sortable = v;
        self
    }
    /// 设置是否为该列启用筛选入口。
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
    /// 跨列显示的可选分组表头；单列声明时为空。
    pub title: Option<String>,
    /// 按显示顺序归入该表头分组的列。
    pub columns: Vec<TableColumn>,
}

impl TableColumnGroup {
    /// 使用分组标题和一组列创建跨列表头。
    pub fn new(title: impl Into<String>, columns: Vec<TableColumn>) -> Self {
        Self {
            title: Some(title.into()),
            columns,
        }
    }

    /// 将单列包装为不参与分组的列声明。
    pub fn column(column: TableColumn) -> Self {
        Self {
            title: None,
            columns: vec![column],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnGroupRange {
    pub(crate) title: Option<String>,
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// 表格行数据。
pub type TableRow = Vec<String>;

/// typed 行数据的一列文本投影。
pub(crate) type TypedCellViewRenderer<R> = Box<dyn Fn(&R) -> crate::ui::view::ViewNode>;

/// 将泛型行数据投影为表格列文本与可选自定义视图的绑定。
pub struct TableDataColumn<R> {
    pub(crate) column: TableColumn,
    pub(crate) accessor: Box<dyn Fn(&R) -> String>,
    pub(crate) renderer: Option<TypedCellViewRenderer<R>>,
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
    /// 两行数据生成了相同的稳定行键。
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
    pub(crate) table: Table,
    pub(crate) rows: Vec<R>,
    pub(crate) row_keys: Vec<String>,
    pub(crate) columns: Vec<TableDataColumn<R>>,
    pub(crate) empty_renderer: Option<TableEmptyRenderer>,
}

/// 扩展行视图工厂；按当前行数据构建普通 View 子树。
pub type ExpandRenderer = Box<dyn Fn(&TableRow) -> crate::ui::view::ViewNode>;

/// 空态 View 工厂；仅在 build 边界消费，接收当前列数，不进入 Table 组件。
pub type TableEmptyRenderer = Box<dyn Fn(usize) -> crate::ui::view::ViewNode>;

/// Type-erased typed-row cell factory kept in the render-handler sidecar.
pub(crate) type TableCellRenderer = Box<dyn Fn(usize, usize) -> crate::ui::view::ViewNode>;

/// 空行且非加载时解析空态 View：表级 `.empty` 优先于 ConfigProvider。
pub(crate) fn resolve_table_empty_view(
    table: &Table,
    empty_renderer: Option<&TableEmptyRenderer>,
) -> Option<crate::ui::view::ViewNode> {
    if !table.rows.is_empty() || table.loading {
        return None;
    }
    if let Some(renderer) = empty_renderer {
        return Some(renderer(table.columns.len()));
    }
    crate::ui::widget_runtime::config::render_empty_for::<Table>()
}

/// 变更事件。
#[derive(Debug, Clone)]
pub struct TableChange {
    /// 本次变更涉及的列索引；非排序变更时为空。
    pub sort_column: Option<usize>,
    /// 本次变更后的排序方向。
    pub sort_direction: SortDirection,
    /// 本次变更后的零基页索引。
    pub page: usize,
    /// 每页允许展示的行数。
    pub page_size: usize,
}

/// 远程分页配置。回调由表格在页码变化时调用，数据本身仍由应用层维护。
pub struct TablePagination<F = fn(usize)> {
    /// 当前远程页码，从一开始计数。
    pub current: usize,
    /// 远程数据源中的总行数。
    pub total: usize,
    /// 每个远程页面包含的行数。
    pub page_size: usize,
    /// 用户切换页码时接收一基目标页码的回调。
    pub on_change: F,
}

pub(crate) type TablePaginationCallback = Rc<dyn Fn(usize)>;
pub(crate) type TableRowClickCallback = Rc<dyn Fn(&TableRow, usize)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TablePointerAction {
    ToggleAll,
    ToggleRow(usize),
    SortColumn(usize),
    ToggleExpand(usize),
    SelectRow(usize),
    ChangePage(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableResizeDrag {
    pub(crate) column: usize,
    pub(crate) start_x: f32,
    pub(crate) start_width: f32,
}

/// 无显式 row key 时按行号生成隐式 key。
pub(crate) fn implicit_row_keys(row_count: usize) -> Vec<String> {
    (0..row_count).map(|index| index.to_string()).collect()
}
