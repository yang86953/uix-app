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
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        changed
    }

    pub(crate) fn is_collapse_content_component(&self, id: ComponentId) -> bool {
        self.get(id)
            .is_some_and(|node| node.component().as_any().is::<Collapse>())
    }

    pub(crate) fn refresh_collapse_content_component(&mut self, id: ComponentId) -> bool {
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
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        true
    }

    pub(crate) fn refresh_image_error_component(&mut self, id: ComponentId) -> bool {
        let error_view = self.get(id).and_then(|node| {
            node.component()
                .as_any()
                .downcast_ref::<Image>()?
                .error_view_for_refresh(node.children().len())
        });
        let Some(error_view) = error_view else {
            return false;
        };

        self.build_child_node(id, ViewAdapter::expand(error_view));
        if let Some(image) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<Image>())
        {
            image.mark_error_view_materialized();
        }
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        true
    }

    // 表格 capability 启用时才构建当前扩展行 View。
    #[cfg(feature = "table")]
    pub(crate) fn table_expand_view(&self, id: ComponentId) -> Option<crate::ui::view::ViewNode> {
        let table = self.get(id)?.component().as_any().downcast_ref::<Table>()?;
        let row = table.rows.get(table.expanded_row()?)?;
        self.render_handler_table.render_table_expand_view(id, row)
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

        let children = row
            .as_ref()
            .and_then(|row| {
                self.render_handler_table
                    .render_table_expand_widget(id, row)
            })
            .into_iter()
            .collect();
        self.set_children(id, children);
        self.mark_table_expand_materialized(id);
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        true
    }

    pub(crate) fn refresh_virtual_scroll_component(
        &mut self,
        id: ComponentId,
        viewport_height: Option<f32>,
    ) -> bool {
        if !self.render_handler_table.contains_virtual_scroll_item(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let scroll = node.component().as_any().downcast_ref::<VirtualScroll>()?;
            let height = viewport_height
                .filter(|height| *height > 0.0)
                .unwrap_or_else(|| scroll.configured_viewport_height());
            let range = scroll.scroll_range(height);
            Some((
                range,
                scroll.needs_child_refresh(height, node.children().len()),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let children = self
            .render_handler_table
            .render_virtual_scroll_items(id, range.0, range.1)
            .unwrap_or_default();
        // 按业务 key 或绝对索引后备 key 协调窗口，保留重叠行身份与状态。
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        // 成功协调后记录当前物化范围，避免同一窗口重复构建。
        if let Some(scroll) = self
            .get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<VirtualScroll>())
        {
            scroll.mark_children_materialized(range);
        }
        // 绑定新进入窗口行捕获的响应式状态。
        self.bind_orphan_pending_states();
        // 绑定新进入窗口行捕获的副作用。
        self.bind_pending_effects();
        // 只有子结构或顺序变化时向调用方报告结构更新。
        changed
    }

    // 表格 capability 启用时才刷新泛型单元格动态子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_cell_component(&mut self, id: ComponentId) -> bool {
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
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
        changed
    }

    pub(crate) fn refresh_select_option_component(&mut self, id: ComponentId) -> bool {
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
        self.bind_orphan_pending_states();
        self.bind_pending_effects();
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
