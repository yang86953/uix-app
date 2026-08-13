// 动态子树协调沿用 ViewAdapter 的私有身份与事务契约。
impl crate::ui::adapter::ViewAdapter {
    // 在成功的 WidgetTree 事务后才接纳动态声明子树的私有状态写入。
    pub(crate) fn reconcile_dynamic_children(
        // 接收将被协调的目标运行时树。
        tree: &mut crate::ui::WidgetTree,
        // 接收动态子树所属的运行时父节点。
        parent_id: crate::ui::ComponentId,
        // 接收本轮声明的完整动态子树。
        mut children: Vec<crate::ui::view::ViewNode>,
        // 返回结构是否发生变化。
    ) -> bool {
        // 嵌套协调只可在正常或协调中状态执行，失败与关闭状态不得触碰 renderer 输出。
        if !tree.accepts_coordination_work() {
            // 拒绝的声明子树在离开作用域时释放其未提交回执。
            return false;
        }
        // 动态交付前再次校验宿主仍属于当前树，拒绝捕获后已失效的旧 generation。
        assert!(
            // 只有仍可寻址的实际节点才能接纳新的运行时子树。
            tree.get(parent_id).is_some(),
            // 为延迟交付旧捕获提供稳定诊断。
            "动态 View 协调 parent 不属于宿主 WidgetTree"
        );
        // 在消费声明节点前转移所有 journal 回执。
        let receipts =
            crate::ui::adapter::capture_guards::take_component_state_receipts_from_children(
                // 让动态子树及其全部后代共用同一提交批次。
                &mut children,
            );
        // 先让 WidgetTree 完整结束事务，异常时外层 receipts 的 Drop 会回滚。
        let changed = tree.with_component_state_transaction(receipts, |tree| {
            // 动态子树的首个结构改写进入不可逆发布区，panic 后必须 fail-stop。
            tree.mark_coordination_publish_started();
            // 协调动态子树并保留既有的无动画替换语义。
            Self::reconcile_children(tree, parent_id, children, None)
        });
        // 返回协调结果。
        changed
    }
}
