//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::ComponentId;
use crate::ui::adapter::ViewAdapter;
use crate::ui::component::widget::WidgetNode;
use crate::ui::view::ViewNode;
use crate::ui::virtualization::virtual_scroll::VirtualScrollRenderer;
use crate::ui::widgets::display::table::{ExpandRenderer, TableCellRenderer, TableRow};
use crate::ui::widgets::input::select::SelectOptionRenderer;

pub(crate) enum RenderHandlerRegistration {
    TableExpand(ExpandRenderer),
    TableCells(TableCellRenderer),
    SelectOptions(SelectOptionRenderer),
    VirtualScrollItem(VirtualScrollRenderer),
}

#[derive(Default)]
pub(crate) struct RenderHandlerTable {
    table_expand: HashMap<ComponentId, ExpandRenderer>,
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
                RenderHandlerRegistration::TableExpand(renderer) => {
                    self.table_expand.insert(component, renderer);
                }
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

    pub(crate) fn render_table_expand_view(
        &self,
        component: ComponentId,
        row: &TableRow,
    ) -> Option<ViewNode> {
        let renderer = self.table_expand.get(&component)?;
        Some(ViewAdapter::capture_root(|| renderer(row)))
    }

    pub(crate) fn render_table_expand_widget(
        &self,
        component: ComponentId,
        row: &TableRow,
    ) -> Option<WidgetNode> {
        self.render_table_expand_view(component, row)
            .map(ViewAdapter::expand)
    }

    pub(crate) fn render_virtual_scroll_items(
        &mut self,
        component: ComponentId,
        start: usize,
        end: usize,
    ) -> Option<Vec<WidgetNode>> {
        let renderer = self.virtual_scroll_item.get_mut(&component)?;
        Some(
            (start..end)
                .map(|index| ViewAdapter::capture_root(|| renderer(index)))
                .map(ViewAdapter::expand)
                .collect(),
        )
    }

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
        self.table_expand.remove(&component);
        self.table_cells.remove(&component);
        self.select_options.remove(&component);
        self.virtual_scroll_item.remove(&component);
    }

    pub(crate) fn clear(&mut self) {
        self.table_expand.clear();
        self.table_cells.clear();
        self.select_options.clear();
        self.virtual_scroll_item.clear();
    }

    pub(crate) fn contains_table_expand(&self, component: ComponentId) -> bool {
        self.table_expand.contains_key(&component)
    }

    pub(crate) fn contains_virtual_scroll_item(&self, component: ComponentId) -> bool {
        self.virtual_scroll_item.contains_key(&component)
    }

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
