// 仅在测试或测试宿主中引入声明式 View 契约。
#[cfg(any(test, feature = "test-harness"))]
use crate::ui::view::View;
// 引入根声明节点以承接捕获阶段的完整输出。
use crate::ui::view::ViewNode;
// 引入运行时树以保持事务与发布的唯一所有者。
use crate::ui::WidgetTree;
// 引入已装箱节点以复用同一身份判定。
use crate::ui::widget_runtime::widget::BoxedWidget;

// 根协调入口保持在 ViewAdapter 私有 Module 内，避免泄漏事务状态。
impl crate::ui::adapter::ViewAdapter {
    // 在调用方已取得当前节点借用时复用同一判定，避免重复按身份查询。
    pub(super) fn can_reuse_current(current: &BoxedWidget, node: &ViewNode) -> bool {
        // 组件具体类型必须保持一致。
        current.widget().as_any().type_id() == node.widget_type_id()
            // 根序号与嵌套顺序共同决定实际组件实例身份。
            && current.uix_widget_scopes() == node.uix_widget_scopes.as_slice()
    }

    // 判断声明节点能否复用既有运行时身份。
    pub(super) fn can_reuse(
        // 接收现有运行时树。
        tree: &WidgetTree,
        // 接收待复用节点身份。
        id: crate::ui::WidgetId,
        // 接收新一轮声明节点。
        node: &ViewNode,
    ) -> bool {
        // 类型相同仍需要求内联组件作用域列表完全一致。
        tree.get(id)
            .is_some_and(|current| Self::can_reuse_current(current, node))
    }

    // 判断当前与下一轮处理器身份是否都已稳定登记 generation。
    pub(super) fn handler_signatures_are_stable(
        // 接收当前运行时处理器签名。
        current: &[crate::ui::event::HandlerSignature],
        // 接收下一轮声明处理器签名。
        next: &[crate::ui::event::HandlerSignature],
    ) -> bool {
        // 两侧每个签名都必须持有稳定 generation。
        current
            // 遍历当前签名。
            .iter()
            // 拒绝仍缺少 generation 的当前签名。
            .all(|signature| signature.generation.is_some())
            // 下一轮也必须全部稳定。
            && next
                // 遍历下一轮签名。
                .iter()
                // 拒绝仍缺少 generation 的新签名。
                .all(|signature| signature.generation.is_some())
    }

    /// 构建 View 并捕获其结构性 State 绑定。
    #[cfg(any(test, feature = "test-harness"))]
    pub fn capture_view(view: impl View) -> ViewNode {
        // 复用根捕获入口以让测试 View 同样获得独立状态所有权。
        Self::capture_root(|| view.build())
    }

    /// 把 View 树构建为独占运行时 WidgetTree。
    #[cfg(any(test, feature = "test-harness"))]
    pub fn build(view: impl View) -> WidgetTree {
        // 让测试构建也拥有可跨后续协调复用的窗口私有状态存储。
        Self::build_nodes(Self::capture_root(|| view.build()))
    }

    /// 把已经展开的 ViewNode 树构建为运行时 WidgetTree。
    pub(crate) fn build_nodes(mut root: ViewNode) -> WidgetTree {
        // 复用捕获根携带的存储，保证首次构建与后续协调归属同一窗口。
        let mut tree = root
            .widget_state_store
            .take()
            .map(WidgetTree::with_widget_state_store)
            .unwrap_or_default();
        // 在纯展开前转移全部 journal，保证展开 panic 时回执自动回滚。
        let receipts = crate::ui::adapter::capture_guards::take_widget_state_receipts(&mut root);
        // 在建树成功前仅暂存根动画源，避免展开 panic 提前替换所有权。
        let animated_sources = std::mem::take(&mut root.animated_sources);
        // 在展开前取出根结构绑定，避免消费声明根后丢失交接所有权。
        let state_binds = std::mem::take(&mut root.captured_state_binds);
        // 在展开前取出根 Effect，等待运行时树成功发布后再替换。
        let effects = std::mem::take(&mut root.captured_effects);
        // 在展开前取出整棵声明树评估过的 @media 断点与构建宽度。
        let (media_breakpoints, built_width) =
            crate::ui::adapter::capture_guards::take_media_breakpoints(&mut root);
        // 纯声明展开在事务发布线前完成，失败时新建树与回执会一同释放。
        let wnode = Self::expand(root);
        // 事务只覆盖运行时发布，panic 会由 WidgetTree 的私有状态机接管。
        tree.with_widget_state_transaction(receipts, |tree| {
            // 从此行起运行时结构可能已不可逆，必须进入 fail-stop 保护范围。
            tree.mark_coordination_publish_started();
            // 挂载完整 WidgetNode 树。
            tree.build(wnode);
            // 完整建树成功后才替换根动画源所有权。
            tree.sync_animated_sources(animated_sources);
            // 提交当前根显式携带的结构性 State 绑定。
            tree.replace_root_captured_state_binds(state_binds);
            // 替换当前根显式携带的 Effect 集合。
            tree.register_root_effects(effects);
            // 记录本次完整构建评估过的 @media 断点。
            tree.replace_media_breakpoints(media_breakpoints, built_width);
        });
        // 返回唯一拥有全部已发布资源的运行时树。
        tree
    }

    /// 把新的 View 树协调到既有 WidgetTree。
    #[cfg(any(test, feature = "test-harness"))]
    pub fn reconcile(tree: &mut WidgetTree, view: impl View) {
        // 停止树必须在执行用户 View::build 前拒绝新的声明输入。
        if !tree.accepts_external_work() {
            // 调用方负责丢弃旧树并创建新的窗口树所有者。
            return;
        }
        // 复用目标树的存储，防止测试协调分配新的私有状态。
        let store = tree.widget_state_store();
        // 测试协调的构建同样在窗口主题作用域内读取 token。
        let root = {
            let _build_theme =
                crate::ui::widget_runtime::build_theme::BuildThemeScope::enter(tree.theme_tokens());
            // 构建期 @media 按所属树根 frame 宽度评估。
            let _build_viewport = crate::ui::widget_runtime::build_viewport::BuildViewportScope::enter(
                tree.viewport_width_for_build(),
            );
            Self::capture_root_with_store(store, || view.build())
        };
        // 用带有复用状态的根协调目标树。
        Self::reconcile_nodes(tree, root);
    }

    /// 重建一个子树作用域节点：重跑其声明闭包并只原位协调该子树。
    ///
    /// 闭包输出的 State/Effect/动画依赖经独立捕获帧挂回本节点；
    /// 事务边界与根协调一致（panic 由组件状态事务回滚并 fail-stop）。
    pub(crate) fn reconcile_scoped_node(tree: &mut WidgetTree, id: crate::ui::WidgetId) {
        // 停止树必须拒绝新的声明输入；请求由消费端作废。
        if !tree.accepts_external_work() {
            return;
        }
        // 节点已离场或不是作用域根时，该请求是迟到投递，直接作废。
        let Some(rebuild) = tree.get(id).and_then(|node| node.scoped_rebuild()) else {
            return;
        };
        // 复用目标树的存储，使作用域重建与根构建拥有同一状态命名空间。
        let store = tree.widget_state_store();
        // 一份工厂进捕获闭包执行，一份随声明根重装为节点租约。
        let rebuild_for_capture = std::sync::Arc::clone(&rebuild);
        // 闭包在完整捕获边界内执行：State 依赖挂到本次声明根而非外层根帧。
        let mut root = {
            // 作用域重建在所属窗口主题作用域内读取 token，与根协调一致。
            let _build_theme =
                crate::ui::widget_runtime::build_theme::BuildThemeScope::enter(tree.theme_tokens());
            // 作用域重建同样按当前根 frame 宽度评估 @media。
            let _build_viewport = crate::ui::widget_runtime::build_viewport::BuildViewportScope::enter(
                tree.viewport_width_for_build(),
            );
            Self::capture_root_with_store(store, move || rebuild_for_capture())
        };
        // 重建工厂随声明根传递，协调时重装为节点作用域租约。
        root.scoped_rebuild = Some(rebuild);
        // 消费声明子树前转移全部 journal，与根协调同一事务边界。
        let receipts = crate::ui::adapter::capture_guards::take_widget_state_receipts(&mut root);
        // 作用域重建只补充断点，不清除其他子树已登记的阈值。
        let (media_breakpoints, built_width) =
            crate::ui::adapter::capture_guards::take_media_breakpoints(&mut root);
        tree.with_widget_state_transaction(receipts, |tree| {
            // 从此行起运行时结构可能已不可逆，必须进入 fail-stop 保护范围。
            tree.mark_coordination_publish_started();
            // 只协调该子树：节点身份保持，差异按 reconcile_existing 语义应用。
            Self::reconcile_existing(tree, id, root);
            tree.merge_media_breakpoints(media_breakpoints, built_width);
        });
    }

    /// 把已经捕获的 ViewNode 树协调到既有 WidgetTree。
    pub(crate) fn reconcile_nodes(tree: &mut WidgetTree, mut root: ViewNode) {
        // 外部协调只可从正常运行态开始，拒绝输入会在离开作用域时释放其回执。
        if !tree.accepts_external_work() {
            // 不接管失败树或已关闭树的任何声明输出。
            return;
        }
        // 在消费声明树前转移全部 journal，保证纯展开异常仍由回执回滚。
        let receipts = crate::ui::adapter::capture_guards::take_widget_state_receipts(&mut root);
        // 在协调成功前仅暂存根动画源，保留 panic 前的旧根注册。
        let animated_sources = std::mem::take(&mut root.animated_sources);
        // 在准备阶段取出根结构绑定，保持事务发布线之后只处理运行时写入。
        let state_binds = std::mem::take(&mut root.captured_state_binds);
        // 在准备阶段取出根 Effect，避免展开后遗失所有权交接。
        let effects = std::mem::take(&mut root.captured_effects);
        // 在准备阶段取出整棵声明树评估过的 @media 断点与构建宽度。
        let (media_breakpoints, built_width) =
            crate::ui::adapter::capture_guards::take_media_breakpoints(&mut root);
        // 先判定是否可原位复用，避免在事务中执行纯身份查询。
        let reuse_root = tree
            .root_id()
            .filter(|root_id| Self::can_reuse(tree, *root_id, &root));
        // 用私有计划把纯展开严格留在首次不可逆发布前。
        enum ReconcilePlan {
            // 保留既有节点身份并协调其可变状态。
            Reuse(crate::ui::WidgetId, ViewNode),
            // 使用已完成的纯展开结果替换根节点。
            Replace(crate::ui::widget_runtime::widget::WidgetNode),
        }
        // 只有替换路径需要展开声明根，panic 发生时旧运行时树仍保持可用。
        let plan = match reuse_root {
            // 将未展开的声明根交给原位协调路径。
            Some(root_id) => ReconcilePlan::Reuse(root_id, root),
            // 在提交线之前完成纯展开，保留旧树作为恢复态。
            None => ReconcilePlan::Replace(Self::expand(root)),
        };
        // 事务覆盖整次发布，允许同轮类型替换继续复用捕获到的状态。
        tree.with_widget_state_transaction(receipts, |tree| {
            // 此后任意 panic 都必须把已触及的运行时树置为 fail-stop。
            tree.mark_coordination_publish_started();
            // 按准备阶段确定的唯一计划发布运行时变更。
            match plan {
                // 同类型与同作用域根执行精细协调。
                ReconcilePlan::Reuse(root_id, root) => {
                    // 协调现有根及其子树。
                    Self::reconcile_existing(tree, root_id, root);
                }
                // 身份变化时替换完整运行时根。
                ReconcilePlan::Replace(wnode) => {
                    // 挂载预先完成纯展开的新声明根。
                    tree.build(wnode);
                }
            }
            // 根协调完整成功后才替换动画源，避免失败路径丢失旧注册。
            tree.sync_animated_sources(animated_sources);
            // 提交本轮根显式携带的结构性 State 绑定。
            tree.replace_root_captured_state_binds(state_binds);
            // 替换本轮根显式携带的 Effect 集合。
            tree.register_root_effects(effects);
            // 完整根协调后用本轮评估过的 @media 断点替换记录。
            tree.replace_media_breakpoints(media_breakpoints, built_width);
        });
    }
}

#[cfg(test)]
#[path = "../../../../tests-src/ui/coordination/adapter/coordination_tests.rs"]
mod scoped_reconcile_tests;

