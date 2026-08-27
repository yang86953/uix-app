use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::draw::scene::NodeId;

pub(crate) type TimerId = u64;

// 单个窗口轮次最多执行固定数量的每类到期 timer 回调，防止输入与关闭饥饿。
const TIMER_CALLBACK_BUDGET_PER_KIND: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ActiveWorkKind {
    Animation(NodeId),
    Timer(TimerId),
    AppTimer(TimerId),
    ImeSession(NodeId),
    GraphicsMaintenance,
}

#[derive(Debug, Default)]
pub(crate) struct ActiveWorkRegistry {
    entries: BTreeMap<ActiveWorkKind, Option<Instant>>,
    /// `None` 表示待重算，`Some(None)` 表示已确认没有有限 deadline。
    next_deadline_cache: Cell<Option<Option<Instant>>>,
    managed_animation_registrations: BTreeMap<NodeId, (Option<Instant>, bool)>,
    open_widget_animations: BTreeMap<NodeId, bool>,
    managed_timers: BTreeMap<TimerId, bool>,
    managed_app_timers: BTreeMap<TimerId, bool>,
    timer_sync_marker: bool,
    app_timer_sync_marker: bool,
    animated_source_sync_marker: bool,
    widget_animation_sync_marker: bool,
}

impl ActiveWorkRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        // 故障停止时丢弃窗口拥有的全部后续调度登记。
        *self = Self::default();
    }

    pub(crate) fn register(&mut self, kind: ActiveWorkKind, next_deadline: Instant) {
        if self.entries.insert(kind, Some(next_deadline)) != Some(Some(next_deadline)) {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn register_open(&mut self, kind: ActiveWorkKind) {
        if self.entries.insert(kind, None) != Some(None) {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn unregister(&mut self, kind: ActiveWorkKind) -> bool {
        if let ActiveWorkKind::Animation(id) = kind {
            self.managed_animation_registrations.remove(&id);
            self.open_widget_animations.remove(&id);
        }
        if let ActiveWorkKind::Timer(id) = kind {
            self.managed_timers.remove(&id);
        }
        if let ActiveWorkKind::AppTimer(id) = kind {
            self.managed_app_timers.remove(&id);
        }
        let removed = self.entries.remove(&kind).is_some();
        if removed {
            self.invalidate_deadline_cache();
        }
        removed
    }

    pub(crate) fn sync_animated_sources<I>(&mut self, registrations: I)
    where
        I: IntoIterator<Item = (NodeId, Option<Instant>)>,
    {
        self.animated_source_sync_marker = !self.animated_source_sync_marker;
        let marker = self.animated_source_sync_marker;
        for (id, deadline) in registrations {
            self.managed_animation_registrations
                .insert(id, (deadline, marker));
        }
        let entries = &mut self.entries;
        let open_widget_animations = &self.open_widget_animations;
        let mut deadlines_changed = false;
        self.managed_animation_registrations
            .retain(|&id, (_, seen_marker)| {
                if *seen_marker == marker {
                    return true;
                }
                let kind = ActiveWorkKind::Animation(id);
                if open_widget_animations.contains_key(&id) {
                    deadlines_changed |= entries.insert(kind, None) != Some(None);
                } else {
                    deadlines_changed |= entries.remove(&kind).is_some();
                }
                false
            });
        for (&id, &(deadline, _)) in &self.managed_animation_registrations {
            let kind = ActiveWorkKind::Animation(id);
            if self.open_widget_animations.contains_key(&id) {
                deadlines_changed |= self.entries.insert(kind, None) != Some(None);
            } else {
                deadlines_changed |= self.entries.insert(kind, deadline) != Some(deadline);
            }
        }
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn sync_widget_animations<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.widget_animation_sync_marker = !self.widget_animation_sync_marker;
        let marker = self.widget_animation_sync_marker;
        for id in ids {
            self.open_widget_animations.insert(id, marker);
        }
        let entries = &mut self.entries;
        let managed_animation_registrations = &self.managed_animation_registrations;
        let mut deadlines_changed = false;
        self.open_widget_animations.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                return true;
            }
            let kind = ActiveWorkKind::Animation(id);
            if let Some(&(deadline, _)) = managed_animation_registrations.get(&id) {
                deadlines_changed |= entries.insert(kind, deadline) != Some(deadline);
            } else {
                deadlines_changed |= entries.remove(&kind).is_some();
            }
            false
        });
        for &id in self.open_widget_animations.keys() {
            deadlines_changed |=
                self.entries.insert(ActiveWorkKind::Animation(id), None) != Some(None);
        }
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn manages_animation(&self, id: NodeId) -> bool {
        self.managed_animation_registrations.contains_key(&id)
            || self.open_widget_animations.contains_key(&id)
    }

    pub(crate) fn park_animated_deadlines(&mut self) {
        let managed = &self.managed_animation_registrations;
        let mut deadlines_changed = false;
        for (kind, deadline) in &mut self.entries {
            if deadline.is_some()
                && matches!(kind, ActiveWorkKind::Animation(id) if managed.contains_key(id))
            {
                *deadline = None;
                deadlines_changed = true;
            }
        }
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        if let Some(deadline) = self.next_deadline_cache.get() {
            return deadline;
        }
        let deadline = self.entries.values().filter_map(|deadline| *deadline).min();
        self.next_deadline_cache.set(Some(deadline));
        deadline
    }

    fn invalidate_deadline_cache(&self) {
        self.next_deadline_cache.set(None);
    }

    #[cfg(test)]
    pub(crate) fn drain_due(&mut self, now: Instant) -> Vec<ActiveWorkKind> {
        let mut due = Vec::new();
        // 测试便捷入口保持“取出全部到期工作”的既有语义。
        self.drain_due_into_with_budget(now, &mut due, usize::MAX);
        due
    }

    pub(crate) fn drain_due_into(&mut self, now: Instant, due: &mut Vec<ActiveWorkKind>) {
        // 生产路径统一采用逐类固定预算，调用方不能绕过公平调度策略。
        self.drain_due_into_with_budget(now, due, TIMER_CALLBACK_BUDGET_PER_KIND);
    }

    // 测试可注入较小预算精确验证跨轮保留，生产调用只使用固定预算入口。
    fn drain_due_into_with_budget(
        &mut self,
        now: Instant,
        due: &mut Vec<ActiveWorkKind>,
        timer_budget_per_kind: usize,
    ) {
        due.clear();
        // Widget timer 与 App timer 各自拥有预算，避免一种来源长期阻塞另一种来源。
        let mut widget_timer_budget = timer_budget_per_kind;
        // App timer 使用独立预算，同时保持本轮回调总量有界。
        let mut app_timer_budget = timer_budget_per_kind;
        let managed_timers = &mut self.managed_timers;
        let managed_app_timers = &mut self.managed_app_timers;
        let mut deadlines_changed = false;
        self.entries.retain(|kind, deadline| {
            if !deadline.is_some_and(|deadline| deadline <= now) {
                return true;
            }
            // 到期 timer 只在其来源仍有预算时离开 registry。
            let has_callback_budget = match *kind {
                // Widget timer 消费本来源的一项预算。
                ActiveWorkKind::Timer(_) => widget_timer_budget > 0,
                // App timer 消费另一来源的一项预算。
                ActiveWorkKind::AppTimer(_) => app_timer_budget > 0,
                // 动画与图形维护不是用户 timer 回调，不受该预算阻塞。
                _ => true,
            };
            // 预算耗尽的到期 timer 保留原 deadline，下一窗口轮次仍会立即发现。
            if !has_callback_budget {
                // 保留当前登记项，不把未执行回调伪装成已消费。
                return true;
            }
            due.push(*kind);
            match *kind {
                ActiveWorkKind::Timer(id) => {
                    // 当前 Widget timer 已取得本轮执行资格。
                    widget_timer_budget -= 1;
                    managed_timers.remove(&id);
                }
                ActiveWorkKind::AppTimer(id) => {
                    // 当前 App timer 已取得本轮执行资格。
                    app_timer_budget -= 1;
                    managed_app_timers.remove(&id);
                }
                _ => {}
            }
            deadlines_changed = true;
            false
        });
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn animation_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.entries
            .iter()
            .filter_map(|(kind, deadline)| match *kind {
                ActiveWorkKind::Animation(id) if deadline.is_none() => Some(id),
                _ => None,
            })
    }

    pub(crate) fn sync_timers<I>(&mut self, timers: I, now: Instant)
    where
        I: IntoIterator<Item = (TimerId, Duration)>,
    {
        self.timer_sync_marker = !self.timer_sync_marker;
        let marker = self.timer_sync_marker;
        let mut deadlines_changed = false;
        for (id, delay) in timers {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                self.entries.entry(ActiveWorkKind::Timer(id))
            {
                entry.insert(Some(now + delay));
                deadlines_changed = true;
            }
            self.managed_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                deadlines_changed |= entries.remove(&ActiveWorkKind::Timer(id)).is_some();
                false
            }
        });
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }

    pub(crate) fn sync_app_timers<I>(&mut self, timers: I)
    where
        I: IntoIterator<Item = (TimerId, Instant)>,
    {
        self.app_timer_sync_marker = !self.app_timer_sync_marker;
        let marker = self.app_timer_sync_marker;
        let mut deadlines_changed = false;
        for (id, deadline) in timers {
            deadlines_changed |= self
                .entries
                .insert(ActiveWorkKind::AppTimer(id), Some(deadline))
                != Some(Some(deadline));
            self.managed_app_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_app_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                deadlines_changed |= entries.remove(&ActiveWorkKind::AppTimer(id)).is_some();
                false
            }
        });
        if deadlines_changed {
            self.invalidate_deadline_cache();
        }
    }
}
