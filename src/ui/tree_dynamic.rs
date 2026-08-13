use crate::core::ComponentId;
use crate::ui::adapter::ViewAdapter;
use crate::ui::component::provider_context::with_provider_context;
use crate::ui::component::widget::tree_core::WidgetTree;
use crate::ui::component::widget::WidgetCore;
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
// 表格 capability 启用时才引入动态扩展行与单元格目标类型。
#[cfg(feature = "table")]
// 该类型只服务同步门控的表格刷新入口。
use crate::ui::widgets::display::table::Table;
use crate::ui::widgets::display::{Calendar, Collapse, Image};
use crate::ui::widgets::input::Select;

impl WidgetTree {
    pub(crate) fn is_calendar_cell_component(&self, id: ComponentId) -> bool {
        self.get(id).is_some_and(|node| {
            node.component()
                .as_any()
                .downcast_ref::<Calendar>()
                .is_some_and(Calendar::owns_custom_cell_children)
        })
    }

    pub(crate) fn refresh_calendar_cell_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的动态单元格 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        let refresh = self.get(id).and_then(|node| {
            let provider_context = node.provider_context().clone();
            with_provider_context(&provider_context, || {
                node.component()
                    .as_any()
                    .downcast_ref::<Calendar>()?
                    .cell_views_for_refresh(node.children().len())
            })
        });
        let Some((views, entries)) = refresh else {
            return false;
        };

        let changed = ViewAdapter::reconcile_dynamic_children(self, id, views);
        if let Some(calendar) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Calendar>())
        {
            calendar.mark_cells_materialized(entries);
        }
        changed
    }

    pub(crate) fn is_collapse_content_component(&self, id: ComponentId) -> bool {
        self.get(id)
            .is_some_and(|node| node.component().as_any().is::<Collapse>())
    }

    pub(crate) fn refresh_collapse_content_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的折叠内容 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        let refresh = self.get(id).and_then(|node| {
            node.component()
                .as_any()
                .downcast_ref::<Collapse>()?
                .content_views_for_refresh(node.children().len())
        });
        let Some((views, entries)) = refresh else {
            return false;
        };

        ViewAdapter::reconcile_dynamic_children(self, id, views);
        if let Some(collapse) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Collapse>())
        {
            collapse.mark_content_materialized(entries);
        }
        true
    }

    pub(crate) fn refresh_image_error_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的错误视图 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        let error_view = self.get(id).and_then(|node| {
            node.component()
                .as_any()
                .downcast_ref::<Image>()?
                .error_view_for_refresh(node.children().len())
        });
        let Some(error_view) = error_view else {
            return false;
        };

        // 在事务发布线前完成纯声明展开，panic 时既有运行时树仍可使用。
        let error_node = ViewAdapter::expand(error_view);
        // 直接挂载路径也建立协调事务，确保已发布 panic 会切换 fail-stop。
        self.with_component_state_transaction(Vec::new(), |tree| {
            // 错误子树即将改写运行时结构，进入不可逆发布区。
            tree.mark_coordination_publish_started();
            // 挂载已经完成纯展开的错误子树。
            tree.build_child_node(id, error_node);
        });
        if let Some(image) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Image>())
        {
            image.mark_error_view_materialized();
        }
        true
    }

    // 表格 capability 启用时才记录扩展行子树物化状态。
    #[cfg(feature = "table")]
    pub(crate) fn mark_table_expand_materialized(&self, id: ComponentId) {
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Table>())
        {
            table.mark_expanded_child_materialized();
        }
    }

    // 表格 capability 启用时才刷新扩展行动态子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_expand_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的表格扩展行 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        if !self.render_handler_table.contains_table_expand(id) {
            return false;
        }

        let Some((expanded_row, materialized_row, child_count, row)) =
            self.get(id).and_then(|node| {
                let table = node.component().as_any().downcast_ref::<Table>()?;
                let expanded_row = table.expanded_row();
                let row = expanded_row.and_then(|index| table.rows.get(index).cloned());
                Some((
                    expanded_row,
                    table.expanded_child_row(),
                    node.children().len(),
                    row,
                ))
            })
        else {
            return false;
        };
        let expected_children = usize::from(row.is_some());
        if expanded_row == materialized_row && child_count == expected_children {
            return false;
        }

        // 捕获本轮扩展行声明子树并保留其 receipt 到动态协调事务。
        let children = row
            .as_ref()
            .and_then(|row| {
                // 构建独立状态所有权的声明节点，避免缺少 row 命名空间时跨行复用。
                self.render_handler_table.render_table_expand_view(id, row)
            })
            .into_iter()
            .collect();
        // 使用动态协调事务挂载扩展行，成功后才接纳其组件状态 journal。
        crate::ui::adapter::ViewAdapter::reconcile_dynamic_children(self, id, children);
        self.mark_table_expand_materialized(id);
        true
    }

    pub(crate) fn refresh_virtual_scroll_component(
        &mut self,
        id: ComponentId,
        viewport_height: Option<f32>,
    ) -> bool {
        // 失败或关闭树不得再调用应用提供的虚拟列表 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        // 正在离场或已经销毁的宿主不能再执行应用 renderer。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 保留现有墓碑与输出直到真正 remove，不创建新物化行。
            return false;
        }
        if !self.render_handler_table.contains_virtual_scroll_item(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let scroll = node.component().as_any().downcast_ref::<VirtualScroll>()?;
            let height = viewport_height
                .filter(|height| *height > 0.0)
                .unwrap_or_else(|| scroll.configured_viewport_height());
            let range = scroll.scroll_range(height);
            // 离场节点仍留在父子链供动画绘制，但不属于当前活动物化窗口。
            let mounted_children = node
                // 只检查 VirtualScroll 的直接物化行。
                .children()
                // 逐项借用轻量运行时身份。
                .iter()
                // 排除离场行及其任何已进入离场阶段的祖先子树。
                .filter(|child_id| !self.is_pending_removal_subtree(**child_id))
                // 得到真正参与本轮窗口协调的活动行数。
                .count();
            Some((
                range,
                // 物化判定不能让尚未完成动画的墓碑触发重复 renderer。
                scroll.needs_child_refresh(height, mounted_children),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        // 在借用 renderer sidecar 前签发固定 store 与 owner 的窄动态捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 使用已验证能力逐项捕获当前物化窗口的完整运行时输出。
        let Some(children) = self
            // handler 缺失不是合法空窗口，不能据此删除现有物化行。
            .render_handler_table
            // 批量捕获当前范围内的完整声明输出。
            .render_virtual_scroll_items(&capture_context, range.0, range.1)
        else {
            // 保留旧 children 与 materialized range，等待声明协调修复 sidecar。
            return false;
        };
        // 按业务 key 或绝对索引后备 key 协调窗口，保留重叠行身份与状态。
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        // 成功协调后记录当前物化范围，避免同一窗口重复构建。
        if let Some(scroll) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<VirtualScroll>())
        {
            scroll.mark_children_materialized(range);
        }
        // 只有子结构或顺序变化时向调用方报告结构更新。
        changed
    }

    // 表格 capability 启用时才刷新泛型单元格动态子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_cell_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的表格单元格 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        if !self.render_handler_table.contains_table_cells(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let table = node.component().as_any().downcast_ref::<Table>()?;
            let range = table.cell_view_range_for_frame(node.frame());
            Some((
                range,
                table.needs_cell_refresh(range, node.children().len()),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let Some((row_keys, view_columns)) = self.get(id).and_then(|node| {
            let table = node.component().as_any().downcast_ref::<Table>()?;
            Some((table.row_keys().to_vec(), table.view_columns().to_vec()))
        }) else {
            return false;
        };

        let children = self
            .render_handler_table
            .render_table_cells(id, range, &row_keys, &view_columns)
            .unwrap_or_default();
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Table>())
        {
            table.mark_cells_materialized(range);
        }
        changed
    }

    pub(crate) fn refresh_select_option_component(&mut self, id: ComponentId) -> bool {
        // 失败或关闭树不得再调用应用提供的选择项 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        if !self.render_handler_table.contains_select_options(id) {
            return false;
        }

        let Some((indices, labels, needs_refresh)) = self.get(id).and_then(|node| {
            let select = node.component().as_any().downcast_ref::<Select>()?;
            let indices = select.custom_option_indices();
            let labels = select.custom_option_labels(&indices);
            let needs_refresh = select.needs_custom_option_refresh(&indices, node.children().len());
            Some((indices, labels, needs_refresh))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let children = self
            .render_handler_table
            .render_select_options(id, &indices, &labels)
            .unwrap_or_default();
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Select>())
        {
            select.mark_custom_options_materialized(indices);
        }
        changed
    }

    pub(crate) fn invalidate_select_option_component(&self, id: ComponentId) {
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Select>())
        {
            select.invalidate_custom_option_materialization();
        }
    }

    // 表格 capability 启用时才查询扩展行 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn has_table_expand_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_table_expand(id)
    }

    // 表格 capability 启用时才查询泛型单元格 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn has_table_cell_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_table_cells(id)
    }

    pub(crate) fn has_select_option_renderer(&self, id: ComponentId) -> bool {
        self.render_handler_table.contains_select_options(id)
    }

    // 供声明树协调器区分 VirtualScroll 动态子树与普通空子列表。
    pub(crate) fn has_virtual_scroll_renderer(&self, id: ComponentId) -> bool {
        // sidecar 中存在行 renderer 即表示子项由虚拟窗口专用入口拥有。
        self.render_handler_table.contains_virtual_scroll_item(id)
    }
}
