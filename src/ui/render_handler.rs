//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::ComponentId;
// 引入静态根捕获入口与由宿主树签发的窄动态捕获能力。
use crate::ui::adapter::{DynamicViewCaptureContext, ViewAdapter};
use crate::ui::view::ViewNode;
use crate::ui::virtualization::VirtualScrollRenderer;
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
        // 可变借用 renderer sidecar 以调用应用持有的 FnMut。
        &mut self,
        // 接收由宿主 WidgetTree 校验并签发的动态捕获能力。
        capture_context: &DynamicViewCaptureContext,
        // 接收当前物化窗口的绝对起始索引。
        start: usize,
        // 接收当前物化窗口的开区间结束索引。
        end: usize,
    ) -> Option<Vec<ViewNode>> {
        // 读取该虚拟滚动节点当前声明的行渲染器。
        let renderer = self.virtual_scroll_item.get_mut(&capture_context.owner())?;
        // 先计算整批稳定键，避免任何行捕获后才发现身份冲突。
        let keyed_indices = (start..end)
            // 每个绝对索引只调用一次应用键工厂。
            .map(|index| {
                // 类型化身份必须先规范化，再同时交给状态与节点所有权。
                (index, (renderer.key)(index).into_runtime_key())
            })
            // 保存本轮确定的索引与业务身份供后续捕获消费。
            .collect::<Vec<_>>();
        // 在执行任意行工厂前拒绝同一物化窗口内的重复稳定键。
        let mut unique_keys = std::collections::HashMap::with_capacity(keyed_indices.len());
        // 逐项验证键工厂满足当前窗口的唯一性前置条件。
        for (index, stable_key) in &keyed_indices {
            // 保存首次出现的绝对索引，不把可能敏感的业务标识写入 panic。
            if let Some(first_index) = unique_keys.insert(stable_key.clone(), *index) {
                // 重复键会同时破坏 keyed reconcile 与组件私有状态所有权。
                panic!(
                    // 只报告冲突位置，避免泄露应用业务标识。
                    "VirtualScroll renderer 的索引 {first_index} 与 {index} 返回了重复稳定键"
                );
            }
        }
        // 完成整批身份校验后才构建当前有界物化窗口中的 View。
        Some(
            keyed_indices
                // 消费已验证的索引与稳定键。
                .into_iter()
                // 每项进入相同 tree-owned capture 与 keyed reconcile 身份。
                .map(|(index, stable_key)| {
                    // 为声明节点保留与捕获命名空间相同的键副本。
                    let view_key = stable_key.clone();
                    // 在所属树的动态命名空间中构建当前业务项的声明行。
                    let view = capture_context
                        // 完整捕获 State、Effect、AnimatedSource 与状态回执。
                        .capture(
                            // 固定槽位把 VirtualScroll 行与同宿主其他延迟 renderer 隔离。
                            "virtual-scroll-item",
                            // 用键工厂输出统一拥有私有状态实例。
                            stable_key,
                            // 在完整捕获边界中调用应用行工厂。
                            || (renderer.item)(index),
                        );
                    // 普通 render 中返回自定义根 key 会形成双身份，必须显式迁移 API。
                    assert!(
                        // renderer 只能返回无 key 行根，由框架设置权威稳定身份。
                        view.key.is_none(),
                        // 指向唯一支持业务稳定键的公开入口。
                        "VirtualScroll 行 renderer 不得直接设置根 key；请改用 render_keyed"
                    );
                    // 强制节点协调 key 与动态状态命名空间完全一致。
                    view.key(view_key)
                })
                // 返回尚未展开的声明节点给动态协调器。
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
