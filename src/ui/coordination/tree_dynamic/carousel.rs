// 导入带 generation 的树内组件身份。
use crate::core::WidgetId;
// 导入动态捕获与 receipt 感知的子树协调入口。
use crate::ui::adapter::ViewAdapter;
// 导入宿主声明期 Provider 上下文恢复能力。
use crate::ui::widget_runtime::provider_context::with_provider_context;
// 导入 Carousel 动态子树的唯一生命周期 owner。
use crate::ui::widget_runtime::widget::tree_core::WidgetTree;
// 导入读取运行时组件与直接子节点所需的树节点契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入专属延迟箭头工厂的合法宿主类型。
use crate::ui::widgets::display::Carousel;

// 为 WidgetTree 增加 Carousel 私有动态箭头协调边界。
impl WidgetTree {
    // 判断当前组件身份是否仍属于一个 live Carousel 槽位。
    pub(crate) fn is_carousel_custom_arrows_widget(&self, id: WidgetId) -> bool {
        // 只接受当前树中可寻址的实际 Carousel，拒绝陈旧 generation 与其他组件。
        self.get(id)
            // 收窄到产品定义的合法动态箭头 owner 类型。
            .is_some_and(|node| node.widget().as_any().is::<Carousel>())
    }

    // 在已挂载 Carousel 下首次物化或恢复固定动态箭头子树。
    pub(crate) fn refresh_carousel_custom_arrows_widget(
        // 借用拥有状态 store、节点和动态事务的真实树。
        &mut self,
        // 接收已经注册到当前树的 Carousel owner 身份。
        id: WidgetId,
        // 返回本轮是否改变了直接子树结构或离场状态。
    ) -> bool {
        // 关闭或 fail-stop 树不得再次执行应用箭头工厂。
        if !self.accepts_coordination_work() {
            // 保留既有资源等待受控 teardown。
            return false;
        }
        // 已销毁、陈旧或离场 owner 不得取得新的动态捕获能力。
        if self.get(id).is_none_or(|node| node.destroyed())
            // owner 及其任一祖先进入 leave 后均拒绝迟到刷新。
            || self.is_pending_removal_subtree(id)
        {
            // 离场墓碑继续持有资源直至真实移除。
            return false;
        }
        // 固定入口只允许当前树中的实际 Carousel 使用。
        if !self.is_carousel_custom_arrows_widget(id) {
            // 非 Carousel 或过期 generation 不执行任何用户代码。
            return false;
        }
        // 查找已经发布或仍在 leave 的固定箭头直接子节点。
        let existing = self.get(id).and_then(|node| {
            // 只按框架保留 key 查找，绝不依赖子节点顺序或具体类型。
            node.children().iter().copied().find(|child_id| {
                // 同一父级下只有固定 key 能代表已发布动态箭头实例。
                self.get(*child_id).and_then(|child| child.key())
                    // 比较 Carousel 声明的唯一产品身份。
                    == Some(Carousel::CUSTOM_ARROWS_CHILD_KEY)
            })
        });
        // 在执行任何应用工厂前读取当前 live Carousel 的需求开关。
        let needs_arrows = self
            // 读取已通过 owner 类型检查的真实节点。
            .get(id)
            // 收窄到 Carousel 私有声明状态。
            .and_then(|node| node.widget().as_any().downcast_ref::<Carousel>())
            // show_arrows、custom 标志与工厂必须同时有效。
            .is_some_and(Carousel::needs_custom_arrows_view);
        // 禁用或移除工厂时只删除旧固定动态箭头，不调用应用代码。
        if !needs_arrows {
            // 没有旧箭头时保持结构不变。
            return existing
                // 有旧实例时沿正常 leave 或真实 remove 生命周期释放。
                .is_some_and(|child_id| ViewAdapter::remove_dynamic_child(self, id, child_id));
        }
        // 首次物化入口遇到同 key leave 墓碑时只恢复旧实例。
        if let Some(child_id) = existing {
            // 取消离场并复用已经提交的 State、Effect 与动画所有权。
            return ViewAdapter::cancel_dynamic_child_removal(self, id, child_id);
        }
        // 为已经验证的 live Carousel 签发不可伪造的树私有捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 在 owner 声明时的 Provider 可见范围内执行箭头工厂。
        let arrows = self.get(id).and_then(|node| {
            // 克隆上下文以在调用用户代码前结束对树节点的长借用意图。
            let provider_context = node.provider_context().clone();
            // 恢复与 Carousel 声明一致的 ProviderContext。
            with_provider_context(&provider_context, || {
                // 再次读取当前节点，防御用户同步重入引发的身份变化。
                node.widget()
                    // 不把 WidgetTree 或其他组件实现泄漏给箭头工厂。
                    .as_any()
                    // 只允许实际 Carousel 读取其私有工厂。
                    .downcast_ref::<Carousel>()?
                    // 捕获完整 State、Effect、AnimatedSource 与 receipt 输出。
                    .custom_arrows_view_for_reconcile(&capture_context)
            })
        });
        // owner 失效或声明不再需要箭头时拒绝发布空候选。
        let Some(arrows) = arrows else {
            // 捕获上下文离开作用域会自动回滚未提交 receipt。
            return false;
        };
        // 通过窄追加事务首次发布固定动态箭头，保留全部 authored slides。
        ViewAdapter::append_dynamic_child(self, id, arrows)
    }

    // 在父 Carousel 原位 patch 后统一协调 authored slides 与最新箭头工厂输出。
    pub(crate) fn reconcile_carousel_custom_arrows_widget(
        // 借用当前运行时树与唯一动态状态 store。
        &mut self,
        // 接收已完成 live patch 的 Carousel owner。
        id: WidgetId,
        // 接收调用方本轮声明的全部幻灯片根。
        mut authored_children: Vec<crate::ui::view::ViewNode>,
        // 返回直接子树结构是否改变。
    ) -> bool {
        // 停止或 fail-stop 树不能消费新的声明输出。
        if !self.accepts_coordination_work() {
            // 未发布捕获输出离开作用域后自行回滚。
            return false;
        }
        // 销毁、陈旧或离场 owner 不得再次执行箭头工厂。
        if self.get(id).is_none_or(|node| node.destroyed())
            // owner leave 期间只允许既有墓碑完成生命周期。
            || self.is_pending_removal_subtree(id)
        {
            // 迟到父协调不改写旧树。
            return false;
        }
        // 重新确认 live 槽位仍属于 Carousel。
        if !self.is_carousel_custom_arrows_widget(id) {
            // 防止过期父声明接管其他组件的状态命名空间。
            return false;
        }
        // authored slide 不得占用框架固定箭头身份。
        assert!(
            // 只检查同一父级直接根，因为 keyed 身份按父作用域解释。
            authored_children.iter().all(|child| {
                // 用户声明 key 必须与框架保留 key 不相等。
                child.key.as_deref() != Some(Carousel::CUSTOM_ARROWS_CHILD_KEY)
            }),
            // 给调用方稳定诊断，拒绝动态 owner 被 authored slide 冒充。
            "Carousel authored child 不得使用保留 key uix:carousel:custom-arrows"
        );
        // 为当前 live owner 签发固定树 store 的捕获能力。
        let capture_context = ViewAdapter::dynamic_capture_context(self, id);
        // 只从完成 patch 的 live Carousel 读取最新版工厂与运行态。
        let arrows = self.get(id).and_then(|node| {
            // 克隆宿主 ProviderContext 供同步工厂读取。
            let provider_context = node.provider_context().clone();
            // 让动态箭头观察与 Carousel 相同的声明期 Provider 值。
            with_provider_context(&provider_context, || {
                // 运行时再次收窄 owner 类型以抵御同步重入。
                node.widget()
                    // 保持具体组件访问局限在树私有协调器内。
                    .as_any()
                    // 只调用当前 live Carousel 保存的工厂。
                    .downcast_ref::<Carousel>()?
                    // 原子捕获最新版完整动态输出。
                    .custom_arrows_view_for_reconcile(&capture_context)
            })
        });
        // 启用时把固定箭头追加在全部 authored slides 之后。
        if let Some(arrows) = arrows {
            // 保持幻灯片相对顺序并让结构计数继续把最后节点视为箭头。
            authored_children.push(arrows);
        }
        // 禁用时不追加固定节点，统一 keyed 协调会启动旧箭头离场或真实移除。
        ViewAdapter::reconcile_dynamic_children(self, id, authored_children)
    }
}
