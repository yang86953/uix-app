//! Node-authored render and child-factory handlers kept outside widget storage.

use std::collections::HashMap;
// 表格 capability 启用时才引入批量身份预检集合。
#[cfg(feature = "table")]
use std::collections::HashSet;
use std::sync::Arc;

use crate::core::WidgetId;
// 引入由宿主树签发的窄动态捕获能力。
use crate::ui::adapter::DynamicViewCaptureContext;
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
    table_expand: HashMap<WidgetId, ExpandRenderer>,
    // 表格 capability 启用时才存储泛型单元格 renderer。
    #[cfg(feature = "table")]
    table_cells: HashMap<WidgetId, TableCellRenderer>,
    select_options: HashMap<WidgetId, SelectOptionRenderer>,
    virtual_scroll_item: HashMap<WidgetId, VirtualScrollRenderer>,
}

impl RenderHandlerTable {
    pub(crate) fn replace_widget(
        &mut self,
        widget: WidgetId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        self.clear_widget(widget);
        for handler in handlers {
            match handler {
                // 表格 capability 启用时才注册扩展行 renderer。
                #[cfg(feature = "table")]
                RenderHandlerRegistration::TableExpand(renderer) => {
                    self.table_expand.insert(widget, renderer);
                }
                // 表格 capability 启用时才注册泛型单元格 renderer。
                #[cfg(feature = "table")]
                RenderHandlerRegistration::TableCells(renderer) => {
                    self.table_cells.insert(widget, renderer);
                }
                RenderHandlerRegistration::SelectOptions(renderer) => {
                    self.select_options.insert(widget, renderer);
                }
                RenderHandlerRegistration::VirtualScrollItem(renderer) => {
                    self.virtual_scroll_item.insert(widget, renderer);
                }
            }
        }
    }

    // 表格 capability 启用时才编译扩展行声明构建入口。
    #[cfg(feature = "table")]
    pub(crate) fn render_table_expand_view(
        &self,
        // 接收由活跃 Table owner 签发的窄动态捕获能力。
        capture_context: &DynamicViewCaptureContext,
        // 接收 Table 数据快照中已经确定的稳定行业务键。
        row_key: &str,
        // 接收与稳定行业务键对应的当前行快照。
        row: &TableRow,
    ) -> Option<ViewNode> {
        // 查找拥有当前表格扩展行的渲染器。
        let renderer = self.table_expand.get(&capture_context.owner())?;
        // 用 UTF-8 字节长度和完整 row_key 构成无拼接歧义的稳定身份。
        let stable_key = format!("table-expand:{}:{row_key}", row_key.len());
        // 为 keyed reconcile 保留与私有状态命名空间相同的身份副本。
        let view_key = stable_key.clone();
        // 在所属 WidgetTree 的展开行槽位中捕获完整声明输出。
        let view = capture_context.capture(
            // 固定槽位隔离同一 Table 下的其他延迟 View 工厂。
            "table-expand",
            // 让稳定行业务身份同时拥有私有状态与结构节点身份。
            stable_key,
            // 在完整捕获边界中调用应用提供的展开行工厂。
            || renderer(row),
        );
        // renderer 自设根 key 会造成状态与结构双身份，必须显式拒绝。
        assert!(
            // 框架是展开行动态根身份的唯一权威。
            view.key.is_none(),
            // 提供不包含业务行键的稳定迁移诊断。
            "Table 展开行 renderer 不得直接设置根 key"
        );
        // 强制节点结构协调与树私有捕获使用同一稳定身份。
        Some(view.key(view_key))
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
        // 借用已注册单元格 renderer 的 sidecar，不取得宿主树所有权。
        &self,
        // 接收由活跃 DataTable owner 签发的窄动态捕获能力。
        capture_context: &DynamicViewCaptureContext,
        // 接收当前物化窗口的半开行范围。
        range: (usize, usize),
        // 接收由 DataTable 唯一性校验过的稳定行业务键。
        row_keys: &[String],
        // 接收逻辑列号，使同一行中的不同声明列拥有独立身份。
        view_columns: &[usize],
    ) -> Option<Vec<ViewNode>> {
        // 只读取已由当前树 owner 绑定的单元格 renderer。
        let renderer = self.table_cells.get(&capture_context.owner())?;
        // 先为所有待物化单元格计算与捕获命名空间完全相同的结构身份。
        let entries = (range.0..range.1)
            // 跳过在当前数据快照中已不存在的行，保持既有空缺行行为。
            .filter_map(|row| {
                // 读取对应的稳定行键，缺失时不调用任何用户 renderer。
                let row_key = row_keys.get(row)?;
                // 为行中的每个视图列预先生成身份与 renderer 参数。
                Some(
                    view_columns
                        .iter()
                        .enumerate()
                        .map(move |(renderer_index, &column)| {
                            // 用逻辑列号、UTF-8 字节长度和完整行键构成可逆且无拼接歧义的身份。
                            let stable_key =
                                format!("table-cell:{column}:{}:{row_key}", row_key.len());
                            // 保留 renderer 的当前行与视图列位置，同时固定业务身份。
                            (row, renderer_index, stable_key)
                        }),
                )
            })
            // 展开为本轮全部单元格的扁平批次。
            .flatten()
            // 在任何用户 renderer 执行前保存完整预检结果。
            .collect::<Vec<_>>();
        // 建立本批次单元格身份集合以拒绝破坏 state 与 keyed reconcile 的冲突。
        let mut unique_keys = HashSet::with_capacity(entries.len());
        // 逐项验证预先生成的稳定身份没有重复。
        for (_, _, stable_key) in &entries {
            // 重复身份会令两个单元格错误共享私有状态与结构节点。
            assert!(
                // 只有首次出现的稳定键可进入后续用户 renderer 捕获阶段。
                unique_keys.insert(stable_key.clone()),
                // 不回显业务行键，避免诊断泄露应用数据。
                "DataTable 单元格 renderer 返回了重复稳定身份"
            );
        }
        // 预分配完整批次容量，避免捕获事务途中改变集合大小语义。
        let mut cells = Vec::with_capacity(
            // 每个预检条目恰好生成一个受协调的单元格根。
            entries.len(),
        );
        // 仅在全量身份和冲突预检通过后依次调用应用 renderer。
        for (row, renderer_index, stable_key) in entries {
            // 为 keyed reconcile 保留与动态 state namespace 相同的键副本。
            let view_key = stable_key.clone();
            // 在已验证的树私有命名空间中捕获完整 State、Effect、动画与 receipt。
            let cell = capture_context.capture(
                // 固定槽位隔离同一 DataTable 的单元格与其他延迟工厂。
                "table-cell",
                // 让业务稳定身份同时拥有状态命名空间与节点协调身份。
                stable_key,
                // 只在所有身份预检完成后调用应用提供的单元格 renderer。
                || renderer(row, renderer_index),
            );
            // 用户 renderer 自设根 key 会造成状态与结构双身份，必须显式拒绝。
            assert!(
                // 框架是单元格动态根身份的唯一权威。
                cell.key.is_none(),
                // 指向由 DataTable 提供的 row_key 与逻辑列稳定身份契约。
                "DataTable 单元格 renderer 不得直接设置根 key"
            );
            // 强制节点结构协调与树私有捕获使用同一稳定身份。
            cells.push(cell.key(view_key));
        }
        // 将已完整捕获但尚未协调的单元格批次交回宿主树事务。
        Some(cells)
    }

    pub(crate) fn render_select_options(
        // 接收由活跃 Select owner 签发的窄动态捕获能力。
        &self,
        // 固定状态存储与宿主身份，renderer 不接触 WidgetTree 所有权细节。
        capture_context: &DynamicViewCaptureContext,
        // 接收当前实际物化的绝对选项索引。
        indices: &[usize],
        // 接收与绝对索引一一对应的显示文案。
        labels: &[String],
        // 返回完整捕获但尚未协调的自定义选项声明节点。
    ) -> Option<Vec<ViewNode>> {
        // 只读取签发能力所属 Select 的 renderer sidecar。
        let renderer = self.select_options.get(&capture_context.owner())?;
        // 索引和文案必须来自同一 Select 快照，长度不一致会破坏身份配对。
        assert_eq!(
            // 验证每个 renderer 输入都有唯一绝对索引。
            indices.len(),
            // 验证每个绝对索引都有对应显示文案。
            labels.len(),
            // 提供不包含应用数据的稳定诊断。
            "Select 自定义选项索引与文案数量不一致"
        );
        // 逐项捕获当前可见选项的完整运行时输出。
        Some(
            indices
                .iter()
                // 保持绝对索引与显示文案的声明顺序一致。
                .zip(labels)
                // 为每个选项建立独立且可协调的树级动态实例。
                .map(|(index, label)| {
                    // 保持既有按绝对选项索引拥有身份的公开行为。
                    let stable_key = format!("select-option:{index}");
                    // 为 keyed reconcile 保留与状态命名空间相同的身份副本。
                    let view_key = stable_key.clone();
                    // 在所属 WidgetTree 的 Select 槽位中捕获完整声明输出。
                    let view = capture_context.capture(
                        // 固定槽位隔离同一宿主下的其他延迟 View 工厂。
                        "select-option",
                        // 绝对选项索引同时拥有私有状态与结构节点身份。
                        stable_key,
                        // 只在完整捕获边界内执行应用 renderer。
                        || renderer(label),
                    );
                    // 应用 renderer 自设根 key 会制造状态与结构双身份。
                    assert!(
                        // 框架是 Select 动态选项根身份的唯一权威。
                        view.key.is_none(),
                        // 明确要求调用方不要绕过绝对索引身份契约。
                        "Select option renderer 不得直接设置根 key"
                    );
                    // 强制结构协调与私有状态使用同一稳定身份。
                    view.key(view_key)
                })
                // 汇总当前可见选项供一次动态事务协调。
                .collect(),
        )
    }

    pub(crate) fn clear_widget(&mut self, widget: WidgetId) {
        // 表格 capability 启用时才清理扩展行 renderer。
        #[cfg(feature = "table")]
        if !self.table_expand.is_empty() {
            self.table_expand.remove(&widget);
        }
        // 表格 capability 启用时才清理泛型单元格 renderer。
        #[cfg(feature = "table")]
        if !self.table_cells.is_empty() {
            self.table_cells.remove(&widget);
        }
        // 全局空 sidecar 不可能持有当前节点，避免为空删除重复哈希。
        if !self.select_options.is_empty() {
            self.select_options.remove(&widget);
        }
        if !self.virtual_scroll_item.is_empty() {
            self.virtual_scroll_item.remove(&widget);
        }
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
    pub(crate) fn contains_table_expand(&self, widget: WidgetId) -> bool {
        // 空 sidecar 不可能拥有节点，避免为普通节点计算完整 WidgetId 哈希。
        !self.table_expand.is_empty() && self.table_expand.contains_key(&widget)
    }

    pub(crate) fn contains_virtual_scroll_item(&self, widget: WidgetId) -> bool {
        // 绝大多数节点没有虚拟滚动 renderer，空表直接返回缺席。
        !self.virtual_scroll_item.is_empty() && self.virtual_scroll_item.contains_key(&widget)
    }

    // 表格 capability 启用时才查询泛型单元格 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn contains_table_cells(&self, widget: WidgetId) -> bool {
        // 空 sidecar 不可能拥有节点，避免为普通节点计算完整 WidgetId 哈希。
        !self.table_cells.is_empty() && self.table_cells.contains_key(&widget)
    }

    pub(crate) fn contains_select_options(&self, widget: WidgetId) -> bool {
        // 绝大多数节点没有自定义选项 renderer，空表直接返回缺席。
        !self.select_options.is_empty() && self.select_options.contains_key(&widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_render_handler_sidecars_report_every_renderer_absent() {
        let table = RenderHandlerTable::default();
        let widget = WidgetId::new(7);

        assert!(!table.contains_virtual_scroll_item(widget));
        assert!(!table.contains_select_options(widget));
        #[cfg(feature = "table")]
        {
            assert!(!table.contains_table_expand(widget));
            assert!(!table.contains_table_cells(widget));
        }
    }
}

// ── 空态渲染回调（System 私有边界）───────────────────────────────
//
// `EmptyRenderer` 由 WidgetConfig（widget）持有、widgets 消费、
// 用户闭包产出 ViewNode（view）：跨 Module 契约归本边界（SMC-04）。

use crate::ui::view::View;
use crate::ui::widget_runtime::config::{WidgetConfig, use_config};
use crate::ui::widget_runtime::traits::Widget;

/// 标识正在请求空态 View 的数据组件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyContext {
    widget_name: &'static str,
}

impl EmptyContext {
    fn of<T: Widget>() -> Self {
        let full_name = std::any::type_name::<T>();
        Self {
            widget_name: full_name.rsplit("::").next().map_or(full_name, |name| name),
        }
    }

    /// 返回请求空态视图的组件短类型名。
    pub fn widget_name(self) -> &'static str {
        self.widget_name
    }
}

/// 由 `WidgetConfig` 持有的可克隆空态 View factory。
#[derive(Clone)]
pub struct EmptyRenderer {
    renderer: Arc<dyn Fn(EmptyContext) -> ViewNode + Send + Sync>,
}

impl EmptyRenderer {
    /// 使用将空态上下文转换为视图的工厂创建渲染器。
    pub fn new<F, V>(renderer: F) -> Self
    where
        F: Fn(EmptyContext) -> V + Send + Sync + 'static,
        V: View,
    {
        Self {
            renderer: Arc::new(move |context| renderer(context).build()),
        }
    }

    /// 为指定组件类型构建一棵空态视图节点树。
    pub fn render<T: Widget>(&self) -> ViewNode {
        (self.renderer)(EmptyContext::of::<T>())
    }

    pub(crate) fn is_same_renderer(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.renderer, &other.renderer)
    }
}

/// 为指定组件类型构建当前配置的空态 View。
pub fn render_empty_for<T: Widget>() -> Option<ViewNode> {
    use_config()
        .empty_renderer
        .map(|renderer| renderer.render::<T>())
}

impl WidgetConfig {
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
