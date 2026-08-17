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
    managed_animation_registrations: BTreeMap<NodeId, (Option<Instant>, bool)>,
    open_component_animations: BTreeMap<NodeId, bool>,
    managed_timers: BTreeMap<TimerId, bool>,
    managed_app_timers: BTreeMap<TimerId, bool>,
    timer_sync_marker: bool,
    app_timer_sync_marker: bool,
    animated_source_sync_marker: bool,
    component_animation_sync_marker: bool,
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
        self.entries.insert(kind, Some(next_deadline));
    }

    pub(crate) fn register_open(&mut self, kind: ActiveWorkKind) {
        self.entries.insert(kind, None);
    }

    pub(crate) fn unregister(&mut self, kind: ActiveWorkKind) -> bool {
        if let ActiveWorkKind::Animation(id) = kind {
            self.managed_animation_registrations.remove(&id);
            self.open_component_animations.remove(&id);
        }
        if let ActiveWorkKind::Timer(id) = kind {
            self.managed_timers.remove(&id);
        }
        if let ActiveWorkKind::AppTimer(id) = kind {
            self.managed_app_timers.remove(&id);
        }
        self.entries.remove(&kind).is_some()
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
        let open_component_animations = &self.open_component_animations;
        self.managed_animation_registrations
            .retain(|&id, (_, seen_marker)| {
                if *seen_marker == marker {
                    return true;
                }
                let kind = ActiveWorkKind::Animation(id);
                if open_component_animations.contains_key(&id) {
                    entries.insert(kind, None);
                } else {
                    entries.remove(&kind);
                }
                false
            });
        for (&id, &(deadline, _)) in &self.managed_animation_registrations {
            let kind = ActiveWorkKind::Animation(id);
            if self.open_component_animations.contains_key(&id) {
                self.entries.insert(kind, None);
            } else {
                self.entries.insert(kind, deadline);
            }
        }
    }

    pub(crate) fn sync_component_animations<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.component_animation_sync_marker = !self.component_animation_sync_marker;
        let marker = self.component_animation_sync_marker;
        for id in ids {
            self.open_component_animations.insert(id, marker);
        }
        let entries = &mut self.entries;
        let managed_animation_registrations = &self.managed_animation_registrations;
        self.open_component_animations.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                return true;
            }
            let kind = ActiveWorkKind::Animation(id);
            if let Some(&(deadline, _)) = managed_animation_registrations.get(&id) {
                entries.insert(kind, deadline);
            } else {
                entries.remove(&kind);
            }
            false
        });
        for &id in self.open_component_animations.keys() {
            self.entries.insert(ActiveWorkKind::Animation(id), None);
        }
    }

    pub(crate) fn manages_animation(&self, id: NodeId) -> bool {
        self.managed_animation_registrations.contains_key(&id)
            || self.open_component_animations.contains_key(&id)
    }

    pub(crate) fn park_animated_deadlines(&mut self) {
        let managed = &self.managed_animation_registrations;
        for (kind, deadline) in &mut self.entries {
            if matches!(kind, ActiveWorkKind::Animation(id) if managed.contains_key(id)) {
                *deadline = None;
            }
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().filter_map(|deadline| *deadline).min()
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
            false
        });
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
        for (id, delay) in timers {
            self.entries
                .entry(ActiveWorkKind::Timer(id))
                .or_insert(Some(now + delay));
            self.managed_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                entries.remove(&ActiveWorkKind::Timer(id));
                false
            }
        });
    }

    pub(crate) fn sync_app_timers<I>(&mut self, timers: I)
    where
        I: IntoIterator<Item = (TimerId, Instant)>,
    {
        self.app_timer_sync_marker = !self.app_timer_sync_marker;
        let marker = self.app_timer_sync_marker;
        for (id, deadline) in timers {
            self.entries
                .insert(ActiveWorkKind::AppTimer(id), Some(deadline));
            self.managed_app_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_app_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                entries.remove(&ActiveWorkKind::AppTimer(id));
                false
            }
        });
    }
}

// 活动工作公平预算的直接观测只编译进单元测试目标。
#[cfg(test)]
// 测试留在 registry Component 内，避免把私有调度结构暴露给集成测试。
mod tests {
    // 复用活动工作类型与 registry 实现。
    use super::*;

    // 验证两类 timer 各自受预算约束，同时非回调维护工作不被积压阻塞。
    #[test]
    // 执行批量到期工作跨轮保留场景。
    fn due_timer_budget_preserves_remainder_for_the_next_turn() {
        // 使用同一时刻构造全部到期登记。
        let now = Instant::now();
        // 创建一个窗口独占的活动工作注册表。
        let mut registry = ActiveWorkRegistry::new();
        // 登记不应被 timer 预算阻塞的图形维护工作。
        registry.register(ActiveWorkKind::GraphicsMaintenance, now);
        // 为两种 timer 各登记三个到期回调。
        for id in 1..=3 {
            // 登记 Widget timer 到期事实。
            registry.register(ActiveWorkKind::Timer(id), now);
            // 登记 App timer 到期事实。
            registry.register(ActiveWorkKind::AppTimer(id), now);
        }
        // 复用输出缓冲保存本轮取得执行资格的工作。
        let mut due = Vec::new();

        // 第一轮为每种 timer 只提供两个回调预算。
        registry.drain_due_into_with_budget(now, &mut due, 2);
        // 非回调维护工作必须在第一轮被正常消费。
        assert!(due.contains(&ActiveWorkKind::GraphicsMaintenance));
        // 第一轮只允许两个 Widget timer 回调离开 registry。
        assert_eq!(
            due.iter()
                .filter(|work| matches!(work, ActiveWorkKind::Timer(_)))
                .count(),
            2
        );
        // 第一轮只允许两个 App timer 回调离开 registry。
        assert_eq!(
            due.iter()
                .filter(|work| matches!(work, ActiveWorkKind::AppTimer(_)))
                .count(),
            2
        );
        // 两类 timer 的剩余项保持到期 deadline，使下一轮无需外部新 wake。
        assert_eq!(registry.next_deadline(), Some(now));

        // 第二轮继续取得上一轮保留的工作。
        registry.drain_due_into_with_budget(now, &mut due, 2);
        // 第二轮恰好取得一个剩余 Widget timer。
        assert_eq!(
            due.iter()
                .filter(|work| matches!(work, ActiveWorkKind::Timer(_)))
                .count(),
            1
        );
        // 第二轮恰好取得一个剩余 App timer。
        assert_eq!(
            due.iter()
                .filter(|work| matches!(work, ActiveWorkKind::AppTimer(_)))
                .count(),
            1
        );
        // 全部到期工作被两轮精确消费后 registry 为空。
        assert!(registry.is_empty());
    }
}
