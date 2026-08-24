use crate::core::WidgetId;
use crate::ui::adapter::ViewAdapter;
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
use crate::ui::widget_runtime::provider_context::with_provider_context;
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::ui::widget_runtime::widget::tree_core::WidgetTree;
// 表格 capability 启用时才引入动态扩展行与单元格目标类型。
#[cfg(feature = "table")]
// 该类型只服务同步门控的表格刷新入口。
use crate::ui::widgets::display::table::Table;
use crate::ui::widgets::display::{Calendar, Collapse, Image};
use crate::ui::widgets::input::Select;
// 引入 Transfer 以约束条目动态子树的唯一合法 owner 类型。
use crate::ui::widgets::Transfer;
// 把 Carousel 固定动态箭头协调拆到独立实现，保持树级生命周期入口内聚。
#[path = "carousel.rs"]
// 编译 Carousel owner、固定身份与捕获事务的专属扩展。
mod carousel;
// 导航 capability 启用时才引入 Anchor 动态容器 owner 类型。
#[cfg(feature = "navigation")]
use crate::ui::widgets::navigation::Anchor;

impl WidgetTree {
    // 判断当前节点是否仍是拥有动态条目 renderer 的 Transfer。
    pub(crate) fn is_transfer_item_widget(&self, id: WidgetId) -> bool {
        // 仅接受当前树中可寻址的实际 Transfer 节点。
        self.get(id)
            // 不把陈旧槽位或其他组件误判为条目动态 owner。
            .is_some_and(|node| node.widget().as_any().is::<Transfer>())
    }

    // 为当前树中活跃的 Transfer owner 捕获并协调全部自定义条目。
    pub(crate) fn refresh_transfer_item_widget(&mut self, id: WidgetId) -> bool {
        // 失败或关闭树不得再调用应用提供的条目 renderer。
        if !self.accepts_coordination_work() {
            // 保留既有运行时子树等待受控 teardown。
            return false;
        }
        // 已销毁、离场或陈旧 owner 不得签发动态捕获能力。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 离场墓碑继续由真实移除边界持有全部资源。
            return false;
        }
        // 固定入口只接受当前树中实际存在的 Transfer owner。
        if !self.is_transfer_item_widget(id) {
            // 非 Transfer 或陈旧 generation 不执行任何应用代码。
            return false;
        }
        // 为已经验证的活跃 Transfer owner 签发树私有捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 在 owner 自身 ProviderContext 中捕获本轮全部条目声明。
        let children = self.get(id).and_then(|node| {
            // 克隆 provider 上下文以缩短运行时节点借用。
            let provider_context = node.provider_context().clone();
            // 让条目 renderer 观察与宿主声明相同的 Provider 值。
            with_provider_context(&provider_context, || {
                // 仅允许已验证的实际 Transfer 执行条目工厂。
                node.widget()
                    // 不向 renderer 暴露 WidgetTree 或其他组件实现。
                    .as_any()
                    // 再次按具体类型收窄，抵御陈旧身份误投递。
                    .downcast_ref::<Transfer>()
                    // 捕获 State、Effect、AnimatedSource 与状态 receipt 的完整批次。
                    .map(|transfer| transfer.item_views_for_reconcile(&capture_context))
            })
        });
        // owner 在捕获前失效时静默拒绝，不删除任何既有子树。
        let Some(children) = children else {
            // 不把缺少合法 owner 解释为空条目集合。
            return false;
        };
        // 用单一树事务按稳定 key 原子协调全部动态条目。
        ViewAdapter::reconcile_dynamic_children(self, id, children)
    }

    // 判断当前节点是否仍是拥有专属动态容器的 Anchor。
    #[cfg(feature = "navigation")]
    pub(crate) fn is_anchor_container_widget(&self, id: WidgetId) -> bool {
        // 仅接受当前树中可寻址的实际 Anchor 节点。
        self.get(id)
            // 不把任意同槽位组件误判为动态容器 owner。
            .is_some_and(|node| node.widget().as_any().is::<Anchor>())
    }

    // 为当前树中活跃的 Anchor owner 捕获并物化首次动态内容容器。
    #[cfg(feature = "navigation")]
    pub(crate) fn refresh_anchor_container_widget(&mut self, id: WidgetId) -> bool {
        // 停止或失败树不得再调用应用容器工厂。
        if !self.accepts_coordination_work() {
            // 保留既有运行时子树，等待受控 teardown。
            return false;
        }
        // 已销毁或离场的 owner 不得重新签发动态捕获能力。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 陈旧 generation 与离场阶段均静默拒绝。
            return false;
        }
        // 固定入口只接受当前树中实际存在的 Anchor owner。
        if !self.is_anchor_container_widget(id) {
            // 非 Anchor 或陈旧 id 不能触发任何结构更新。
            return false;
        }
        // 先寻找包括 pending leave 在内的既有固定动态容器。
        let existing = self.get(id).and_then(|node| {
            // 只从 Anchor 的直接 children 判断当前容器物化状态。
            node.children().iter().copied().find(|child_id| {
                // 以框架唯一 key 匹配，不根据顺序或类型猜测。
                self.get(*child_id).and_then(|child| child.key())
                    == Some(Anchor::CONTAINER_CHILD_KEY)
            })
        });
        // 在任何用户工厂调用前读取 live Anchor 的容器需求。
        let needs_container = self
            // 读取已经通过 owner 类型检查的实际节点。
            .get(id)
            // 恢复具体 Anchor 以查询声明开关与工厂是否同时存在。
            .and_then(|node| node.widget().as_any().downcast_ref::<Anchor>())
            // 只有完整声明才需要保留或首次创建动态容器。
            .is_some_and(Anchor::needs_container_view);
        // 禁用或缺少工厂时只移除旧框架容器，不执行应用代码。
        if !needs_container {
            // 有旧容器才建立窄动态删除事务。
            return existing
                .is_some_and(|child_id| ViewAdapter::remove_dynamic_child(self, id, child_id));
        }
        // 已物化同 key 容器只需恢复可能中的离场，不重复执行工厂。
        if let Some(child_id) = existing {
            // 恢复既有状态、Effect 与动画所有权，避免空刷新替换已提交实例。
            return ViewAdapter::cancel_dynamic_child_removal(self, id, child_id);
        }
        // 为已经确认活跃的实际 Anchor owner 签发树私有捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 在 owner 自身 ProviderContext 中读取并捕获当前容器工厂。
        let container = self.get(id).and_then(|node| {
            // 克隆声明期上下文，避免工厂读取错误的 provider 作用域。
            let provider_context = node.provider_context().clone();
            // 在原 Anchor 的 provider 可见范围内执行用户工厂。
            with_provider_context(&provider_context, || {
                // 已验证类型仍使用可选 downcast 抵御同步重入。
                node.widget()
                    // 不向树外泄漏 Anchor 的具体实现。
                    .as_any()
                    // 只让当前 live Anchor 调用其私有工厂。
                    .downcast_ref::<Anchor>()?
                    // 捕获完整 View 输出及其延迟提交 receipt。
                    .container_view_for_reconcile(&capture_context)
            })
        });
        // 签发后若 owner 因重入失效则安全拒绝，不发布半捕获输出。
        let Some(container) = container else {
            // 捕获节点离开作用域时自动回滚未提交 receipt。
            return false;
        };
        // 通过 receipt 感知的追加事务物化首次完整动态子树。
        ViewAdapter::append_dynamic_child(self, id, container)
    }

    // 在父声明协调完成 live Anchor patch 后，将 authored children 与当前动态容器原子协调。
    #[cfg(feature = "navigation")]
    pub(crate) fn reconcile_anchor_container_widget(
        // 借用目标运行时树以执行同一嵌套事务。
        &mut self,
        // 接收已经完成 live Anchor 原位同步的真实组件身份。
        id: WidgetId,
        // 接收当前父声明提供的全部 authored 直接子节点。
        mut authored_children: Vec<crate::ui::view::ViewNode>,
        // 返回运行时直接子节点结构是否发生改变。
    ) -> bool {
        // 停止或失败树不能消费 authored 或动态声明输出。
        if !self.accepts_coordination_work() {
            // 声明节点离开作用域时自动回滚未提交 receipt。
            return false;
        }
        // 销毁、离场或陈旧 owner 不能重建动态容器。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 保留旧树，避免迟到协调破坏离场生命周期。
            return false;
        }
        // 重新确认当前槽位仍属于 live Anchor，防止错误状态命名空间接管。
        if !self.is_anchor_container_widget(id) {
            // 安全拒绝本轮过期父协调。
            return false;
        }
        // authored children 不得抢占框架固定 key，避免与动态容器身份混淆。
        assert!(
            // 仅检查直接子节点，因为 keyed 协调身份只在同一父级下生效。
            authored_children
                .iter()
                // 读取声明根的显式 key。
                .all(|child| child.key.as_deref() != Some(Anchor::CONTAINER_CHILD_KEY)),
            // 明确拒绝会让用户节点被误复用为框架容器的声明。
            "Anchor authored child 不得使用保留 key uix:anchor:container"
        );
        // 在 live Anchor 已同步的最新 provider 作用域内签发固定 owner 的捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 只从 live Anchor 读取刚完成 patch 的最新工厂，不借用旧声明 widget。
        let container = self.get(id).and_then(|node| {
            // 克隆 live 节点的 provider 上下文供工厂同步执行。
            let provider_context = node.provider_context().clone();
            // 让捕获沿用 Anchor 声明期可见的 provider 值。
            with_provider_context(&provider_context, || {
                // 使用可选 downcast 抵御用户工厂触发的同步重入。
                node.widget()
                    // 保持具体组件访问仅在树私有协调器内。
                    .as_any()
                    // 只调用 live Anchor 保存的最新容器工厂。
                    .downcast_ref::<Anchor>()?
                    // 捕获当前动态容器完整 State、Effect、动画和 receipt。
                    .container_view_for_reconcile(&capture_context)
            })
        });
        // 启用时将固定框架容器追加到 authored 声明，随后一次性 keyed 协调。
        if let Some(container) = container {
            // 容器固定排在 authored children 后，保持用户声明的相对顺序。
            authored_children.push(container);
        }
        // 禁用时不追加容器，协调器会只删除旧固定动态 child 并保留 authored children。
        ViewAdapter::reconcile_dynamic_children(self, id, authored_children)
    }

    pub(crate) fn is_calendar_cell_widget(&self, id: WidgetId) -> bool {
        self.get(id).is_some_and(|node| {
            node.widget()
                .as_any()
                .downcast_ref::<Calendar>()
                .is_some_and(Calendar::owns_custom_cell_children)
        })
    }

    pub(crate) fn refresh_calendar_cell_widget(&mut self, id: WidgetId) -> bool {
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
            node.widget().as_any().is::<Calendar>()
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
                node.widget()
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
            .and_then(|node| node.widget().as_any().downcast_ref::<Calendar>())
        {
            calendar.mark_cells_materialized(entries);
        }
        changed
    }

    pub(crate) fn is_collapse_content_widget(&self, id: WidgetId) -> bool {
        self.get(id)
            .is_some_and(|node| node.widget().as_any().is::<Collapse>())
    }

    pub(crate) fn refresh_collapse_content_widget(&mut self, id: WidgetId) -> bool {
        // 失败或关闭树不得再调用应用提供的折叠内容 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        let refresh = self.get(id).and_then(|node| {
            node.widget()
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
            .and_then(|node| node.widget().as_any().downcast_ref::<Collapse>())
        {
            collapse.mark_content_materialized(entries);
        }
        true
    }

    // 只为当前树中活跃的 Image owner 签发错误 View 动态捕获能力。
    fn image_error_capture_context(
        &self,
        // 接收将执行错误工厂的运行时 Image 节点身份。
        id: WidgetId,
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
            .is_some_and(|node| node.widget().as_any().is::<Image>());
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
        id: WidgetId,
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
                node.widget()
                    // 不泄漏具体组件实现给树外部调用方。
                    .as_any()
                    // 已由签发前检查确认的转换仍使用可选路径抵御并发式重入。
                    .downcast_ref::<Image>()?
                    // 捕获完整运行时输出，供父级嵌套事务原子接纳。
                    .error_view_for_reconcile(&capture_context)
            })
        })
    }

    pub(crate) fn refresh_image_error_widget(&mut self, id: WidgetId) -> bool {
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
            let image = node.widget().as_any().downcast_ref::<Image>()?;
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
                    .and_then(|node| node.widget().as_any().downcast_ref::<Image>())
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
                .and_then(|node| node.widget().as_any().downcast_ref::<Image>())
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
                node.widget()
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
            .and_then(|node| node.widget().as_any().downcast_ref::<Image>())
        {
            image.mark_error_view_materialized();
        }
        true
    }

    // 表格 capability 启用时才记录扩展行子树物化状态。
    #[cfg(feature = "table")]
    pub(crate) fn mark_table_expand_materialized(&self, id: WidgetId) {
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.widget().as_any().downcast_ref::<Table>())
        {
            table.mark_expanded_child_materialized();
        }
    }

    // 表格 capability 启用时才刷新扩展行动态子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_expand_widget(&mut self, id: WidgetId) -> bool {
        // 失败或关闭树不得再调用应用提供的表格扩展行 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        // 已销毁或正在离场的宿主不再签发动态捕获能力或执行应用 renderer。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 保留既有墓碑直到真实移除，不在离场阶段重建展开行子树。
            return false;
        }
        // 陈旧 id 或非 Table owner 不能借用同一 widget 槽位的 renderer。
        if !self
            // 只读取当前运行时节点的实际组件类型。
            .get(id)
            // 让 Table 是本刷新入口唯一允许的 owner 类型。
            .is_some_and(|node| node.widget().as_any().is::<Table>())
        {
            // 拒绝非 Table owner，避免状态写入错误的动态命名空间。
            return false;
        }
        if !self.render_handler_table.contains_table_expand(id) {
            return false;
        }

        let Some((expanded_row, materialized_row, child_count, expanded)) =
            self.get(id).and_then(|node| {
                let table = node.widget().as_any().downcast_ref::<Table>()?;
                let expanded_row = table.expanded_row();
                // 同时读取行快照与对应稳定键，缺少任一项时都不得执行用户 renderer。
                let expanded = expanded_row.and_then(|index| {
                    // 行数据与 row_key 必须来自同一 Table 数据快照。
                    table
                        // 读取当前展开行的数据快照。
                        .rows
                        // 越界展开索引视为没有可物化行。
                        .get(index)
                        // 克隆行数据以缩短 WidgetTree 的不可变借用。
                        .cloned()
                        // 只有同索引稳定 row_key 存在时才形成合法动态实例。
                        .zip(table.row_keys().get(index).cloned())
                });
                Some((
                    expanded_row,
                    table.expanded_child_row(),
                    node.children().len(),
                    expanded,
                ))
            })
        else {
            return false;
        };
        let expected_children = usize::from(expanded.is_some());
        if expanded_row == materialized_row && child_count == expected_children {
            return false;
        }

        // 仅在存在合法展开行时签发固定树 store 与已验证 Table owner 的窄能力。
        let children = if let Some((row, row_key)) = expanded.as_ref() {
            // 捕获能力固定当前树与 Table owner，renderer 无法伪造状态归属。
            let capture_context = ViewAdapter::dynamic_capture_context(self, id);
            // 在稳定 row_key 命名空间内捕获本轮完整展开行声明输出。
            self.render_handler_table
                // 让 State、Effect、AnimatedSource 与 receipt 都进入宿主树事务。
                .render_table_expand_view(&capture_context, row_key, row)
                // 将可选单根转换为动态协调器消费的子节点集合。
                .into_iter()
                // 当前 Table 同时最多拥有一个活动展开行根。
                .collect()
        } else {
            // 折叠或无效索引必须协调为空集合并释放旧展开行资源。
            Vec::new()
        };
        // 使用动态协调事务挂载扩展行，成功后才接纳其组件状态 journal。
        crate::ui::adapter::ViewAdapter::reconcile_dynamic_children(self, id, children);
        self.mark_table_expand_materialized(id);
        true
    }

    pub(crate) fn refresh_virtual_scroll_widget(
        &mut self,
        id: WidgetId,
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
            let scroll = node.widget().as_any().downcast_ref::<VirtualScroll>()?;
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
            .and_then(|node| node.widget().as_any().downcast_ref::<VirtualScroll>())
        {
            scroll.mark_children_materialized(range);
        }
        // 只有子结构或顺序变化时向调用方报告结构更新。
        changed
    }

    // 表格 capability 启用时才刷新泛型单元格动态子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_cell_widget(&mut self, id: WidgetId) -> bool {
        // 失败或关闭树不得再调用应用提供的表格单元格 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        // 已销毁或正在离场的宿主不再签发动态捕获能力或执行应用 renderer。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 保留既有墓碑直到真实移除，不在离场阶段重建单元格树。
            return false;
        }
        // 陈旧 id 或非 Table owner 不能借用同一 widget 槽位的 renderer。
        if !self
            // 只读取当前运行时节点的实际组件类型。
            .get(id)
            // 让 Table 是本刷新入口唯一允许的 owner 类型。
            .is_some_and(|node| node.widget().as_any().is::<Table>())
        {
            // 拒绝非 Table owner，避免状态写入错误的动态命名空间。
            return false;
        }
        if !self.render_handler_table.contains_table_cells(id) {
            return false;
        }

        let Some((range, needs_refresh)) = self.get(id).and_then(|node| {
            let table = node.widget().as_any().downcast_ref::<Table>()?;
            let range = table.cell_view_range_for_frame(node.frame());
            // 离场墓碑仍保留在直接 children 链中，但不属于当前活动物化窗口。
            let mounted_children = node
                // 只检查 DataTable 直接拥有的动态单元格。
                .children()
                // 排除正在 leave 或其祖先已经 leave 的单元格子树。
                .iter()
                // 保留会参与本轮 keyed reconcile 的活动子项。
                .filter(|child_id| !self.is_pending_removal_subtree(**child_id))
                // 得到活动物化单元格总数，避免墓碑触发重复 renderer。
                .count();
            Some((
                range,
                // 只让活动子项参与是否需要重建当前单元格窗口的判定。
                table.needs_cell_refresh(range, mounted_children),
            ))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }

        let Some((row_keys, view_columns)) = self.get(id).and_then(|node| {
            let table = node.widget().as_any().downcast_ref::<Table>()?;
            Some((table.row_keys().to_vec(), table.view_columns().to_vec()))
        }) else {
            return false;
        };

        // 在借用 renderer sidecar 前签发固定树 store 与已验证 Table owner 的窄能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 缺少 sidecar 不是合法空窗口，不能据此删除已有动态单元格。
        let Some(children) = self
            .render_handler_table
            // 让每个单元格 renderer 在宿主树私有命名空间中完成捕获。
            .render_table_cells(&capture_context, range, &row_keys, &view_columns)
        else {
            // 保留旧 children 与物化范围，等待声明 sidecar 被恢复。
            return false;
        };
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(table) = self
            .get(id)
            .and_then(|node| node.widget().as_any().downcast_ref::<Table>())
        {
            table.mark_cells_materialized(range);
        }
        changed
    }

    pub(crate) fn refresh_select_option_widget(&mut self, id: WidgetId) -> bool {
        // 失败或关闭树不得再调用应用提供的选择项 renderer。
        if !self.accepts_coordination_work() {
            // 直接拒绝本轮刷新，保留等待 owner teardown 的既有资源。
            return false;
        }
        // 已销毁或正在离场的宿主不再签发动态捕获能力或执行应用 renderer。
        if self.get(id).is_none_or(|node| node.destroyed()) || self.is_pending_removal_subtree(id) {
            // 保留既有墓碑直到真实移除，不在离场阶段重建选项树。
            return false;
        }
        // 陈旧 id 或非 Select owner 不能借用同一组件槽位的 renderer。
        if !self
            // 只读取当前运行时节点的实际组件类型。
            .get(id)
            // 让 Select 成为本刷新入口唯一允许的 owner 类型。
            .is_some_and(|node| node.widget().as_any().is::<Select>())
        {
            // 拒绝非 Select owner，避免状态写入错误的动态命名空间。
            return false;
        }
        if !self.render_handler_table.contains_select_options(id) {
            return false;
        }

        let Some(needs_refresh) = self.get(id).and_then(|node| {
            let select = node.widget().as_any().downcast_ref::<Select>()?;
            // 离场墓碑仍保留在直接 children 链中，但不属于当前活动选项窗口。
            let mounted_children = node
                // 只检查 Select 直接拥有的动态选项。
                .children()
                // 排除正在 leave 或其祖先已经 leave 的选项子树。
                .iter()
                // 保留会参与本轮 keyed reconcile 的活动子项。
                .filter(|child_id| !self.is_pending_removal_subtree(**child_id))
                // 得到活动选项总数，避免墓碑触发重复 renderer。
                .count();
            // 先用无分配遍历比较活动窗口，变化时才物化捕获参数。
            Some(select.needs_custom_option_refresh(mounted_children))
        }) else {
            return false;
        };
        if !needs_refresh {
            return false;
        }
        let Some((indices, labels)) = self.get(id).and_then(|node| {
            let select = node.widget().as_any().downcast_ref::<Select>()?;
            let indices = select.custom_option_indices();
            let labels = select.custom_option_labels(&indices);
            Some((indices, labels))
        }) else {
            return false;
        };

        // 在借用 renderer sidecar 前签发固定树 store 与已验证 Select owner 的窄能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 缺少 sidecar 不是合法空窗口，不能据此删除已有动态选项。
        let Some(children) = self
            .render_handler_table
            // 让每个选项 renderer 在宿主树私有命名空间中完成捕获。
            .render_select_options(&capture_context, &indices, &labels)
        else {
            // 保留旧 children 与物化索引，等待声明 sidecar 被恢复。
            return false;
        };
        let changed = ViewAdapter::reconcile_dynamic_children(self, id, children);
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.widget().as_any().downcast_ref::<Select>())
        {
            select.mark_custom_options_materialized(indices);
        }
        changed
    }

    pub(crate) fn invalidate_select_option_widget(&self, id: WidgetId) {
        if let Some(select) = self
            .get(id)
            .and_then(|node| node.widget().as_any().downcast_ref::<Select>())
        {
            select.invalidate_custom_option_materialization();
        }
    }

    // 表格 capability 启用时才查询扩展行 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn has_table_expand_renderer(&self, id: WidgetId) -> bool {
        self.render_handler_table.contains_table_expand(id)
    }

    // 表格 capability 启用时才查询泛型单元格 renderer。
    #[cfg(feature = "table")]
    pub(crate) fn has_table_cell_renderer(&self, id: WidgetId) -> bool {
        self.render_handler_table.contains_table_cells(id)
    }

    pub(crate) fn has_select_option_renderer(&self, id: WidgetId) -> bool {
        self.render_handler_table.contains_select_options(id)
    }

    // 供声明树协调器区分 VirtualScroll 动态子树与普通空子列表。
    pub(crate) fn has_virtual_scroll_renderer(&self, id: WidgetId) -> bool {
        // sidecar 中存在行 renderer 即表示子项由虚拟窗口专用入口拥有。
        self.render_handler_table.contains_virtual_scroll_item(id)
    }
}
