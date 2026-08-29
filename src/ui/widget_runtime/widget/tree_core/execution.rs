// 复用树核心类型与私有字段定义。
use super::*;
// 提供根初始化约束所需的尺寸类型。
use crate::core::Size;

// 承载 WidgetTree 私有事务与关闭实现。
impl WidgetTree {
    // 保存根节点在真实窗口 viewport 可用前的最小布局约束。
    pub(crate) const ROOT_BOOTSTRAP_SIZE: Size = Size { w: 800.0, h: 600.0 };

    // 使用已捕获根绑定的存储创建窗口树，避免首次构建与后续协调分裂状态所有权。
    pub(crate) fn with_widget_state_store(
        // 接收首次根捕获已经使用的窗口私有存储。
        store: crate::ui::widget_state::WidgetStateStore,
    ) -> Self {
        // 在构造时注入捕获根指定的窗口私有存储，其余资源沿用默认值。
        let tree = Self {
            // 保持窗口树对组件状态存储的单一所有权。
            widget_state_store: store,
            // 复用其余默认运行时资源。
            ..Self::default()
        };
        // 返回拥有单一状态存储的树。
        tree
    }

    // 返回当前树唯一拥有的组件私有状态存储句柄。
    pub(crate) fn widget_state_store(&self) -> crate::ui::widget_state::WidgetStateStore {
        // 克隆轻量共享句柄而不复制任何状态。
        self.widget_state_store.clone()
    }

    // 开始适配器的构建或协调事务并推迟作用域状态清理。
    fn begin_widget_state_transaction(&mut self) {
        // 已停止树没有可恢复的协调边界，调用方必须先停止本轮工作。
        assert!(self.accepts_coordination_work());
        // 最外层事务独占初始化本轮发布事实。
        if self.widget_state_transaction_depth == 0 {
            // 从运行态进入协调态，嵌套入口不会覆盖这个状态。
            self.execution_state.begin_coordination();
        }
        // 允许嵌套构建入口而不提前释放仍会在本轮重挂载的状态。
        self.widget_state_transaction_depth = self.widget_state_transaction_depth.saturating_add(1);
    }

    // 结束适配器事务；最终作用域解析由最外层成功入口统一执行。
    fn end_widget_state_transaction(&mut self) {
        // 防御性忽略不成对结束，避免测试辅助路径下溢。
        if self.widget_state_transaction_depth == 0 {
            // 没有事务时无需再次清理。
            return;
        }
        // 释放一层事务深度。
        self.widget_state_transaction_depth -= 1;
    }

    // 在异常展开路径恢复一层事务深度，保留既有挂载状态且不执行破坏性清理。
    fn abort_widget_state_transaction(&mut self) {
        // 没有活跃事务时保持调用幂等。
        if self.widget_state_transaction_depth == 0 {
            // 提前返回避免深度下溢。
            return;
        }
        // 仅撤销当前入口增加的一层深度。
        self.widget_state_transaction_depth -= 1;
    }

    // 返回树是否仍可接收会调用用户组件的外部工作。
    pub(crate) fn accepts_external_work(&self) -> bool {
        // 状态机是外部入口的唯一准入事实来源。
        self.execution_state.accepts_external_work()
    }

    // 返回树是否仍可接受新的或嵌套的协调事务。
    pub(crate) fn accepts_coordination_work(&self) -> bool {
        // 只有运行态与现有协调态可以继续协调。
        self.execution_state.accepts_coordination_work()
    }

    // 返回驱动与绘制是否必须停止访问当前树。
    pub(crate) fn is_fail_stopped(&self) -> bool {
        // poison 与 shutdown 共享同一 fail-stop 外部语义。
        self.execution_state.is_fail_stopped()
    }

    // 在首次真实结构或最终 journal 提交前线性化发布事实。
    pub(crate) fn mark_coordination_publish_started(&mut self) {
        // 已停止树必须在任何真实 mutator 改写前拒绝继续执行。
        assert!(
            // 运行态兼容既有直接 API，协调态承接当前事务，停止态一律失败。
            self.accepts_coordination_work(),
            // 诊断明确要求所有者拆除旧树并创建新的 WidgetTree。
            "已停止的 WidgetTree 不能继续发布协调修改"
        );
        // 协调外的既有直接 API 保持原语义，不伪造事务发布事实。
        if self.widget_state_transaction_depth > 0 {
            // 嵌套 mutator 只能把外层发布标记单调置位。
            self.execution_state.mark_publish_started();
        }
    }

    // 丢弃当前失败或停止协调留下的全部暂存 journal。
    pub(super) fn discard_pending_widget_state_journal(&mut self) {
        // 取走全部回执以在本函数结束前触发其精确回滚。
        let receipts = std::mem::take(&mut self.widget_state_pending_receipts);
        // 回执 Drop 释放尚未接纳的组件私有状态槽位。
        drop(receipts);
        // 取走全部未提交动画源替换以保留旧树绑定。
        let updates = std::mem::take(&mut self.pending_animated_source_owner_updates);
        // 未提交来源不会绑定到当前树作用域。
        drop(updates);
    }

    // 在异常安全边界内执行一次组件状态建树或协调事务。
    pub(crate) fn with_widget_state_transaction<R>(
        &mut self,
        // 接收由本事务及其声明子树创建的待确认状态 journal。
        receipts: Vec<crate::ui::widget_state::WidgetStateCaptureReceipt>,
        // 接收事务期间唯一可变访问当前树的同步闭包。
        action: impl FnOnce(&mut Self) -> R,
    ) -> R {
        // 只保留由当前 WidgetTree 唯一状态存储产生的回执。
        let receipts = receipts
            // 消费输入集合，让错误 store 回执在过滤时立即 Drop 回滚。
            .into_iter()
            // 禁止其他树或一次性捕获的状态 journal 被当前树接纳。
            .filter(|receipt| receipt.belongs_to(&self.widget_state_store))
            // 收集可安全加入本树最外层事务的回执。
            .collect::<Vec<_>>();
        // 记录进入本层前的回执边界，供 panic 精确回滚本层及成功嵌套层。
        let checkpoint = self.widget_state_pending_receipts.len();
        // 记录进入本层前的动画源替换边界，供 panic 保留既有树绑定。
        let animation_checkpoint = self.pending_animated_source_owner_updates.len();
        // 开启一层延迟清理事务。
        self.begin_widget_state_transaction();
        // 将本层声明捕获产生的 journal 交给当前树暂存。
        self.widget_state_pending_receipts.extend(receipts);
        // 把 action 与最外层全部提交收敛到同一异常边界。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 先运行本层唯一允许改写当前树的同步操作。
            let value = action(self);
            // 正常返回后关闭当前事务层。
            self.end_widget_state_transaction();
            // 仅最外层可以接纳全部递归积累的 journal。
            if self.widget_state_transaction_depth == 0 {
                // 内层 panic 即使被调用方捕获也会保留 poison，禁止提交。
                if self.is_fail_stopped() {
                    // 停止树不能保留任何未提交状态或动画临时资源。
                    self.discard_pending_widget_state_journal();
                } else {
                    // 最终动画与 receipt 接纳本身也是不可回滚发布点。
                    self.execution_state.mark_publish_started();
                    // 在最外层成功后才让动画源替换进入真实树注册表。
                    self.commit_pending_animated_source_owner_updates();
                    // 取走全部已成功挂载的回执，避免后续 Drop 回滚。
                    let receipts = std::mem::take(&mut self.widget_state_pending_receipts);
                    // 收集事务成功后仍由实际节点承载的最终作用域真相。
                    let live_scopes = self.widget_state_live_scopes();
                    // 原子提交 live claim、释放缺席 claim 并保留其他外部未决捕获。
                    crate::ui::widget_state::resolve_widget_state_receipts(
                        // 传入当前窗口树唯一的状态存储。
                        &self.widget_state_store,
                        // 传入最外层事务积累的完整回执批次。
                        receipts,
                        // 传入最终运行时树承载的作用域集合。
                        &live_scopes,
                    );
                    // 所有最终提交成功后才重新开放树。
                    self.execution_state.finish_coordination();
                }
            }
            // 返回调用方的原始动作结果。
            value
        }));
        // 只在异常路径恢复本层检查点并保持原 panic payload。
        match result {
            // 成功路径已经在异常边界内完成全部最终提交。
            Ok(value) => value,
            // 异常路径恢复本层 journal、深度与树终态后继续原始展开。
            Err(payload) => {
                // 在恢复深度前读取共享外层的单调发布事实。
                let publish_started = self.execution_state.publish_started();
                // shutdown 可能已经取走整批回执，因此只在检查点仍有效时分割。
                let receipts = if checkpoint <= self.widget_state_pending_receipts.len() {
                    // 取走本层开始后加入的回执，包含所有成功的嵌套事务回执。
                    self.widget_state_pending_receipts.split_off(checkpoint)
                } else {
                    // shutdown 已负责释放全部回执，异常恢复不得用越界 panic 覆盖原 payload。
                    Vec::new()
                };
                // 立即丢弃本层回执以精确撤销对应的新建状态槽。
                drop(receipts);
                // shutdown 也可能已经取走动画草稿，只在原检查点仍有效时分割。
                let updates =
                    if animation_checkpoint <= self.pending_animated_source_owner_updates.len() {
                        // 丢弃本层及其成功嵌套层的动画源请求，旧树注册保持不变。
                        self.pending_animated_source_owner_updates
                            .split_off(animation_checkpoint)
                    } else {
                        // 关闭路径已释放草稿时保持原始异常而不制造二次越界 panic。
                        Vec::new()
                    };
                // 释放未提交来源，确保异常路径不会产生树作用域绑定。
                drop(updates);
                // 避免基于半完成树执行破坏性 prune。
                self.abort_widget_state_transaction();
                // 已发布或最终提交中的 panic 不能恢复半完成树。
                if publish_started {
                    // 所有外部入口随后必须拒绝继续访问树。
                    self.execution_state.poison();
                } else if self.widget_state_transaction_depth == 0 {
                    // 发布前 panic 保留旧树并重新开放下一轮独立协调。
                    self.execution_state.finish_coordination();
                }
                // 保持调用方观察到原始 panic。
                std::panic::resume_unwind(payload)
            }
        }
    }

    // 在关闭路径捕获单个用户生命周期回调，保证后续资源释放继续执行。
    fn shutdown_lifecycle_step(action: impl FnOnce()) -> Option<Box<dyn std::any::Any + Send>> {
        // 只保留 panic payload，调用者负责选择第一个需要恢复的异常。
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).err()
    }

    // 关闭时逐阶段执行一个节点的生命周期，单阶段失败不会跳过剩余阶段。
    fn deactivate_detach_and_destroy_for_shutdown(
        &mut self,
        // 接收需要执行受控生命周期的真实节点身份。
        id: WidgetId,
    ) -> Option<Box<dyn std::any::Any + Send>> {
        // AppState 注册不调用用户代码，先释放它避免回调重入观察旧快照。
        self.unregister_app_state_snapshot(id);
        // 保存该节点生命周期中第一个需要在资源释放后恢复的 panic。
        let mut first_panic = None;
        // 节点可能已在此前的异常回调中移除，因此按存在性防御访问。
        if let Some(node) = self.get_mut_raw(id) {
            // inactive 回调只对仍活跃节点执行一次。
            if node.active() {
                // 先更新标志，防止 panic 后重复调用同一用户回调。
                node.set_active(false);
                // 捕获用户 inactive 回调，关闭仍会继续释放其余资源。
                first_panic = Self::shutdown_lifecycle_step(|| node.on_inactive());
            }
            // unmount 回调只对仍挂载节点执行一次。
            if node.mounted() {
                // 先更新标志，防止 panic 后重复调用同一用户回调。
                node.set_mounted(false);
                // 只在此前没有异常时记录第一个生命周期 panic。
                if first_panic.is_none() {
                    // 捕获用户 unmount 回调并继续完成关闭。
                    first_panic = Self::shutdown_lifecycle_step(|| node.on_unmount());
                } else {
                    // 忽略后续 panic payload 但保证回调仍按关闭契约执行。
                    let _ = Self::shutdown_lifecycle_step(|| node.on_unmount());
                }
            }
            // detach 回调只对仍附着节点执行一次。
            if node.attached() {
                // 先更新标志，防止 panic 后重复调用同一用户回调。
                node.set_attached(false);
                // 只在此前没有异常时记录第一个生命周期 panic。
                if first_panic.is_none() {
                    // 捕获用户 detach 回调并继续完成关闭。
                    first_panic = Self::shutdown_lifecycle_step(|| node.on_detach());
                } else {
                    // 忽略后续 panic payload 但保证回调仍按关闭契约执行。
                    let _ = Self::shutdown_lifecycle_step(|| node.on_detach());
                }
            }
            // destroy 回调只对尚未销毁节点执行一次。
            if !node.destroyed() {
                // 先更新标志，防止 panic 后重复调用同一用户回调。
                node.set_destroyed(true);
                // 只在此前没有异常时记录第一个生命周期 panic。
                if first_panic.is_none() {
                    // 捕获用户 destroy 回调并继续完成关闭。
                    first_panic = Self::shutdown_lifecycle_step(|| node.on_destroy());
                } else {
                    // 忽略后续 panic payload 但保证回调仍按关闭契约执行。
                    let _ = Self::shutdown_lifecycle_step(|| node.on_destroy());
                }
            }
        }
        // 调用方在所有资源都释放后恢复这个最早的 panic。
        first_panic
    }

    // 在关闭路径完成全部节点受控生命周期并返回最早 panic。
    fn teardown_all_for_shutdown(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        // 从根开始用私有 raw accessor 快照结构，公开访问已在 Shutdown 状态关闭。
        let mut stack = self.root_id.into_iter().collect::<Vec<_>>();
        // 保存父先于子的稳定遍历，随后反转为子先于父的关闭顺序。
        let mut ids = Vec::new();
        // 迭代遍历避免深树递归耗尽线程栈。
        while let Some(id) = stack.pop() {
            // 记录当前仍真实存在的节点身份。
            ids.push(id);
            // 只有物理槽位仍存在时才继续访问其子节点关系。
            if let Some(node) = self.get_raw(id) {
                // 逆序压栈以保持声明子节点的原有遍历顺序。
                stack.extend(node.children().iter().rev().copied());
            }
        }
        // 保存跨节点最早发生的用户生命周期 panic。
        let mut first_panic = None;
        // 逆序保持现有子先于父的关闭顺序。
        for id in ids.into_iter().rev() {
            // 执行单节点关闭且不让单个 panic 中断资源释放。
            let panic = self.deactivate_detach_and_destroy_for_shutdown(id);
            // 保留最早 payload，后续节点仍继续清理。
            if first_panic.is_none() {
                // 首个异常由树所有者在最终资源释放后恢复。
                first_panic = panic;
            }
        }
        // 返回延迟恢复的首个用户生命周期异常。
        first_panic
    }

    /// 在所属窗口释放渲染资源前结束整棵树的挂载生命周期。
    /// 重复关闭保持幂等；每个节点的生命周期标志会抑制重复回调。
    pub(crate) fn shutdown(&mut self) {
        // 先线性化 shutdown，禁止回调重入后继续处理外部或协调工作。
        let run_lifecycle = self.execution_state.begin_shutdown();
        // 仅完整运行树拥有执行用户生命周期的可靠结构前提。
        let lifecycle_panic = run_lifecycle
            .then(|| self.teardown_all_for_shutdown())
            .flatten();
        // poison 或协调中关闭不再调用用户生命周期，只直接释放树拥有资源。
        // 从全部实际槽位收集身份，不能依赖 panic 后可能不完整的根可达关系。
        let mut registered_ids = self
            // 遍历物理节点槽以覆盖已插入但尚未连接到父节点的半发布节点。
            .nodes
            // 只读访问不会调用任何用户组件能力。
            .iter()
            // 保留物理槽位索引以在非测试构建中重建完整身份。
            .enumerate()
            // 忽略未占用或已经释放的槽位。
            .filter_map(|(slot, node)| {
                // 实际节点沿用该槽当前 generation 与树作用域。
                node.as_ref().map(|_| {
                    // 构造不会依赖测试专用 WidgetCore::id 的稳定身份。
                    WidgetId::from_scoped_parts(self.tree_scope, slot, self.generations[slot])
                })
            })
            // 用有序集合去重并保持关闭行为确定。
            .collect::<BTreeSet<_>>();
        // 额外纳入没有对应节点槽但仍可能绑定外部 AppState 的焦点句柄身份。
        registered_ids.extend(self.focus_handles.keys().copied());
        // 即使半树跳过生命周期，也必须解除 AppState 与 FocusHandle 外部绑定。
        for id in registered_ids {
            // 注销只操作框架 registry，不执行节点用户回调。
            self.unregister_app_state_snapshot(id);
        }
        // 释放根生命周期持有的结构性 State 订阅租约。
        self.root_reconcile_state_binds.clear();
        // 释放根捕获的 Effect，避免关闭后继续保留待处理工作。
        self.effects.clear();
        // 清空全部所有者分区，确保关闭不再保留根或节点声明。
        self.animated_source_owners.clear();
        // 取消关闭前尚未提交的全部动画源替换，同时保持活跃事务检查点有效。
        self.cancel_pending_animated_source_owner_updates(|_| true);
        // 取走未提交回执，事务内 checkpoint 仍需要稳定向量长度直到栈展开。
        let pending_receipts = std::mem::take(&mut self.widget_state_pending_receipts);
        // 取走未提交动画更新，同样避免 shutdown 压缩活跃事务检查点。
        let pending_animation_updates =
            std::mem::take(&mut self.pending_animated_source_owner_updates);
        // 关闭后没有活跃事务深度可继续持有暂存状态。
        self.widget_state_transaction_depth = 0;
        // 释放全部捕获的动画源，并由绑定包装器解除源端所有权。
        self.animated_sources.clear();
        // 关闭后不再保留任何组件动画活动标记。
        self.active_widget_animations.clear();
        // 清空动画遍历暂存，避免关闭后的旧节点身份继续存活。
        self.animation_ids_scratch.clear();
        // 释放仍由未移除节点持有的 State 租约与 Effect。
        for node in self.nodes.iter_mut().flatten() {
            // 关闭节点结构性 State 订阅。
            node.clear_reconcile_state_binds();
            // 关闭节点绘制 State 订阅并释放失效队列强引用。
            node.clear_paint_state_binds();
            // 布局依赖与绘制依赖分别持有，停止前必须同时释放。
            node.clear_layout_state_binds();
            // 清空节点 Effect，关闭后不再参与调度。
            node.replace_captured_effects(Vec::new());
        }
        // 释放所有延迟 View 工厂及其应用捕获资源，关闭后不得保留 sidecar。
        self.render_handler_table.clear();
        // 释放事件处理器 sidecar，关闭后不得保留用户闭包。
        self.handler_table.clear();
        // 清空定时器路由，关闭后的队列项不能再命中旧节点。
        self.timer_routes.clear();
        // 释放焦点句柄及恢复栈，防止旧节点身份跨所有者存活。
        self.focus_handles.clear();
        // 清空焦点陷阱恢复记录，关闭后不再允许恢复旧焦点。
        self.focus_trap_restore.clear();
        // 清空所有交互 manager 对旧节点的引用。
        self.reset_interaction_state();
        // 清空悬浮层 sidecar，关闭后不再保留组件覆盖内容。
        self.overlay_stack.clear();
        // 释放默认 manager 与全部组件覆写中可能保留的旧节点运行态。
        self.managers = WidgetManagers::new();
        // 清空尚未交给窗口系统的动作与滚动工作。
        self.pending_window_actions.clear();
        // 清空尚未消费的滚动区域记录。
        self.scroll_region_moves.clear();
        // 清空协调请求，防止关闭树触发空转帧循环。
        self.reconcile_requested
            .store(false, std::sync::atomic::Ordering::Release);
        // 清空本地与共享失效工作，关闭后不得继续请求渲染。
        self.reset_invalidation();
        // 关闭树不会再完成未闭合的失效批次。
        self.invalidation_batch_depth = 0;
        // 清空布局与遍历暂存，避免旧节点身份被下一所有者观察。
        self.layout_ancestor_scratch.clear();
        // 重置布局帧暂存以释放其中保留的旧节点工作集合。
        self.layout_scratch = tree_layout::LayoutFrameScratch::default();
        // 清空遍历缓存，关闭后不得再暴露旧节点身份快照。
        self.cached_traversal.borrow_mut().0.clear();
        self.traversal_stack_scratch.borrow_mut().clear();
        // 清空动画外的生命周期暂存。
        self.lifecycle_states_scratch.clear();
        // 清空语义事件暂存。
        self.app_state_semantic_events_scratch.clear();
        // 清空焦点请求暂存。
        self.app_state_focus_requests_scratch.clear();
        // 清空根身份并使所有旧节点槽位立即不可达。
        self.root_id = None;
        // 清空真实节点以释放其组件和任意剩余用户资源。
        self.nodes.clear();
        // 清空可重用槽位，关闭树不会再次分配 WidgetId。
        self.free_slots.clear();
        // 清空下一槽位游标，保持 shutdown 后内部状态一致。
        self.next_slot = 0;
        // 使旧 WidgetId generation 不再匹配未来所有者。
        for generation in &mut self.generations {
            // 递增 generation 使任何保留旧身份都失效。
            *generation = generation.wrapping_add(1);
        }
        // 释放窗口语义状态注册表持有的全部剩余快照。
        self.app_state = None;
        // 测试宿主关闭时同步释放自动化记录器持有的窗口资源。
        #[cfg(feature = "test-harness")]
        // 终止记录器所有权，禁止 shutdown 后继续记录旧树事件。
        {
            // 清空可选记录器并释放其捕获资源。
            self.automation_recorder = None;
        }
        // 窗口关闭后不再允许任何组件私有状态继续存活。
        self.widget_state_store.clear();
        // 所有活跃事务栈离开前暂存回执必须继续存活，避免 action 在 shutdown 后复用 provisional State。
        drop(pending_receipts);
        // 动画草稿在真实 owner 全部清理后统一释放。
        drop(pending_animation_updates);
        // 所有树拥有资源都已释放后，才恢复最早的用户生命周期 panic。
        if let Some(payload) = lifecycle_panic {
            // 保留原始 panic payload 供所有者的错误边界观察。
            std::panic::resume_unwind(payload);
        }
    }
}
