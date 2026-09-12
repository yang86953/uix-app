// 引入动态子节点直接父子关系与离场状态的窄读取契约。
use crate::ui::widget_runtime::widget::WidgetCore;

// 动态子树协调沿用 ViewAdapter 的私有身份与事务契约。
impl crate::ui::adapter::ViewAdapter {
    // 让一个直接动态子节点进入离场或立即释放，并保留其生命周期语义。
    pub fn remove_dynamic_child(
        // 接收拥有动态子节点的目标运行时树。
        tree: &mut crate::ui::WidgetTree,
        // 接收动态子节点的实际父节点身份。
        parent_id: crate::ui::WidgetId,
        // 接收将被离场或移除的直接子节点身份。
        child_id: crate::ui::WidgetId,
        // 返回本轮是否开始了新的移除工作。
    ) -> bool {
        // 失败或关闭树不得再改变动态子树生命周期。
        if !tree.accepts_coordination_work() {
            // 保留停止树的既有资源等待所有者 teardown。
            return false;
        }
        // 动态移除只接受仍由指定父节点直接拥有的实际子节点。
        assert!(
            // 同时校验父侧顺序与子侧 parent，拒绝陈旧 generation 或跨树 id。
            tree.get(parent_id)
                .is_some_and(|parent| parent.children().contains(&child_id))
                && tree
                    .get(child_id)
                    .is_some_and(|child| child.parent() == Some(parent_id)),
            // 为错误接线提供稳定诊断。
            "动态 View 移除目标不是指定 parent 的直接子节点"
        );
        // 已在离场的子节点无需重复建立事务或重启动画。
        if tree
            // 读取目标节点的局部离场状态，不把父级离场误判为本节点请求。
            .get(child_id)
            // 只识别节点自身已经进入 pending removal。
            .is_some_and(|child| child.pending_removal())
        {
            // 保持当前离场进度与资源所有权不变。
            return false;
        }
        // 用独立事务包住首次离场或立即删除，统一发布后 panic 的 fail-stop 语义。
        tree.with_widget_state_transaction(Vec::new(), |tree| {
            // 任何交互取消、离场状态或结构删除前先进入不可逆发布区。
            tree.mark_coordination_publish_started();
            // 有 leave 动画时保留节点及其 State、Effect、动画源直到真实 remove。
            if !tree.start_leave_transition(child_id) {
                // 无 leave 动画时立即走完整 teardown 并释放全部节点资源。
                tree.remove(child_id);
            }
        });
        // 事务正常返回表示移除工作已经建立。
        true
    }

    // 取消一个直接动态子节点的离场，使同 key 错误重入复用原节点。
    pub fn cancel_dynamic_child_removal(
        // 接收拥有动态子节点的目标运行时树。
        tree: &mut crate::ui::WidgetTree,
        // 接收动态子节点的实际父节点身份。
        parent_id: crate::ui::WidgetId,
        // 接收将恢复为活跃状态的直接子节点身份。
        child_id: crate::ui::WidgetId,
        // 返回本轮是否实际取消了离场。
    ) -> bool {
        // 失败或关闭树不得恢复任何动态子树入口。
        if !tree.accepts_coordination_work() {
            // 保留停止树的现状等待所有者 teardown。
            return false;
        }
        // 动态恢复只接受仍由指定父节点直接拥有的实际子节点。
        assert!(
            // 同时校验父侧顺序与子侧 parent，拒绝陈旧 generation 或跨树 id。
            tree.get(parent_id)
                .is_some_and(|parent| parent.children().contains(&child_id))
                && tree
                    .get(child_id)
                    .is_some_and(|child| child.parent() == Some(parent_id)),
            // 为错误接线提供稳定诊断。
            "动态 View 恢复目标不是指定 parent 的直接子节点"
        );
        // 非离场节点已经满足复用条件，无需建立空事务。
        if tree
            // 读取目标节点自身的离场状态。
            .get(child_id)
            // 只在确有 pending removal 时进入恢复事务。
            .is_none_or(|child| !child.pending_removal())
        {
            // 报告本轮没有结构生命周期变化。
            return false;
        }
        // 用独立事务包住离场取消，统一发布后 panic 的 fail-stop 语义。
        tree.with_widget_state_transaction(Vec::new(), |tree| {
            // 清除 pending removal 会改变真实运行态，必须先进入发布区。
            tree.mark_coordination_publish_started();
            // 恢复同 key 原节点及其既有 State、Effect 与动画源所有权。
            let _ = tree.cancel_pending_removal(child_id);
        });
        // 事务正常返回表示同 key 节点已经恢复活跃。
        true
    }

    // 在成功的 WidgetTree 事务后才追加一个动态声明子树的私有状态写入。
    pub fn append_dynamic_child(
        // 接收将被追加子树的目标运行时树。
        tree: &mut crate::ui::WidgetTree,
        // 接收动态子树所属的运行时父节点。
        parent_id: crate::ui::WidgetId,
        // 接收本轮声明的单个动态子树。
        mut child: crate::ui::view::ViewNode,
        // 返回是否已经把子树发布到运行时树。
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
            "动态 View 追加 parent 不属于宿主 WidgetTree"
        );
        // 在消费声明节点前转移其完整子树的所有 journal 回执。
        let receipts = crate::ui::adapter::capture_guards::take_widget_state_receipts(
            // 让动态子树及其全部后代共用同一提交批次。
            &mut child,
        );
        // 动态子树评估过的 @media 断点合并到宿主树记录。
        let (media_breakpoints, built_width) =
            crate::ui::adapter::capture_guards::take_media_breakpoints(&mut child);
        // 在事务发布线前完成纯声明展开，异常时既有运行时树仍可继续服务。
        let child = Self::expand(child);
        // 先让 WidgetTree 完整结束事务，异常时外层 receipts 的 Drop 会回滚。
        tree.with_widget_state_transaction(receipts, |tree| {
            // 动态子树的首个结构改写进入不可逆发布区，panic 后必须 fail-stop。
            tree.mark_coordination_publish_started();
            tree.merge_media_breakpoints(media_breakpoints, built_width);
            // 追加已经完成纯展开的错误子树。
            tree.build_child_node(parent_id, child);
        });
        // 只有事务成功返回后才报告已经发布。
        true
    }

    // 在成功的 WidgetTree 事务后才接纳动态声明子树的私有状态写入。
    pub fn reconcile_dynamic_children(
        // 接收将被协调的目标运行时树。
        tree: &mut crate::ui::WidgetTree,
        // 接收动态子树所属的运行时父节点。
        parent_id: crate::ui::WidgetId,
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
        let receipts = crate::ui::adapter::capture_guards::take_widget_state_receipts_from_children(
            // 让动态子树及其全部后代共用同一提交批次。
            &mut children,
        );
        // 动态子树评估过的 @media 断点合并到宿主树记录。
        let mut media_breakpoints = Vec::new();
        let mut built_width = None;
        for child in &mut children {
            let (breakpoints, width) =
                crate::ui::adapter::capture_guards::take_media_breakpoints(child);
            media_breakpoints.extend(breakpoints);
            built_width = width.or(built_width);
        }
        // 先让 WidgetTree 完整结束事务，异常时外层 receipts 的 Drop 会回滚。
        let changed = tree.with_widget_state_transaction(receipts, |tree| {
            // 动态子树的首个结构改写进入不可逆发布区，panic 后必须 fail-stop。
            tree.mark_coordination_publish_started();
            tree.merge_media_breakpoints(media_breakpoints, built_width);
            // 协调动态子树并保留既有的无动画替换语义。
            Self::reconcile_children(tree, parent_id, children, None)
        });
        // 返回协调结果。
        changed
    }
}
