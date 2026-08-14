//! 基于现有 `Animated<T>` 源的有限动画组合。

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ui::animation::traits::Animatable;

use super::{Animated, Easing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupItemTiming {
    Finite(Duration),
    Delayed,
    Unbounded,
    Inactive,
}

trait AnimationGroupControl: Send + Sync {
    fn source_id(&self) -> crate::core::ComponentId;
    fn timing(&self) -> GroupItemTiming;
    fn schedule_at(&self, delay: Duration, now: Instant);
    fn pause_at(&self, now: Instant);
    fn resume_at(&self, now: Instant);
    fn restart_at(&self, now: Instant);
    fn stop(&self);
    fn progress(&self) -> f64;
    fn is_finished(&self) -> bool;
}

impl<T> AnimationGroupControl for Animated<T>
where
    T: Animatable + Sync,
{
    fn source_id(&self) -> crate::core::ComponentId {
        self.group_source_id()
    }

    fn timing(&self) -> GroupItemTiming {
        self.group_timing()
    }

    fn schedule_at(&self, delay: Duration, now: Instant) {
        self.group_schedule_at(delay, now);
    }

    fn pause_at(&self, now: Instant) {
        self.group_pause_at(now);
    }

    fn resume_at(&self, now: Instant) {
        self.group_resume_at(now);
    }

    fn restart_at(&self, now: Instant) {
        self.group_restart_at(now);
    }

    fn stop(&self) {
        Animated::stop(self);
    }

    fn progress(&self) -> f64 {
        self.group_progress()
    }

    fn is_finished(&self) -> bool {
        Animated::is_finished(self)
    }
}

/// 被 [`AnimationGroup`] 接受的类型擦除有限 `Animated<T>` 源。
#[derive(Clone)]
pub struct AnimationGroupItem {
    control: Arc<dyn AnimationGroupControl>,
}

impl AnimationGroupItem {
    fn timing(&self) -> GroupItemTiming {
        self.control.timing()
    }
}

impl<T> From<Animated<T>> for AnimationGroupItem
where
    T: Animatable + Sync,
{
    fn from(animated: Animated<T>) -> Self {
        Self {
            control: Arc::new(animated),
        }
    }
}

impl<T> From<&Animated<T>> for AnimationGroupItem
where
    T: Animatable + Sync,
{
    fn from(animated: &Animated<T>) -> Self {
        animated.clone().into()
    }
}

impl fmt::Debug for AnimationGroupItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnimationGroupItem")
            .field("timing", &self.timing())
            .finish_non_exhaustive()
    }
}

/// 构造有限动画组失败。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationGroupError {
    Empty,
    DuplicateItem { first: usize, duplicate: usize },
    InactiveItem(usize),
    DelayedItem(usize),
    UnboundedItem(usize),
}

impl fmt::Display for AnimationGroupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("animation group must not be empty"),
            Self::DuplicateItem { first, duplicate } => write!(
                formatter,
                "animation group item {duplicate} duplicates item {first}"
            ),
            Self::InactiveItem(index) => {
                write!(formatter, "animation group item {index} has no playback")
            }
            Self::DelayedItem(index) => write!(
                formatter,
                "animation group item {index} already has a delay; use AnimationGroup::delay"
            ),
            Self::UnboundedItem(index) => write!(
                formatter,
                "animation group item {index} has no finite duration"
            ),
        }
    }
}

impl Error for AnimationGroupError {}

#[derive(Clone)]
struct ScheduledItem {
    item: AnimationGroupItem,
    offset: Duration,
    duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupSchedule {
    Parallel,
    Sequential,
    Stagger(Duration),
}

/// 对现有有限动画源的并行、串行或交错控制。
///
/// 组自身不注册帧工作。读取组进度会捕获每个子源，调度则复用每个子源
/// 已有的 open 或 deadline 注册。
#[derive(Clone)]
pub struct AnimationGroup {
    items: Vec<ScheduledItem>,
    duration: Duration,
    schedule: GroupSchedule,
}

impl AnimationGroup {
    pub fn parallel(
        items: impl IntoIterator<Item = AnimationGroupItem>,
    ) -> Result<Self, AnimationGroupError> {
        Self::build(items, GroupSchedule::Parallel)
    }

    pub fn sequential(
        items: impl IntoIterator<Item = AnimationGroupItem>,
    ) -> Result<Self, AnimationGroupError> {
        Self::build(items, GroupSchedule::Sequential)
    }

    /// 每个子项在额外一个归一化间隔之后启动。
    pub fn stagger(
        items: impl IntoIterator<Item = AnimationGroupItem>,
        interval: f64,
    ) -> Result<Self, AnimationGroupError> {
        Self::build(items, GroupSchedule::Stagger(normalized_offset(interval)))
    }

    /// 用于串行间隙或末尾停留的有限空操作动画项。
    pub fn delay(duration: f64) -> AnimationGroupItem {
        let duration = if duration.is_finite() {
            duration.max(0.0)
        } else {
            0.0
        };
        Animated::new(0.0_f32)
            .to(1.0, duration, Easing::linear)
            .into()
    }

    fn build(
        items: impl IntoIterator<Item = AnimationGroupItem>,
        schedule: GroupSchedule,
    ) -> Result<Self, AnimationGroupError> {
        let items = items.into_iter().collect::<Vec<_>>();
        // 空组直接拒绝。
        if items.is_empty() {
            return Err(AnimationGroupError::Empty);
        }

        // 同一动画源只能出现一次，重复则报告首尾位置。
        let mut source_indices = HashMap::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            if let Some(first) = source_indices.insert(item.control.source_id(), index) {
                return Err(AnimationGroupError::DuplicateItem {
                    first,
                    duplicate: index,
                });
            }
        }

        // 校验每个子项的时序类型：有限时长可用，其余状态报错。
        let durations = items
            .iter()
            .enumerate()
            .map(|(index, item)| match item.timing() {
                GroupItemTiming::Finite(duration) => Ok(duration),
                GroupItemTiming::Inactive => Err(AnimationGroupError::InactiveItem(index)),
                GroupItemTiming::Delayed => Err(AnimationGroupError::DelayedItem(index)),
                GroupItemTiming::Unbounded => Err(AnimationGroupError::UnboundedItem(index)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let now = Instant::now();
        // 串行模式下用游标累积偏移；总时长取所有子项结束点的最大值。
        let mut cursor = Duration::ZERO;
        let mut total = Duration::ZERO;
        let scheduled = items
            .into_iter()
            .zip(durations)
            .enumerate()
            .map(|(index, (item, duration))| {
                let offset = match schedule {
                    GroupSchedule::Parallel => Duration::ZERO,
                    GroupSchedule::Sequential => cursor,
                    GroupSchedule::Stagger(interval) => {
                        interval.saturating_mul(u32::try_from(index).unwrap_or(u32::MAX))
                    }
                };
                // 立即按偏移调度子源，使其在各自截止时刻启动。
                item.control.schedule_at(offset, now);
                let end = offset.saturating_add(duration);
                total = total.max(end);
                if matches!(schedule, GroupSchedule::Sequential) {
                    cursor = end;
                }
                ScheduledItem {
                    item,
                    offset,
                    duration,
                }
            })
            .collect();

        Ok(Self {
            items: scheduled,
            duration: total,
            schedule,
        })
    }

    /// 总调度时长（秒）。
    pub fn duration(&self) -> f64 {
        self.duration.as_secs_f64()
    }

    /// 组时间线进度。读取时会捕获每个子源。
    pub fn progress(&self) -> f64 {
        // 收集所有子源进度后折算为组时间线上的已过时长。
        let progress = self
            .items
            .iter()
            .map(|scheduled| scheduled.item.control.progress())
            .collect::<Vec<_>>();
        // 零时长组直接视为完成。
        if self.duration.is_zero() {
            return 1.0;
        }

        // 每个子项按其完成/进度推算时间线位置，取最大值作为组已过时长。
        let elapsed =
            self.items
                .iter()
                .zip(progress)
                .fold(0.0_f64, |elapsed, (scheduled, progress)| {
                    let item_elapsed = if scheduled.item.control.is_finished() {
                        scheduled
                            .offset
                            .saturating_add(scheduled.duration)
                            .as_secs_f64()
                    } else if progress > 0.0 {
                        scheduled.offset.as_secs_f64() + scheduled.duration.as_secs_f64() * progress
                    } else {
                        0.0
                    };
                    elapsed.max(item_elapsed)
                });
        (elapsed / self.duration.as_secs_f64()).clamp(0.0, 1.0)
    }

    /// 每个已调度子项（含延迟项、停留项）都结束后返回 true。
    pub fn is_finished(&self) -> bool {
        self.items
            .iter()
            .all(|scheduled| scheduled.item.control.is_finished())
    }

    pub fn pause(&self) {
        let now = Instant::now();
        // 统一暂停全部子源，保持组内相对时序。
        for scheduled in &self.items {
            scheduled.item.control.pause_at(now);
        }
    }

    pub fn resume(&self) {
        let now = Instant::now();
        // 统一恢复全部子源，剩余延迟按暂停余额重建。
        for scheduled in &self.items {
            scheduled.item.control.resume_at(now);
        }
    }

    pub fn restart(&self) {
        let now = Instant::now();
        // 统一重启全部子源，重新执行各自的偏移延迟。
        for scheduled in &self.items {
            scheduled.item.control.restart_at(now);
        }
    }

    pub fn stop(&self) {
        // 统一停止全部子源。
        for scheduled in &self.items {
            scheduled.item.control.stop();
        }
    }
}

impl fmt::Debug for AnimationGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnimationGroup")
            .field("items", &self.items.len())
            .field("duration", &self.duration)
            .field("schedule", &self.schedule)
            .finish()
    }
}

fn normalized_offset(seconds: f64) -> Duration {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Duration::ZERO;
    }
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX)
}
