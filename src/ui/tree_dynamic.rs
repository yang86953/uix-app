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
        // 确认 owner 仍是当前树中未销毁的实际节点。
        let owner_is_live = self.get(id).is_some_and(|node| !node.destroyed());
        // 确认 owner 仍是 Calendar，保留 custom 切回 plain 时的旧子树清理机会。
        let owner_is_calendar = self.get(id).is_some_and(|node| {
            // 这里只验证组件类型；是否仍需日期格由刷新差异计算决定。
            node.component().as_any().is::<Calendar>()
        });
        // 确认 owner 及其祖先尚未进入延迟离场阶段。
        let owner_is_leaving = self.is_pending_removal_subtree(id);
        // 迟到事件、陈旧 generation 与离场节点都必须静默拒绝刷新。
        if !owner_is_live || !owner_is_calendar || owner_is_leaving {
            // 不签发动态捕获能力，也不调用任何应用日期格工厂。
            return false;
        }
        // 先为已验证的 Calendar owner 签发树私有动态捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        let refresh = self.get(id).and_then(|node| {
            let provider_context = node.provider_context().clone();
            with_provider_context(&provider_context, || {
                node.component()
                    .as_any()
                    .downcast_ref::<Calendar>()?
                    // 让日期格工厂在 owner、槽位和日期身份限定的捕获边界中执行。
                    .cell_views_for_refresh(&capture_context, node.children().len())
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

    // 只为当前树中活跃的 Image owner 签发错误 View 动态捕获能力。
    fn image_error_capture_context(
        &self,
        // 接收将执行错误工厂的运行时 Image 节点身份。
        id: ComponentId,
    ) -> Option<crate::ui::adapter::DynamicViewCaptureContext> {
        // 失败或关闭树不得再调用应用提供的错误视图 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return None;
        }
        // 确认 owner 仍是当前树中未销毁的实际节点。
        let owner_is_live = self.get(id).is_some_and(|node| !node.destroyed());
        // 确认 owner 仍是 Image，拒绝陈旧 id 对复用槽位的误投递。
        let owner_is_image = self
            // 只读取当前树的实际运行时组件类型。
            .get(id)
            // 确认组件仍保持 Image 身份。
            .is_some_and(|node| node.component().as_any().is::<Image>());
        // 确认 owner 及其祖先尚未进入延迟离场阶段。
        let owner_is_leaving = self.is_pending_removal_subtree(id);
        // 迟到事件、陈旧 generation、非 Image 与离场节点都必须静默拒绝刷新。
        if !owner_is_live || !owner_is_image || owner_is_leaving {
            // 不签发动态捕获能力，也不调用任何应用错误 View 工厂。
            return None;
        }
        // 为已验证的活跃 Image owner 签发树私有动态捕获能力。
        Some(ViewAdapter::dynamic_capture_context(self, id))
    }

    // 为父级声明协调捕获当前 Image 错误子树，保留同一失败实例的状态所有权。
    pub(crate) fn image_error_view_for_reconcile(
        &self,
        // 接收当前正在被父级协调的运行时节点身份。
        id: ComponentId,
    ) -> Option<crate::ui::view::ViewNode> {
        // 先取得只对当前活跃 Image owner 有效的树私有捕获能力。
        let capture_context = self.image_error_capture_context(id)?;
        // 在节点自己的 ProviderContext 中调用错误 View 工厂。
        self.get(id).and_then(|node| {
            // 克隆上下文以在节点借用以外安装正确的依赖作用域。
            let provider_context = node.provider_context().clone();
            // 在声明组件相同的 provider 可见范围内构建动态错误 View。
            with_provider_context(&provider_context, || {
                // 只允许已验证的 Image 使用本 owner 专属捕获能力。
                node.component()
                    // 不泄漏具体组件实现给树外部调用方。
                    .as_any()
                    // 已由签发前检查确认的转换仍使用可选路径抵御并发式重入。
                    .downcast_ref::<Image>()?
                    // 捕获完整运行时输出，供父级嵌套事务原子接纳。
                    .error_view_for_reconcile(&capture_context)
            })
        })
    }

    pub(crate) fn refresh_image_error_component(&mut self, id: ComponentId) -> bool {
        // 先取得只对当前活跃 Image owner 有效的树私有捕获能力。
        let capture_context = match self.image_error_capture_context(id) {
            // 活跃 owner 可以继续完成本轮错误 View 捕获。
            Some(capture_context) => capture_context,
            // 陈旧、非 Image、离场或停止树都必须安全拒绝。
            None => return false,
        };
        // 读取当前错误需求与同 key 直接子节点，运行时树是实际物化状态的权威来源。
        let Some((needs_error_view, error_child)) = self.get(id).and_then(|node| {
            // 再次确认组件仍为 Image，抵御工厂重入造成的陈旧身份。
            let image = node.component().as_any().downcast_ref::<Image>()?;
            // 只在 Image handler 与当前 Error 状态同时成立时保留错误子树。
            let needs_error_view = image.needs_error_view();
            // 在直接子节点中查找固定错误 key，包括尚未结束 leave 的节点。
            let error_child = node.children().iter().copied().find(|child_id| {
                // 只匹配仍可寻址且 key 精确相等的直接子节点。
                self.get(*child_id)
                    // 借用子节点的稳定运行时 key。
                    .and_then(|child| child.key())
                    // 与 Image 错误实例的唯一身份比较。
                    == Some(Image::ERROR_CHILD_KEY)
            });
            // 返回本轮窄生命周期决策所需的最小快照。
            Some((needs_error_view, error_child))
        }) else {
            // owner 在签发后失效时静默拒绝，不调用工厂也不改变其他节点。
            return false;
        };
        // Error 已消失或 handler 已关闭时只移除错误子树，不能重建已消费的 placeholder。
        if !needs_error_view {
            // 有实际错误子节点时让其进入事务化 leave 或立即 teardown。
            let changed = error_child.is_some_and(|child_id| {
                // 窄移除保持 placeholder 与其他 authored 子节点完全不变。
                ViewAdapter::remove_dynamic_child(self, id, child_id)
            });
            // 重新读取固定 key 子节点，只有立即 remove 后实际缺席才能清理物化标记。
            let error_child_still_exists = self.get(id).is_some_and(|node| {
                // pending leave 仍保留在直接子节点集合中，必须继续视为已物化。
                node.children().iter().copied().any(|child_id| {
                    // 只匹配仍可寻址且 key 精确相等的错误子树。
                    self.get(child_id).and_then(|child| child.key()) == Some(Image::ERROR_CHILD_KEY)
                })
            });
            // 无 leave 的立即 remove 已完成 teardown，此时才允许清理派生标记。
            if !error_child_still_exists {
                // 只更新仍属于当前 owner 的实际 Image 私有状态。
                if let Some(image) = self
                    // 读取 remove 后仍存活的父组件。
                    .get(id)
                    // 确认类型未因重入发生变化。
                    .and_then(|node| node.component().as_any().downcast_ref::<Image>())
                {
                    // 实际子节点缺席与 on_children_changed 的清理语义保持一致。
                    image.clear_error_view_materialized();
                }
            }
            // 返回本轮是否首次建立了移除工作。
            return changed;
        }
        // 同 key 错误子节点仍存在时复用它；若正在 leave，则事务化取消离场。
        if let Some(error_child) = error_child {
            // 只取消目标错误节点的 pending removal，不触碰 placeholder。
            let changed = ViewAdapter::cancel_dynamic_child_removal(self, id, error_child);
            // 恢复成功或原本活跃时都承认同 key 错误子树已经物化。
            if let Some(image) = self
                // 只读取仍属于当前 owner 的实际运行时组件。
                .get(id)
                // 确认类型仍是 Image 后更新其私有派生状态。
                .and_then(|node| node.component().as_any().downcast_ref::<Image>())
            {
                // 保持后续布局不会重复执行用户工厂或追加重复节点。
                image.mark_error_view_materialized();
            }
            // 只有取消离场时才报告结构生命周期发生变化。
            return changed;
        }
        // 在 Image 自己的 ProviderContext 中捕获首次错误子树。
        let error_view = self.get(id).and_then(|node| {
            // 克隆上下文以在节点借用以外安装正确的依赖作用域。
            let provider_context = node.provider_context().clone();
            // 在声明组件相同的 provider 可见范围内构建动态错误 View。
            with_provider_context(&provider_context, || {
                // 只允许当前 Image 在未物化时请求首次错误子树。
                node.component()
                    // 不泄漏具体组件实现给树外部调用方。
                    .as_any()
                    // 已由签发前检查确认的转换仍使用可选路径抵御并发式重入。
                    .downcast_ref::<Image>()?
                    // 捕获完整运行时输出与回执，供追加事务原子接纳。
                    .error_view_for_refresh(&capture_context, node.children().len())
            })
        });
        let Some(error_view) = error_view else {
            return false;
        };
        // 用回执感知追加事务发布首次错误子树，panic 后沿用 fail-stop 语义。
        if !ViewAdapter::append_dynamic_child(self, id, error_view) {
            // 追加入口在停止树时拒绝，不得伪称子树已物化。
            return false;
        }
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
