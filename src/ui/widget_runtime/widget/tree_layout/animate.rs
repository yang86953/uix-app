use super::super::super::*;
// 引入树级动画绑定、待提交请求、所有者分区与宿主树类型。
use super::super::{
    // 引入动画源生命周期所有者身份。
    AnimatedSourceOwner,
    // 引入负责最终解绑的树级包装器。
    BoundAnimatedSource,
    // 引入保持检查点稳定的待提交更新记录。
    PendingAnimatedSourceOwnerUpdate,
    // 引入承载中央动画源注册表的宿主树。
    WidgetTree,
};
use crate::core::Rect;
// 支持按工作身份维护树内唯一动画源绑定。
use crate::ui::animation::{AnimatedRegistration, AnimatedSource};
// 保存所有者到工作身份集合的去重索引。
use std::collections::{BTreeMap, BTreeSet};
// 转交动态动画源的共享所有权。
use std::sync::Arc;
use std::time::Instant;

impl WidgetTree {
    // 仅替换声明根拥有的动画源，保留动态节点的独立所有权。
    pub(crate) fn sync_animated_sources(&mut self, sources: Vec<Arc<dyn AnimatedSource>>) {
        // 根协调在事务内暂存、在事务外立即替换 Root 分区。
        self.request_animated_source_owner_update(AnimatedSourceOwner::Root, sources);
    }

    // 用已存在节点的新捕获结果替换其专属动画源所有权。
    pub(crate) fn replace_node_animated_sources(
        &mut self,
        id: WidgetId,
        sources: Vec<Arc<dyn AnimatedSource>>,
    ) {
        // 已移除或尚未建立的身份不得获得动画源所有权。
        if self.get(id).is_none() {
            // 保留当前注册，避免陈旧协调结果误解绑有效节点。
            return;
        }
        // 节点协调在事务内暂存、在事务外立即替换自身 WidgetId 分区。
        self.request_animated_source_owner_update(AnimatedSourceOwner::Node(id), sources);
    }

    // 按当前事务状态决定暂存或立即应用一次所有权替换。
    fn request_animated_source_owner_update(
        &mut self,
        owner: AnimatedSourceOwner,
        sources: Vec<Arc<dyn AnimatedSource>>,
    ) {
        // 停止树不得重新绑定动画源或建立新的 owner 分区。
        assert!(self.accepts_coordination_work());
        // 普通节点没有动画源时不建立空事务记录；真实旧绑定或同事务内的非空请求仍必须清除。
        if sources.is_empty()
            && !self.animated_source_owners.contains_key(&owner)
            && !self
                .pending_animated_source_owner_updates
                .iter()
                .any(|update| {
                    !update.cancelled && update.owner == owner && !update.sources.is_empty()
                })
        {
            // 空声明没有可变更的生命周期事实，也无需进入提交阶段的 owner 合并表。
            return;
        }
        // 构建或协调事务期间不得改变既有树绑定。
        if self.widget_state_transaction_depth > 0 {
            // 记录待最外层成功后提交的所有权快照。
            self.pending_animated_source_owner_updates
                .push(PendingAnimatedSourceOwnerUpdate {
                    // 保存本次待提交替换所属的生命周期分区。
                    owner,
                    // 保存事务成功后才会尝试绑定的来源。
                    sources,
                    // 新请求尚未被任何真实结构销毁取消。
                    cancelled: false,
                });
            // 保持旧注册并结束本次请求。
            return;
        }
        // 事务外的已建立事实可立即更新树注册表。
        self.replace_animated_source_owner(owner, sources);
    }

    // 在最外层事务成功后，按所有者提交最后一份有效动画源快照。
    pub(in crate::ui::widget_runtime::widget::tree_core) fn commit_pending_animated_source_owner_updates(
        &mut self,
    ) {
        // 一次性取走成功事务积累的请求，此时已不存在活跃检查点。
        let updates = std::mem::take(&mut self.pending_animated_source_owner_updates);
        // 按所有者合并为最后一次有效更新，避免提交中间状态的无谓绑定。
        let mut final_updates = BTreeMap::new();
        // 遍历所有成功事务层留下的请求。
        for update in updates {
            // 已被真实销毁取消的请求不得重新登记失效所有者。
            if update.cancelled {
                // 继续处理仍然有效的待提交请求。
                continue;
            }
            // 后出现的同所有者更新覆盖较早快照。
            final_updates.insert(update.owner, update.sources);
        }
        // 提交每个所有者最终且唯一的来源快照。
        for (owner, sources) in final_updates {
            // 此时事务已成功完成，允许更新中央工作身份注册表。
            self.replace_animated_source_owner(owner, sources);
        }
    }

    // 取消匹配所有者的待提交更新，并在活跃事务内保持队列索引稳定。
    pub(in crate::ui::widget_runtime::widget::tree_core) fn cancel_pending_animated_source_owner_updates(
        &mut self,
        // 由调用方限定根、全部节点或某个实际节点。
        matches_owner: impl Fn(AnimatedSourceOwner) -> bool,
    ) {
        // 活跃事务的检查点依赖队列长度单调不减。
        if self.widget_state_transaction_depth > 0 {
            // 原位访问全部待提交项而不压缩向量。
            for update in &mut self.pending_animated_source_owner_updates {
                // 不匹配的所有者继续保留原始请求。
                if !matches_owner(update.owner) {
                    // 继续检查下一个待提交项。
                    continue;
                }
                // 原位标记取消以保持所有外层和内层检查点下标有效。
                update.cancelled = true;
                // 立即释放已取消的未绑定来源，避免延长无效捕获生命周期。
                update.sources.clear();
            }
            // 活跃事务内完成逻辑取消后保持向量长度不变。
            return;
        }
        // 没有活跃检查点时可物理删除失效请求。
        self.pending_animated_source_owner_updates
            .retain(|update| !matches_owner(update.owner));
    }

    // 以所有者为原子边界替换来源集合，并保持工作身份在树内只绑定一次。
    pub(in crate::ui::widget_runtime::widget::tree_core) fn replace_animated_source_owner(
        &mut self,
        owner: AnimatedSourceOwner,
        sources: Vec<Arc<dyn AnimatedSource>>,
    ) {
        // 同一所有者的重复捕获只保留第一个工作身份对应的源。
        let mut requested_sources = BTreeMap::new();
        // 逐个建立按工作身份去重的请求集。
        for source in sources {
            // 读取该源在树内稳定的工作身份。
            let work_id = source.work_id();
            // 重复工作身份不覆盖先捕获到的源实例。
            requested_sources.entry(work_id).or_insert(source);
        }
        // 暂时取走该所有者的旧声明，其他所有者保持不变。
        let previous_work_ids = self
            .animated_source_owners
            .remove(&owner)
            .unwrap_or_default();
        // 保存本次确实成功接纳的工作身份。
        let mut next_work_ids = BTreeSet::new();
        // 接纳本次请求的每个唯一工作身份。
        for (work_id, source) in requested_sources {
            // 已由树绑定的源只增加声明所有者，不重复调用源端绑定。
            if self.animated_sources.contains_key(&work_id) {
                // 记录该所有者对既有树绑定的引用。
                next_work_ids.insert(work_id);
                // 继续接纳下一个工作身份。
                continue;
            }
            // 首个所有者负责建立树作用域绑定。
            if source.bind_owner(self.tree_scope) {
                // 由中央工作身份注册表唯一持有绑定包装器。
                self.animated_sources.insert(
                    work_id,
                    BoundAnimatedSource {
                        // 让包装器在最后一个所有者离开时解除同一树作用域绑定。
                        tree_scope: self.tree_scope,
                        // 保存实际被树接纳的动态动画源。
                        source,
                    },
                );
                // 仅为成功绑定的源保存该所有者的引用。
                next_work_ids.insert(work_id);
            }
        }
        // 非空集合才需要保留所有者记录。
        if !next_work_ids.is_empty() {
            // 写回该所有者的最新声明快照。
            self.animated_source_owners
                .insert(owner, next_work_ids.clone());
        }
        // 仅检查本所有者原先持有的工作身份，新增身份不可能成为待释放项。
        for work_id in previous_work_ids {
            // 仍由本所有者保留的身份无需释放。
            if next_work_ids.contains(&work_id) {
                // 继续检查下一个旧工作身份。
                continue;
            }
            // 只要还有任何根或节点所有者引用，就必须保留中央绑定。
            let still_owned = self
                .animated_source_owners
                .values()
                .any(|work_ids| work_ids.contains(&work_id));
            // 最后一个所有者离开时移除包装器并触发源端解绑。
            if !still_owned {
                // 删除中央工作身份注册以交由 Drop 执行唯一解绑。
                self.animated_sources.remove(&work_id);
            }
        }
    }

    // 释放所有即将因完整换根而失效的节点动画源所有权。
    pub(in crate::ui::widget_runtime::widget::tree_core) fn clear_node_animated_source_owners(
        &mut self,
    ) {
        // 先逻辑取消所有已失效节点请求，且不破坏活跃事务检查点。
        self.cancel_pending_animated_source_owner_updates(|owner| {
            // 完整换根只取消 Node 分区，Root 仍由根同步独立替换。
            matches!(owner, AnimatedSourceOwner::Node(_))
        });
        // 先取得稳定快照，避免迭代所有者表时修改同一张表。
        let node_owners: Vec<_> = self
            .animated_source_owners
            .keys()
            .filter(|owner| matches!(owner, AnimatedSourceOwner::Node(_)))
            .copied()
            .collect();
        // 逐个以空声明替换节点所有权。
        for owner in node_owners {
            // 最后一个引用离开时由中央注册表解除对应绑定。
            self.replace_animated_source_owner(owner, Vec::new());
        }
    }

    // 在节点实际离开树后，取消其待提交项并立即释放该节点所有权。
    pub(in crate::ui::widget_runtime::widget::tree_core) fn release_node_animated_sources_immediately(
        &mut self,
        id: WidgetId,
    ) {
        // 构造与已销毁节点一一对应的所有权身份。
        let owner = AnimatedSourceOwner::Node(id);
        // 原位取消该节点尚未提交的捕获结果，保持嵌套事务检查点稳定。
        self.cancel_pending_animated_source_owner_updates(|pending_owner| {
            // 仅匹配本次已经真实销毁的节点所有者。
            pending_owner == owner
        });
        // 绕过事务延迟，在节点真实销毁线性化点立即释放树绑定引用。
        self.replace_animated_source_owner(owner, Vec::new());
    }

    /// 推进整棵树的动画，并报告是否仍存在活动工作项。
    pub fn update(&mut self, dt: f64) -> bool {
        // 已停止的树不得再推进会执行组件或动画源代码的动画时钟。
        if !self.accepts_external_work() {
            // 对调度器报告没有活动动画。
            return false;
        }
        self.update_animations(dt)
            .into_iter()
            .any(|(_, still_active)| still_active)
    }

    pub(crate) fn update_animations(&mut self, dt: f64) -> Vec<(WidgetId, bool)> {
        // 已停止的树不得产生后续动画调度结果。
        if !self.accepts_external_work() {
            // 返回空更新以撤销外部帧工作。
            return Vec::new();
        }
        self.update_animations_at(Instant::now(), dt)
    }

    pub(crate) fn update_animations_at(&mut self, now: Instant, dt: f64) -> Vec<(WidgetId, bool)> {
        // 已停止的树不得直接从测试或平台循环推进动画。
        if !self.accepts_external_work() {
            // 返回空更新以保持 fail-stop 语义。
            return Vec::new();
        }
        let ids = self.take_animation_node_ids();
        let updates = self.update_animation_nodes_at(ids.iter().copied(), now, dt);
        self.animation_ids_scratch = ids;
        updates
    }

    pub(crate) fn update_animations_except_at(
        &mut self,
        excluded_ids: &[WidgetId],
        now: Instant,
        dt: f64,
    ) -> Vec<(WidgetId, bool)> {
        // 已停止的树不得通过排除路径绕过动画门禁。
        if !self.accepts_external_work() {
            // 返回空更新以保持 fail-stop 语义。
            return Vec::new();
        }
        let mut ids = self.take_animation_node_ids();
        ids.retain(|id| !excluded_ids.contains(id));
        let updates = self.update_animation_nodes_at(ids.iter().copied(), now, dt);
        self.animation_ids_scratch = ids;
        updates
    }

    pub(crate) fn take_animation_node_ids(&mut self) -> Vec<WidgetId> {
        let mut ids = std::mem::take(&mut self.animation_ids_scratch);
        ids.clear();
        ids.extend(
            self.traverse()
                .iter()
                .copied()
                .filter(|&id| self.active_animation_frame(id).is_some()),
        );
        ids.extend(
            self.animated_sources
                .iter()
                .filter(|(_, source)| {
                    source.source.registration() != AnimatedRegistration::Inactive
                })
                .map(|(id, _)| *id),
        );
        ids
    }

    pub(crate) fn animated_source_registrations(&self) -> Vec<(WidgetId, Option<Instant>)> {
        // 已停止的树不得登记动画源 deadline。
        if !self.accepts_external_work() {
            // 返回空集合以清除窗口侧动画工作。
            return Vec::new();
        }
        self.animated_sources
            .iter()
            .filter_map(|(&id, source)| match source.source.registration() {
                AnimatedRegistration::Inactive => None,
                AnimatedRegistration::Open => Some((id, None)),
                AnimatedRegistration::Deadline(deadline) => Some((id, Some(deadline))),
            })
            .collect()
    }

    pub(crate) fn active_animation_frame(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        let visible_and_active = self.is_effectively_visible(id) && node.active();
        let view_transition =
            node.view_transition_active() && (node.pending_removal() || visible_and_active);
        let widget_animation = visible_and_active
            && !self.is_pending_removal_subtree(id)
            && node
                .capabilities()
                .contains(crate::ui::widget_runtime::traits::WidgetCapabilities::ANIMATION);
        (view_transition || widget_animation).then_some(node.frame())
    }

    // 测试目标保留活动过渡 id 观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn active_view_transition_ids(&self) -> Vec<WidgetId> {
        self.traverse()
            .iter()
            .copied()
            .filter(|&id| {
                self.get(id)
                    .is_some_and(BoxedWidget::view_transition_active)
                    && (self.get(id).is_some_and(BoxedWidget::pending_removal)
                        || self.is_effectively_visible(id))
            })
            .collect()
    }

    // 测试目标保留过渡注册观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn view_transition_registrations(&self) -> Vec<(WidgetId, Option<Instant>)> {
        let mut registrations = Vec::new();
        self.extend_view_transition_registrations(&mut registrations);
        registrations
    }

    pub(crate) fn extend_view_transition_registrations(
        &self,
        registrations: &mut Vec<(WidgetId, Option<Instant>)>,
    ) {
        // 已停止的树不得向窗口调度器提供过渡 deadline。
        if !self.accepts_external_work() {
            // 保持调用方现有集合不变。
            return;
        }
        registrations.extend(self.traverse().iter().copied().filter_map(|id| {
            let node = self.get(id)?;
            (node.view_transition_active()
                && (node.pending_removal() || self.is_effectively_visible(id)))
            .then_some((id, node.view_transition_deadline()))
        }));
    }

    // 测试目标保留动画节点更新便捷入口，供时间推进测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn update_animation_nodes<I>(&mut self, ids: I, dt: f64) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        self.update_animation_nodes_at(ids, Instant::now(), dt)
    }

    pub(crate) fn update_animation_nodes_at<I>(
        &mut self,
        ids: I,
        now: Instant,
        dt: f64,
    ) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        // 已停止的树不得通过细粒度入口执行动画组件代码。
        if !self.accepts_external_work() {
            // 返回空更新以保持 fail-stop 语义。
            return Vec::new();
        }
        // 生产调度传入的切片迭代器提供精确下界，按本帧身份数一次申请结果区。
        let ids = ids.into_iter();
        let (lower_bound, _) = ids.size_hint();
        let mut updates = Vec::with_capacity(lower_bound);
        self.update_animation_nodes_at_into(ids, now, dt, &mut updates);
        updates
    }

    // 把同一推进语义写入调用方给定的结果区，统一临时与测试复用路径。
    pub(crate) fn update_animation_nodes_at_into<I>(
        &mut self,
        ids: I,
        now: Instant,
        dt: f64,
        updates: &mut Vec<(WidgetId, bool)>,
    ) where
        I: IntoIterator<Item = WidgetId>,
    {
        // 每轮覆盖上一帧结果，但保留调用方已明确持有的容量。
        updates.clear();
        // 已停止的树不得通过结果区入口执行动画组件代码。
        if !self.accepts_external_work() {
            return;
        }
        let mut widget_overlays_changed = false;
        let mut completed_removals = Vec::new();
        for id in ids {
            // 处理下一动画身份前复核树仍处于可接受外部工作的阶段。
            if !self.accepts_external_work() {
                // 前序动画回调停止树时不再推进后续身份。
                break;
            }
            if let Some(source) = self.animated_sources.get(&id) {
                // 保存本次动画源推进结果，以便返回后先复核树状态。
                let still_active = source.source.advance(now, dt);
                // 动画源回调返回后必须阻止停止树继续登记任何后续工作。
                if !self.accepts_external_work() {
                    // 首个停止态直接结束本轮动画源与组件动画遍历。
                    break;
                }
                // 正常运行态才向调度器报告该动画源的续期状态。
                updates.push((id, still_active));
                continue;
            }
            let Some(frame) = self.active_animation_frame(id) else {
                self.active_widget_animations.remove(&id);
                updates.push((id, false));
                continue;
            };

            let modal_was_present = crate::ui::tree_widget_hooks::modal_was_present(self, id);

            let view_was_active = self
                .get(id)
                .is_some_and(BoxedWidget::view_transition_active);
            let view_is_waiting = self
                .get(id)
                .and_then(BoxedWidget::view_transition_deadline)
                .is_some_and(|deadline| deadline > now);
            let old_visual_bounds = (view_was_active && !view_is_waiting)
                .then(|| self.visual_subtree_bounds(id))
                .flatten();
            let (view_still_active, remove_now) = if view_was_active && !view_is_waiting {
                self.get_mut(id)
                    .map_or((false, false), |node| node.advance_view_transition(now, dt))
            } else {
                (false, false)
            };
            // 视图过渡推进返回后复核它没有触发停止树的协调失败。
            if !self.accepts_external_work() {
                // 停止态不得继续计算组件动画或动态子树刷新。
                break;
            }
            let new_visual_bounds = (view_was_active && !view_is_waiting)
                .then(|| self.visual_subtree_bounds(id))
                .flatten();
            for rect in [old_visual_bounds, new_visual_bounds].into_iter().flatten() {
                if rect.w > 0.0 && rect.h > 0.0 {
                    self.push_paint_invalidation(id, Some(rect));
                }
            }

            // 组件动画回调前再次复核树的外部工作准入。
            if !self.accepts_external_work() {
                // 停止态不得进入组件提供的动画实现。
                break;
            }
            let (widget_still_active, dirty) = self
                .get_mut(id)
                .and_then(|node| {
                    let animation = node.widget_mut().as_animation_mut()?;
                    let still_active = animation.update_animation(dt);
                    let dirty = animation.dirty_bounds(frame);
                    Some((still_active, dirty))
                })
                .unwrap_or((false, Rect::zero()));
            // 组件动画回调返回后立即阻断已停止树的剩余处理。
            if !self.accepts_external_work() {
                // 停止态不得触发动态子树协调或继续同级动画。
                break;
            }
            // 动态选择项刷新前复核树仍允许进入协调路径。
            if !self.accepts_external_work() {
                // 停止态不得调用应用提供的动态 renderer。
                break;
            }
            let dynamic_children_changed = self.refresh_select_option_widget(id);
            // 动态刷新可能捕获协调 panic，因此返回后必须复核树状态。
            if !self.accepts_external_work() {
                // 首个停止态不得继续处理当前或后续动画身份。
                break;
            }
            if dynamic_children_changed {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
                self.invalidate_paint(id);
            }
            let animation_layout_requested = self
                .get_mut(id)
                .is_some_and(|node| node.take_layout_request());
            if animation_layout_requested {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
            if widget_still_active {
                self.active_widget_animations.insert(id);
            } else {
                self.active_widget_animations.remove(&id);
            }

            if dirty.w > 0.0 && dirty.h > 0.0 {
                self.invalidate_paint_rect(id, dirty);
            }
            widget_overlays_changed |= !self.widget_overlay_is_current(id);
            updates.push((id, view_still_active || widget_still_active));
            if remove_now {
                completed_removals.push(id);
            }

            let modal_closed_for_destruction = modal_was_present
                && crate::ui::tree_widget_hooks::modal_closed_for_destruction(self, id);
            if modal_closed_for_destruction {
                completed_removals.push(id);
            }
        }
        // 停止树不再执行真实移除或覆盖层重建等后续协调工作。
        if !self.accepts_external_work() {
            // 保留已完成身份的结果供上层撤销后续调度。
            return;
        }
        for id in completed_removals {
            // 销毁下一个完成过渡节点前复核树仍允许外部工作。
            if !self.accepts_external_work() {
                // 前序销毁停止树时不再销毁同级节点。
                break;
            }
            if self.get(id).is_some() {
                self.remove(id);
                widget_overlays_changed = true;
            }
            // 节点销毁生命周期回调返回后复核树状态。
            if !self.accepts_external_work() {
                // 停止态不得继续进入后续节点销毁。
                break;
            }
        }
        // 销毁阶段可能使树停止，因此后续交互清理也必须被阻断。
        if !self.accepts_external_work() {
            // 保留已完成身份的结果供上层撤销后续调度。
            return;
        }
        self.cancel_hidden_interaction();
        if widget_overlays_changed || updates.iter().any(|(_, still_active)| !still_active) {
            self.rebuild_widget_overlays();
        }
    }

    pub(crate) fn widget_animation_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        // 已停止的树不得向调度器暴露活动组件动画。
        self.active_widget_animations
            .iter()
            .copied()
            // 非运行态在迭代层返回空集合，避免扩大内部 API 契约。
            .filter(|id| self.accepts_external_work() && self.get(*id).is_some())
    }

    pub(crate) fn widget_overlay_is_current(&self, id: WidgetId) -> bool {
        let desired = (self.is_effectively_visible(id) && !self.is_pending_removal_subtree(id))
            .then(|| self.get(id))
            .flatten()
            .and_then(|node| node.overlay_entry(id, node.frame()));
        let mut current = self
            .overlay_stack
            .iter()
            .filter(|entry| !entry.is_managed() && entry.owner() == id);

        match (desired, current.next()) {
            (None, None) => true,
            (Some(desired), Some(current_entry)) if current.next().is_none() => {
                desired.kind() == current_entry.kind()
                    && desired.bounds_rect() == current_entry.bounds_rect()
                    && desired.z_index_value() == current_entry.z_index_value()
                    && desired.is_modal() == current_entry.is_modal()
                    && desired.dismisses_on_outside() == current_entry.dismisses_on_outside()
                    && desired.traps_focus() == current_entry.traps_focus()
            }
            _ => false,
        }
    }
}
