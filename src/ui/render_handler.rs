//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::ComponentId;
use crate::ui::adapter::ViewAdapter;
use crate::ui::view::ViewNode;
use crate::ui::virtualization::virtual_scroll::VirtualScrollRenderer;
// 表格 capability 启用时才引入扩展行、自定义单元格与行数据类型。
#[cfg(feature = "table")]
// 这些类型只服务表格动态渲染 sidecar。
use crate::ui::widgets::display::table::{ExpandRenderer, TableCellRenderer, TableRow};
use crate::ui::widgets::input::select::SelectOptionRenderer;

pub(crate) enum RenderHandlerRegistration {
    // 表格 capability 启用时才接受扩展行 renderer。
    #[cfg(feature = "table")]
    TableExpand(ExpandRenderer),
    // 表格 capability 启用时才接受泛型单元格 renderer。
    #[cfg(feature = "table")]
    TableCells(TableCellRenderer),
    SelectOptions(SelectOptionRenderer),
    VirtualScrollItem(VirtualScrollRenderer),
}

#[derive(Default)]
pub(crate) struct RenderHandlerTable {
    // 表格 capability 启用时才存储扩展行 renderer。
    #[cfg(feature = "table")]
    table_expand: HashMap<ComponentId, ExpandRenderer>,
    // 表格 capability 启用时才存储泛型单元格 renderer。
    #[cfg(feature = "table")]
    table_cells: HashMap<ComponentId, TableCellRenderer>,
    select_options: HashMap<ComponentId, SelectOptionRenderer>,
    virtual_scroll_item: HashMap<ComponentId, VirtualScrollRenderer>,
}

impl RenderHandlerTable {
    pub(crate) fn replace_component(
        &mut self,
        component: ComponentId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        self.clear_component(component);
        for handler in handlers {
            match handler {
                // 表格 capability 启用时才注册扩展行 renderer。
                #[cfg(feature = "table")]
                RenderHandlerRegistration::TableExpand(renderer) => {
                    self.table_expand.insert(component, renderer);
                }
                // 表格 capability 启用时才注册泛型单元格 renderer。
                #[cfg(feature = "table")]
                RenderHandlerRegistration::TableCells(renderer) => {
                    self.table_cells.insert(component, renderer);
                }
                RenderHandlerRegistration::SelectOptions(renderer) => {
                    self.select_options.insert(component, renderer);
                }
                RenderHandlerRegistration::VirtualScrollItem(renderer) => {
                    self.virtual_scroll_item.insert(component, renderer);
                }
            }
        }
    }

    // 表格 capability 启用时才编译扩展行声明构建入口。
    #[cfg(feature = "table")]
    pub(crate) fn render_table_expand_view(
        &self,
        component: ComponentId,
        row: &TableRow,
    ) -> Option<ViewNode> {
        // 查找拥有当前表格扩展行的渲染器。
        let renderer = self.table_expand.get(&component)?;
        // 用独立 capture 避免尚未拥有稳定 row 命名空间的动态行跨行复用私有状态。
        Some(ViewAdapter::capture_root(|| renderer(row)))
    }

    pub(crate) fn render_virtual_scroll_items(
        &mut self,
        component: ComponentId,
        start: usize,
        end: usize,
    ) -> Option<Vec<ViewNode>> {
        // 读取该虚拟滚动节点当前声明的行渲染器。
        let renderer = self.virtual_scroll_item.get_mut(&component)?;
        // 只构建当前有界物化窗口中的 View。
        Some(
            (start..end)
                .map(|index| {
                    // 在捕获上下文中构建绝对索引对应的声明行。
                    let mut view = ViewAdapter::capture_root(|| renderer(index));
                    // 用户业务 key 优先；缺省时用绝对索引提供确定性身份。
                    if view.key.is_none() {
                        // 后备 key 让重叠物化窗口可复用同一行组件。
                        view = view.key(format!("virtual-scroll-item:{index}"));
                    }
                    // 保留 ViewNode 供动态协调器按 key 复用，而不提前展开。
                    view
                })
                .collect(),
        )
    }

    // 表格 capability 启用时才编译泛型单元格批量构建入口。
    #[cfg(feature = "table")]
    pub(crate) fn render_table_cells(
        &self,
        component: ComponentId,
        range: (usize, usize),
        row_keys: &[String],
        view_columns: &[usize],
    ) -> Option<Vec<ViewNode>> {
        let renderer = self.table_cells.get(&component)?;
        let mut cells = Vec::with_capacity(
            range
                .1
                .saturating_sub(range.0)
                .saturating_mul(view_columns.len()),
        );
        for row in range.0..range.1 {
            let Some(row_key) = row_keys.get(row) else {
                continue;
            };
            for (renderer_index, &column) in view_columns.iter().enumerate() {
                let cell = ViewAdapter::capture_root(|| renderer(row, renderer_index));
                cells.push(cell.key(format!("table-cell:{row_key}:{column}")));
            }
        }
        Some(cells)
    }

    pub(crate) fn render_select_options(
        &self,
        component: ComponentId,
        indices: &[usize],
        labels: &[String],
    ) -> Option<Vec<ViewNode>> {
        let renderer = self.select_options.get(&component)?;
        Some(
            labels
                .iter()
                .map(|label| ViewAdapter::capture_root(|| renderer(label)))
                .zip(indices)
                .map(|(view, index)| view.key(format!("select-option:{index}")))
                .collect(),
        )
    }

    pub(crate) fn clear_component(&mut self, component: ComponentId) {
        // 表格 capability 启用时才清理扩展行 renderer。
        #[cfg(feature = "table")]
        self.table_expand.remove(&component);
        // 表格 capability 启用时才清理泛型单元格 renderer。
        #[cfg(feature = "table")]
        self.table_cells.remove(&component);
        self.select_options.remove(&component);
        self.virtual_scroll_item.remove(&component);
    }

    pub(crate) fn clear(&mut self) {
        // 表格 capability 启用时才清空扩展行 renderer 表。
        #[cfg(feature = "table")]
        self.table_expand.clear();
        // 表格 capability 启用时才清空泛型单元格 renderer 表。
        #[cfg(feature = "table")]
        self.table_cells.clear();
        self.select_options.clear();
        self.virtual_scroll_item.clear();
    }

    // 表格 capability 启用时才查询扩展行 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn contains_table_expand(&self, component: ComponentId) -> bool {
        self.table_expand.contains_key(&component)
    }

    pub(crate) fn contains_virtual_scroll_item(&self, component: ComponentId) -> bool {
        self.virtual_scroll_item.contains_key(&component)
    }

    // 表格 capability 启用时才查询泛型单元格 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn contains_table_cells(&self, component: ComponentId) -> bool {
        self.table_cells.contains_key(&component)
    }

    pub(crate) fn contains_select_options(&self, component: ComponentId) -> bool {
        self.select_options.contains_key(&component)
    }
}

// ── 空态渲染回调（System 私有边界）───────────────────────────────
//
// `EmptyRenderer` 由 ComponentConfig（component）持有、widgets 消费、
// 用户闭包产出 ViewNode（view）：跨 Module 契约归本边界（SMC-04）。

use crate::ui::component::config::{use_config, ComponentConfig};
use crate::ui::component::traits::WidgetComponent;
use crate::ui::view::View;

/// 标识正在请求空态 View 的数据组件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyContext {
    component_name: &'static str,
}

impl EmptyContext {
    fn of<T: WidgetComponent>() -> Self {
        let full_name = std::any::type_name::<T>();
        Self {
            component_name: full_name.rsplit("::").next().map_or(full_name, |name| name),
        }
    }

    pub fn component_name(self) -> &'static str {
        self.component_name
    }
}

/// 由 `ComponentConfig` 持有的可克隆空态 View factory。
#[derive(Clone)]
pub struct EmptyRenderer {
    renderer: Arc<dyn Fn(EmptyContext) -> ViewNode + Send + Sync>,
}

impl EmptyRenderer {
    pub fn new<F, V>(renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        Self {
            renderer: Arc::new(move |context| renderer(context).build()),
        }
    }

    pub fn render<T: WidgetComponent>(&self) -> ViewNode {
        (self.renderer)(EmptyContext::of::<T>())
    }

    pub(crate) fn is_same_renderer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.renderer, &other.renderer)
    }
}

/// 为指定组件类型构建当前配置的空态 View。
pub fn render_empty_for<T: WidgetComponent>() -> Option<ViewNode> {
    use_config()
        .empty_renderer
        .map(|renderer| renderer.render::<T>())
}

impl ComponentConfig {
    /// 为数据组件设置空态 View factory（方法定义随跨 Module 契约归本边界）。
    pub fn render_empty<F, V>(mut self, renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        self.empty_renderer = Some(EmptyRenderer::new(renderer));
        self
    }
}
