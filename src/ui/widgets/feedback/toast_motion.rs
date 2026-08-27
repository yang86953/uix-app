//! Message / Notification 共用的逐项动画与到期调度。

// 引入关闭观察器的稳定 key 表。
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
// 引入线程安全关闭观察器存储。
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::core::Point;
use crate::ui::AnimationConfig;
use crate::ui::animation::TransitionPlayer;
use crate::ui::reactive::state::{State, StateSlotId};

// 引入反馈声明关闭原因。
use super::declaration::FeedbackCloseReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToastKey {
    Local(u64),
    External(u64),
}

#[derive(Clone)]
pub(crate) struct ToastRequest<T> {
    key: ToastKey,
    item: T,
    display_duration: Option<Duration>,
}

pub(crate) struct ToastQueue<T> {
    state: State<Vec<ToastRequest<T>>>,
    next_id: Arc<AtomicU64>,
    // 只为声明条目的外部 key 保存关闭观察器。
    close_observers: Arc<Mutex<HashMap<ToastKey, CloseObserver>>>,
}

// 队列关闭观察器在实际移除前接收关闭原因。
type CloseObserver = Arc<dyn Fn(FeedbackCloseReason) + Send + Sync>;

impl<T> Clone for ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            next_id: Arc::clone(&self.next_id),
            // 克隆句柄继续共享同一组声明关闭观察器。
            close_observers: Arc::clone(&self.close_observers),
        }
    }
}

impl<T> Default for ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            state: State::new(Vec::new()),
            next_id: Arc::new(AtomicU64::new(1)),
            // 新队列没有声明关闭观察器。
            close_observers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn push(&self, item: T, duration_ms: u64) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.state.update(|items| {
            items.push(ToastRequest {
                key: ToastKey::Local(id),
                item,
                display_duration: display_duration(duration_ms),
            });
        });
        id
    }

    pub(crate) fn push_external(&self, id: u64, item: T, duration_ms: u64) {
        // 普通外部条目不得继承同 ID 的旧声明观察器。
        self.close_observers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&ToastKey::External(id));
        // 写入或替换普通外部条目。
        self.update_external(id, item, duration_ms);
    }

    // 首次写入带关闭观察器的声明条目。
    pub(crate) fn push_external_with_close(
        &self,
        id: u64,
        item: T,
        duration_ms: u64,
        observer: CloseObserver,
    ) {
        // 观察器与外部稳定 key 使用同一身份。
        self.close_observers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(ToastKey::External(id), observer);
        // 写入初始声明条目。
        self.update_external(id, item, duration_ms);
    }

    // 幂等更新外部条目并保留既有关闭观察器。
    pub(crate) fn update_external(&self, id: u64, item: T, duration_ms: u64) {
        self.state.update(|items| {
            items.retain(|request| request.key != ToastKey::External(id));
            items.push(ToastRequest {
                key: ToastKey::External(id),
                item,
                display_duration: display_duration(duration_ms),
            });
        });
    }

    pub(crate) fn replace_external<I>(&self, items: I)
    where
        I: IntoIterator<Item = (u64, T, u64)>,
    {
        self.state.set(
            items
                .into_iter()
                .map(|(id, item, duration_ms)| ToastRequest {
                    key: ToastKey::External(id),
                    item,
                    display_duration: display_duration(duration_ms),
                })
                .collect(),
        );
    }

    pub(crate) fn remove_local(&self, id: u64) -> bool {
        // Rust API 主动关闭必须产生 Programmatic 原因。
        self.close_keys(&[ToastKey::Local(id)], FeedbackCloseReason::Programmatic) > 0
    }

    pub(crate) fn remove_external(&self, id: u64) -> bool {
        // Rust API 主动关闭必须产生 Programmatic 原因。
        self.close_keys(&[ToastKey::External(id)], FeedbackCloseReason::Programmatic) > 0
    }

    // 声明卸载时释放条目但不产生关闭事实。
    pub(crate) fn release_external(&self, id: u64) -> bool {
        // 复用无事件移除路径。
        self.remove_keys(&[ToastKey::External(id)]) > 0
    }

    // 按实际原因关闭指定条目并通知声明 owner。
    pub(crate) fn close_keys(&self, keys: &[ToastKey], reason: FeedbackCloseReason) -> usize {
        // 先复制观察器，避免在锁内执行 owner 或用户回调。
        let observers = {
            // 短暂读取观察器表。
            let observers = self
                .close_observers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // 只复制本次关闭 key 的回调。
            keys.iter()
                .filter_map(|key| observers.get(key).cloned())
                .collect::<Vec<_>>()
        };
        // 在移除前发布真实关闭原因，让 owner 建立 tombstone。
        for observer in observers {
            // 每个声明 key 至多有一个观察器。
            observer(reason);
        }
        // 最后从可见队列与观察器表移除条目。
        self.remove_keys(keys)
    }

    pub(crate) fn retain_latest(&self, maximum: usize) {
        let mut items = self.state.get();
        let excess = items.len().saturating_sub(maximum);
        if excess > 0 {
            items.drain(0..excess);
            self.state.set(items);
        }
    }

    pub(crate) fn remove_keys(&self, keys: &[ToastKey]) -> usize {
        if keys.is_empty() {
            return 0;
        }
        let mut items = self.state.get();
        let before = items.len();
        items.retain(|request| !keys.contains(&request.key));
        let removed = before.saturating_sub(items.len());
        if removed > 0 {
            self.state.set(items);
            // 可见条目移除后同步清理声明观察器。
            let mut observers = self
                .close_observers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // 只删除实际请求的稳定 key。
            for key in keys {
                // 不保留不可达回调。
                observers.remove(key);
            }
        }
        removed
    }

    pub(crate) fn clear(&self) {
        // 清空属于程序化关闭，声明条目必须收到事实。
        let keys = self
            .state
            .get()
            .into_iter()
            .map(|request| request.key)
            .collect::<Vec<_>>();
        // 空队列保持幂等。
        if !keys.is_empty() {
            // 按统一原因关闭全部条目。
            self.close_keys(&keys, FeedbackCloseReason::Programmatic);
        }
    }

    pub(crate) fn values(&self) -> Vec<T> {
        self.state
            .get()
            .into_iter()
            .map(|request| request.item)
            .collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.state.get().len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn generation(&self) -> u64 {
        self.state.generation()
    }

    pub(crate) fn slot_id(&self) -> StateSlotId {
        self.state.slot_id()
    }

    fn requests(&self) -> Vec<ToastRequest<T>> {
        self.state.get()
    }
}

fn display_duration(duration_ms: u64) -> Option<Duration> {
    (duration_ms > 0).then(|| Duration::from_millis(duration_ms))
}

pub(crate) struct ToastMotion<T> {
    entries: Vec<ToastMotionEntry<T>>,
    observed_slot: Option<StateSlotId>,
    observed_generation: u64,
    timer_id: u32,
}

impl<T> Default for ToastMotion<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            observed_slot: None,
            observed_generation: 0,
            timer_id: 1,
        }
    }
}

impl<T> ToastMotion<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(crate) fn sync(
        &mut self,
        queue: &ToastQueue<T>,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
    ) -> bool {
        let slot = queue.slot_id();
        let generation = queue.generation();
        // 即使代次未变，渲染阶段也必须读取 State，以续订当前组件的窄脏区绑定。
        let requests = queue.requests();
        if self.observed_slot == Some(slot) && self.observed_generation == generation {
            return false;
        }

        let mut changed = false;
        let mut timer_changed = false;

        for entry in &mut self.entries {
            if let Some(request) = requests.iter().find(|request| request.key == entry.key) {
                // 内容与外观更新不改变现有计时进度。
                entry.item = request.item.clone();
                // 新 generation 中的条目配置需要重绘。
                changed = true;
                // 只有 duration 变化才从提交点重启 Holding 计时。
                if entry.display_duration != request.display_duration {
                    // 保存新的展示时长。
                    entry.display_duration = request.display_duration;
                    // 已进入 Holding 时立即用新时长重启截止计时。
                    if let ToastPhase::Holding { remaining } = &mut entry.phase {
                        // 零时长映射为无自动关闭截止点。
                        *remaining = request.display_duration;
                        // 通知 active_timer 刷新稳定 timer id。
                        timer_changed = true;
                    }
                }
                if entry.is_leaving() {
                    entry.phase = ToastPhase::Entering(TransitionPlayer::new(enter_animation));
                    changed = true;
                }
            } else if !entry.is_leaving() {
                timer_changed |= entry.is_holding_with_deadline();
                entry.phase = ToastPhase::Leaving(TransitionPlayer::new(leave_animation));
                changed = true;
            }
        }

        for request in requests {
            if self.entries.iter().any(|entry| entry.key == request.key) {
                continue;
            }
            self.entries.push(ToastMotionEntry {
                key: request.key,
                item: request.item,
                display_duration: request.display_duration,
                phase: ToastPhase::Entering(TransitionPlayer::new(enter_animation)),
            });
            changed = true;
        }

        self.observed_slot = Some(slot);
        self.observed_generation = generation;
        if timer_changed {
            self.bump_timer_id();
        }
        changed
    }

    pub(crate) fn update(&mut self, dt: f64) -> ToastMotionUpdate {
        let previous_len = self.entries.len();
        let mut changed = false;
        let mut timer_changed = false;

        for entry in &mut self.entries {
            match &mut entry.phase {
                ToastPhase::Entering(player) => {
                    player.update(dt);
                    changed = true;
                    if player.finished {
                        entry.phase = ToastPhase::Holding {
                            remaining: entry.display_duration,
                        };
                        timer_changed |= entry.display_duration.is_some();
                    }
                }
                ToastPhase::Holding { .. } => {}
                ToastPhase::Leaving(player) => {
                    player.update(dt);
                    changed = true;
                }
            }
        }

        let before_retain = self.entries.len();
        self.entries.retain(|entry| !entry.leave_finished());
        if self.entries.len() != before_retain {
            changed = true;
        }
        if timer_changed {
            self.bump_timer_id();
        }

        ToastMotionUpdate {
            active: self.entries.iter().any(ToastMotionEntry::is_animating),
            changed,
            previous_len,
            current_len: self.entries.len(),
        }
    }

    pub(crate) fn fire_timer(
        &mut self,
        timer_id: u32,
        leave_animation: AnimationConfig,
    ) -> Vec<ToastKey> {
        if timer_id != self.timer_id {
            return Vec::new();
        }
        let Some(elapsed) = self.next_delay() else {
            return Vec::new();
        };

        let mut expired = Vec::new();
        for entry in &mut self.entries {
            let ToastPhase::Holding {
                remaining: Some(remaining),
            } = &mut entry.phase
            else {
                continue;
            };
            if *remaining <= elapsed {
                expired.push(entry.key);
                entry.phase = ToastPhase::Leaving(TransitionPlayer::new(leave_animation));
            } else {
                *remaining = remaining.saturating_sub(elapsed);
            }
        }
        self.bump_timer_id();
        expired
    }

    pub(crate) fn active_timer(&self) -> Option<(u64, Duration)> {
        self.next_delay()
            .map(|delay| (u64::from(self.timer_id), delay))
    }

    pub(crate) fn entries(&self) -> &[ToastMotionEntry<T>] {
        &self.entries
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn next_delay(&self) -> Option<Duration> {
        self.entries
            .iter()
            .filter_map(|entry| match entry.phase {
                ToastPhase::Holding {
                    remaining: Some(remaining),
                } => Some(remaining),
                _ => None,
            })
            .min()
    }

    fn bump_timer_id(&mut self) {
        self.timer_id = self.timer_id.wrapping_add(1).max(1);
    }
}

pub(crate) struct ToastMotionUpdate {
    pub(crate) active: bool,
    pub(crate) changed: bool,
    pub(crate) previous_len: usize,
    pub(crate) current_len: usize,
}

pub(crate) struct ToastMotionEntry<T> {
    key: ToastKey,
    item: T,
    display_duration: Option<Duration>,
    phase: ToastPhase,
}

impl<T> ToastMotionEntry<T> {
    pub(crate) fn key(&self) -> ToastKey {
        self.key
    }

    pub(crate) fn item(&self) -> &T {
        &self.item
    }

    pub(crate) fn opacity(&self) -> f32 {
        self.player().map_or(1.0, |player| player.opacity_progress)
    }

    pub(crate) fn offset(&self) -> Point {
        self.player()
            .map_or_else(|| Point::new(0.0, 0.0), |player| player.offset)
    }

    pub(crate) fn scale(&self) -> f32 {
        self.player().map_or(1.0, |player| player.scale)
    }

    pub(crate) fn offset_endpoints(&self) -> (Point, Point) {
        self.player().map_or(
            (Point::new(0.0, 0.0), Point::new(0.0, 0.0)),
            TransitionPlayer::offset_endpoints,
        )
    }

    pub(crate) fn is_leaving(&self) -> bool {
        matches!(self.phase, ToastPhase::Leaving(_))
    }

    fn player(&self) -> Option<&TransitionPlayer> {
        match &self.phase {
            ToastPhase::Entering(player) | ToastPhase::Leaving(player) => Some(player),
            ToastPhase::Holding { .. } => None,
        }
    }

    fn is_holding_with_deadline(&self) -> bool {
        matches!(self.phase, ToastPhase::Holding { remaining: Some(_) })
    }

    fn is_animating(&self) -> bool {
        matches!(self.phase, ToastPhase::Entering(_) | ToastPhase::Leaving(_))
    }

    fn leave_finished(&self) -> bool {
        matches!(&self.phase, ToastPhase::Leaving(player) if player.finished)
    }
}

enum ToastPhase {
    Entering(TransitionPlayer),
    Holding { remaining: Option<Duration> },
    Leaving(TransitionPlayer),
}
