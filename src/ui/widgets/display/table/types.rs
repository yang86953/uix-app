//! Table 公开类型与常量 — table 子模块。
//!
//! 列配置、排序/分页契约、数据表格与回调类型；不包含任何组件实现。
//! [`Table`]（component! 宏）定义在父模块，[`TableBuilder`] 在 [`super::builder`]。

use std::error::Error;
use std::fmt;
use std::rc::Rc;

use super::Table;

pub(crate) const COLUMN_RESIZE_HANDLE_HALF_WIDTH: f32 = 4.0;
pub(crate) const MIN_RESIZABLE_COLUMN_WIDTH: f32 = 32.0;
pub(crate) const TABLE_PAGINATION_HEIGHT: f32 = 40.0;
pub(crate) const TABLE_PAGINATION_ITEM_SIZE: f32 = 28.0;
pub(crate) const TABLE_PAGINATION_GAP: f32 = 4.0;
pub(crate) const TABLE_PAGINATION_LABEL_WIDTH: f32 = 80.0;
pub(crate) const TABLE_PAGINATION_INSET: f32 = 8.0;

pub(crate) fn finite_nonnegative(value: f32) -> f32 {
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
pub(crate) struct ColumnGroupRange {
    pub(crate) title: Option<String>,
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// 表格行数据。
pub type TableRow = Vec<String>;

/// typed 行数据的一列文本投影。
pub(crate) type TypedCellViewRenderer<R> = Box<dyn Fn(&R) -> crate::ui::view::ViewNode>;

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
