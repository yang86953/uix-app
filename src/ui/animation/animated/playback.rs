//! 动画播放状态机（单值过渡、循环、延迟）。

use std::fmt;
use std::time::{Duration, Instant};

use crate::ui::animation::traits::Animatable;
use crate::ui::animation::{
    Animation, KeyframeAnimation, KeyframeDirection, KeyframeFillMode, KeyframePlayback,
    SpringAnimation,
};

use super::AnimatedRegistration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoopMode {
    Once,
    Count(u64),
    Forever,
    Alternate,
    AlternateCount(u64),
}

#[derive(Clone, Copy, Debug)]
enum DelayState {
    None,
    Waiting(Instant),
    Paused(Duration),
}

#[derive(Debug)]
pub(super) enum AnimatedMotion<T: Animatable> {
    Timed(Animation<T>),
    Keyframes(KeyframeAnimation<T>),
    Spring(SpringAnimation<T>),
}

impl<T: Animatable> AnimatedMotion<T> {
    pub(super) fn value(&self) -> T {
        match self {
            Self::Timed(animation) => animation.value(),
            Self::Keyframes(animation) => animation.value(),
            Self::Spring(animation) => animation.value(),
        }
    }

    fn update(&mut self, dt: f64) -> T {
        match self {
            Self::Timed(animation) => animation.update(dt),
            Self::Keyframes(animation) => animation.update(dt),
            Self::Spring(animation) => animation.update(dt),
        }
    }

    pub(super) fn progress(&self) -> f64 {
        match self {
            Self::Timed(animation) => animation.progress(),
            Self::Keyframes(animation) => animation.progress(),
            Self::Spring(animation) => animation.progress(),
        }
    }

    fn is_running(&self) -> bool {
        match self {
            Self::Timed(animation) => animation.running,
            Self::Keyframes(animation) => animation.is_running(),
            Self::Spring(animation) => animation.is_running(),
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        match self {
            Self::Timed(animation) => animation.is_finished(),
            Self::Keyframes(animation) => animation.is_finished(),
            Self::Spring(animation) => animation.is_finished(),
        }
    }

    fn pause(&mut self) {
        match self {
            Self::Timed(animation) => animation.pause(),
            Self::Keyframes(animation) => animation.pause(),
            Self::Spring(animation) => animation.pause(),
        }
    }

    fn resume(&mut self) {
        match self {
            Self::Timed(animation) => animation.resume(),
            Self::Keyframes(animation) => animation.resume(),
            Self::Spring(animation) => animation.resume(),
        }
    }

    pub(super) fn stop(&mut self) {
        match self {
            Self::Timed(animation) => animation.stop(),
            Self::Keyframes(animation) => animation.stop(),
            Self::Spring(animation) => animation.stop(),
        }
    }

    fn restart(&mut self) {
        match self {
            Self::Timed(animation) => animation.restart(),
            Self::Keyframes(animation) => animation.restart(),
            Self::Spring(animation) => animation.restart(),
        }
    }

    fn reverse(&mut self) {
        match self {
            Self::Timed(animation) => animation.reverse(),
            Self::Keyframes(animation) => animation.reverse(),
            Self::Spring(animation) => animation.reverse(),
        }
    }

    fn supports_loops(&self) -> bool {
        // 定时与关键帧动画都具有可重启的固定单轮时长。
        matches!(self, Self::Timed(_) | Self::Keyframes(_))
    }
}

pub(super) struct AnimatedPlayback<T: Animatable> {
    pub(super) motion: AnimatedMotion<T>,
    original_from: T,
    original_to: T,
    pub(super) loop_mode: LoopMode,
    completed_plays: u64,
    pub(super) delay: Duration,
    delay_state: DelayState,
    finish_callback: Option<Box<dyn FnOnce() + Send + 'static>>,
    // 保存不应用关键帧填充时恢复的调用方基础值。
    underlying: T,
    // 保存关键帧声明是否在延迟期显示首帧。
    fill_backwards: bool,
    // 保存关键帧声明是否在完成后保留终帧。
    fill_forwards: bool,
    // 保存整个播放序列的基础倒放方向。
    base_reversed: bool,
}

impl<T: Animatable> AnimatedPlayback<T> {
    pub(super) fn new(
        animation: Animation<T>,
        loop_mode: LoopMode,
        delay: Duration,
        now: Instant,
    ) -> Self {
        let mut playback = Self {
            // 普通定时动画的基础值是原始起点。
            underlying: animation.from,
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Timed(animation),
            loop_mode,
            completed_plays: 0,
            delay,
            delay_state: DelayState::None,
            finish_callback: None,
            // 普通定时动画没有 CSS 延迟填充差异。
            fill_backwards: true,
            // 普通定时动画保持既有终值行为。
            fill_forwards: true,
            // 普通定时动画初始正放。
            base_reversed: false,
        };
        playback.arm_delay(now);
        playback
    }

    pub(super) fn new_spring(animation: SpringAnimation<T>) -> Self {
        Self {
            // 弹簧动画的基础值是原始起点。
            underlying: animation.from,
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Spring(animation),
            loop_mode: LoopMode::Once,
            completed_plays: 0,
            delay: Duration::ZERO,
            delay_state: DelayState::None,
            finish_callback: None,
            // 弹簧没有关键帧延迟填充差异。
            fill_backwards: true,
            // 弹簧保持既有终值行为。
            fill_forwards: true,
            // 弹簧初始正放。
            base_reversed: false,
        }
    }

    pub(super) fn new_keyframes(
        mut animation: KeyframeAnimation<T>,
        underlying: T,
        options: KeyframePlayback,
        now: Instant,
    ) -> Self {
        // 反向与交替反向都从倒放轮次开始。
        let base_reversed = matches!(
            options.direction,
            KeyframeDirection::Reverse | KeyframeDirection::AlternateReverse
        );
        // 首轮在构造期就切到声明方向。
        animation.seek(0.0, base_reversed);
        // 判断是否每轮交替方向。
        let alternate = matches!(
            options.direction,
            KeyframeDirection::Alternate | KeyframeDirection::AlternateReverse
        );
        // 把有限、无限与交替组合映射到统一循环状态机。
        let loop_mode = match (options.iterations, alternate) {
            // 单轮无需循环分支。
            (Some(1), _) => LoopMode::Once,
            // 普通有限轮次复用计数循环。
            (Some(count), false) => LoopMode::Count(count),
            // 交替有限轮次保留奇偶方向。
            (Some(count), true) => LoopMode::AlternateCount(count),
            // 普通无限轮次持续正向或反向重播。
            (None, false) => LoopMode::Forever,
            // 交替无限轮次持续来回播放。
            (None, true) => LoopMode::Alternate,
        };
        // 填充策略决定延迟期是否显示首帧。
        let fill_backwards = matches!(
            options.fill_mode,
            KeyframeFillMode::Backwards | KeyframeFillMode::Both
        );
        // 填充策略决定完成后是否保留终帧。
        let fill_forwards = matches!(
            options.fill_mode,
            KeyframeFillMode::Forwards | KeyframeFillMode::Both
        );
        // 从规范化关键帧读取正向两端。
        let original_from = animation.frames()[0].value;
        // 读取最后一个规范化帧作为正向终点。
        let original_to = animation.frames()[animation.frames().len() - 1].value;
        // 构造统一播放状态。
        let mut playback = Self {
            // 保存关键帧外部基础值。
            underlying,
            // 保存正向起点。
            original_from,
            // 保存正向终点。
            original_to,
            // 交给统一 motion 分派推进。
            motion: AnimatedMotion::Keyframes(animation),
            // 保存由迭代与方向组合出的循环模式。
            loop_mode,
            // 新播放尚未完成任何轮次。
            completed_plays: 0,
            // 统一归一化非有限或负延迟。
            delay: normalized_delay(options.delay),
            // 构造后由统一入口武装 deadline。
            delay_state: DelayState::None,
            // 新播放没有完成回调。
            finish_callback: None,
            // 保存延迟期填充事实。
            fill_backwards,
            // 保存完成后填充事实。
            fill_forwards,
            // 保存首轮基础方向。
            base_reversed,
        };
        // 使用所属窗口当前时刻登记可休眠延迟。
        playback.arm_delay(now);
        // 零轮迭代必须立即进入完成态，避免留下不会续帧的半活跃播放。
        if matches!(
            playback.loop_mode,
            LoopMode::Count(0) | LoopMode::AlternateCount(0)
        ) {
            // 统一完成路径会按 fill-mode 选择最终可见值。
            playback.finish_forward(0);
            // 零轮播放不保留启动延迟。
            playback.delay_state = DelayState::None;
        }
        // 返回完整关键帧播放状态。
        playback
    }

    fn is_active(&self) -> bool {
        matches!(self.delay_state, DelayState::None)
            && self.motion.is_running()
            && !matches!(
                self.loop_mode,
                LoopMode::Count(0) | LoopMode::AlternateCount(0)
            )
            && !self.motion.is_finished()
    }

    pub(super) fn registration(&self) -> AnimatedRegistration {
        // 等待延迟期 → 有截止时刻；运行中 → 开放注册；否则视为不活跃。
        match self.delay_state {
            DelayState::Waiting(deadline) => AnimatedRegistration::Deadline(deadline),
            DelayState::None if self.is_active() => AnimatedRegistration::Open,
            DelayState::None | DelayState::Paused(_) => AnimatedRegistration::Inactive,
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        matches!(self.delay_state, DelayState::None) && self.motion.is_finished()
    }

    pub(super) fn progress(&self) -> f64 {
        // 延迟等待或暂停期间进度恒为 0，只反映真实播放进度。
        match self.delay_state {
            DelayState::None => self.motion.progress(),
            DelayState::Waiting(_) | DelayState::Paused(_) => 0.0,
        }
    }

    pub(super) fn group_progress(&self) -> f64 {
        // 组进度用于动画组编排：把已播轮次与当前轮次折算到 [0, 1]。
        if !matches!(self.delay_state, DelayState::None) {
            return 0.0;
        }
        match self.loop_mode {
            LoopMode::Count(0) | LoopMode::AlternateCount(0) => 1.0,
            LoopMode::Count(total) | LoopMode::AlternateCount(total) => {
                if self.completed_plays >= total && self.motion.is_finished() {
                    1.0
                } else {
                    ((self.completed_plays as f64 + self.motion.progress()) / total as f64)
                        .clamp(0.0, 1.0)
                }
            }
            LoopMode::Once | LoopMode::Forever | LoopMode::Alternate => self.motion.progress(),
        }
    }

    pub(super) fn advance(&mut self, now: Instant, dt: f64) -> (Option<T>, bool) {
        let mut dt = dt;
        // 先处理延迟状态机：等待未到期 → 本帧不动；到期 → 转入播放并追补时间差。
        match self.delay_state {
            DelayState::Waiting(deadline) if deadline > now => return (None, false),
            DelayState::Waiting(deadline) => {
                self.delay_state = DelayState::None;
                self.motion.resume();
                if self.motion.is_finished() {
                    self.finish_forward(self.completed_plays);
                    // 零时长完成同样必须立即应用 fill-mode。
                    return (Some(self.value()), false);
                }
                dt = now.saturating_duration_since(deadline).as_secs_f64();
            }
            DelayState::Paused(_) => return (None, false),
            DelayState::None => {}
        }
        // 暂停/未运行/计数为零时不再推进。
        if !self.is_active() {
            return (None, false);
        }
        // 非有限或非正的时间步长不做推进（但仍保留活跃标记）。
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        if dt <= 0.0 {
            return (None, true);
        }
        // 按循环模式分派推进策略。
        let (value, active) = match self.loop_mode {
            LoopMode::Once => self.advance_once(dt),
            LoopMode::Count(total_plays) => self.advance_counted(dt, total_plays, false),
            LoopMode::AlternateCount(total_plays) => {
                // 有限交替迭代按奇偶轮次切换方向。
                self.advance_counted(dt, total_plays, true)
            }
            LoopMode::Forever => self.advance_repeating(dt, false),
            LoopMode::Alternate => self.advance_repeating(dt, true),
        };
        // 完成帧必须立即应用 fill-mode，而不是短暂发布底层终值。
        if !active {
            // 返回完成后的最终可见值。
            return (Some(self.value()), false);
        }
        // 活跃播放直接发布本轮采样值。
        (Some(value), true)
    }

    fn advance_once(&mut self, dt: f64) -> (T, bool) {
        // 单次播放：直接推进底层动画并回报是否仍在运行。
        let value = self.motion.update(dt);
        let active = self.motion.is_running() && !self.motion.is_finished();
        (value, active)
    }

    fn advance_counted(&mut self, dt: f64, total_plays: u64, alternate: bool) -> (T, bool) {
        // 计数循环只对固定时长动画生效，弹簧退化回单次播放。
        if !self.motion.supports_loops() {
            return self.advance_once(dt);
        }
        let remaining_plays = total_plays.saturating_sub(self.completed_plays);
        if remaining_plays == 0 {
            self.finish_forward(total_plays);
            return (self.motion.value(), false);
        }

        let (duration, elapsed) = match &self.motion {
            AnimatedMotion::Timed(animation) => (animation.duration, animation.elapsed + dt),
            AnimatedMotion::Keyframes(animation) => {
                // 关键帧使用同一轮次时长与已用时计算跨帧轮数。
                (animation.duration(), animation.elapsed() + dt)
            }
            AnimatedMotion::Spring(_) => {
                return self.advance_once(dt);
            }
        };
        // 剩余播放次数全部完成：直接定格在终点并结束。
        if elapsed >= duration * remaining_plays as f64 {
            self.finish_forward(total_plays);
            return (self.motion.value(), false);
        }

        // 计算本帧越过的完整轮次，并重置到当前轮次对应的阶段位置。
        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        // 有限交替模式按当前完成轮次决定下一轮方向。
        let reverse_leg = alternate && self.completed_plays % 2 == 1;
        // 把剩余时间定位到当前轮次。
        self.set_leg(reverse_leg, elapsed % duration);
        (self.motion.value(), true)
    }

    fn advance_repeating(&mut self, dt: f64, alternate: bool) -> (T, bool) {
        let (duration, elapsed) = match &self.motion {
            AnimatedMotion::Timed(animation) => (animation.duration, animation.elapsed + dt),
            AnimatedMotion::Keyframes(animation) => {
                // 关键帧使用同一轮次时长与已用时计算跨帧轮数。
                (animation.duration(), animation.elapsed() + dt)
            }
            AnimatedMotion::Spring(_) => {
                return self.advance_once(dt);
            }
        };
        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        // 交替模式下奇偶轮次互换起点与终点，形成来回摆动。
        let reverse_leg = alternate && self.completed_plays % 2 == 1;
        if reverse_leg {
            self.set_leg(true, elapsed % duration);
        } else {
            self.set_leg(false, elapsed % duration);
        }
        (self.motion.value(), true)
    }

    fn set_leg(&mut self, alternate_reversed: bool, elapsed: f64) {
        // 基础倒放与交替轮次按异或组合最终方向。
        let reversed = self.base_reversed ^ alternate_reversed;
        // 按 motion 类型原位定位当前轮次。
        match &mut self.motion {
            // 定时动画直接改写阶段端点与已耗时。
            AnimatedMotion::Timed(animation) => {
                // 倒放轮次互换原始端点。
                let (from, to) = if reversed {
                    // 返回倒放端点。
                    (self.original_to, self.original_from)
                } else {
                    // 返回正放端点。
                    (self.original_from, self.original_to)
                };
                // 保存本轮起点。
                animation.from = from;
                // 保存本轮终点。
                animation.to = to;
                // 定位本轮时间。
                animation.elapsed = elapsed;
                // 跨轮后继续运行。
                animation.running = true;
            }
            // 关键帧原位定位时间并设置方向。
            AnimatedMotion::Keyframes(animation) => animation.seek(elapsed, reversed),
            // 弹簧不支持循环定位。
            AnimatedMotion::Spring(_) => {}
        }
    }

    fn finish_forward(&mut self, completed_plays: u64) {
        // 有限交替播放按最后一轮奇偶性决定终点方向。
        let alternate_reversed = matches!(self.loop_mode, LoopMode::AlternateCount(_))
            // 至少完成一轮时才存在最后一轮方向。
            && completed_plays > 0
            // 第二、第四等偶数轮为反向轮次。
            && (completed_plays - 1) % 2 == 1;
        // 先定位到真实最后一轮终点。
        self.set_leg(alternate_reversed, self.motion_duration());
        // 停止底层推进并保留刚才定位的方向。
        match &mut self.motion {
            AnimatedMotion::Timed(animation) => {
                // 当前轮次已经由 set_leg 定位到末端。
                animation.running = false;
            }
            AnimatedMotion::Keyframes(animation) => {
                // seek 已定位到末端，只需停止继续推进。
                animation.pause();
            }
            AnimatedMotion::Spring(animation) => animation.stop(),
        }
        self.completed_plays = completed_plays;
    }

    // 返回当前 motion 的单轮规范化时长。
    fn motion_duration(&self) -> f64 {
        // 按 motion 类型读取稳定时长。
        match &self.motion {
            // 定时动画公开保存秒级时长。
            AnimatedMotion::Timed(animation) => animation.duration,
            // 关键帧通过只读入口暴露规范化时长。
            AnimatedMotion::Keyframes(animation) => animation.duration(),
            // 弹簧没有固定单轮时长，完成定位交给自身 stop。
            AnimatedMotion::Spring(_) => 0.0,
        }
    }

    pub(super) fn restart(&mut self, now: Instant) {
        self.completed_plays = 0;
        if matches!(
            self.loop_mode,
            LoopMode::Count(0) | LoopMode::AlternateCount(0)
        ) {
            // 计数为零直接进入完成态。
            self.finish_forward(0);
        } else {
            // 支持循环的动画回到起点并重新武装延迟，其余类型直接重启。
            if self.motion.supports_loops() {
                self.set_leg(false, 0.0);
            } else {
                self.motion.restart();
            }
            self.arm_delay(now);
        }
    }

    pub(super) fn reverse(&mut self, now: Instant) {
        // 切换整个播放序列的基础方向。
        self.base_reversed = !self.base_reversed;
        self.completed_plays = 0;
        if self.motion.supports_loops() {
            self.set_leg(false, 0.0);
        } else {
            self.motion.reverse();
        }
        self.arm_delay(now);
    }

    pub(super) fn configure_loop(&mut self, loop_mode: LoopMode, now: Instant) -> Option<T> {
        // 非定时动画不支持循环配置。
        if !self.motion.supports_loops() {
            return None;
        }
        self.loop_mode = loop_mode;
        self.completed_plays = 0;
        // 配置为不播时立即定格为终点值。
        if matches!(loop_mode, LoopMode::Count(0) | LoopMode::AlternateCount(0)) {
            self.delay_state = DelayState::None;
            self.finish_forward(0);
            return Some(self.motion.value());
        }
        // 已结束的播放切到新循环模式时自动从头重播。
        if matches!(self.delay_state, DelayState::None) && self.motion.is_finished() {
            self.restart(now);
            return Some(self.motion.value());
        }
        None
    }

    pub(super) fn pause(&mut self, now: Instant) {
        // 延迟等待中的暂停：把剩余等待时长折算为暂停余额。
        if let DelayState::Waiting(deadline) = self.delay_state {
            self.delay_state = DelayState::Paused(deadline.saturating_duration_since(now));
        }
        self.motion.pause();
    }

    pub(super) fn resume(&mut self, now: Instant) {
        // 从暂停余额恢复：按当前时刻重建新的等待截止点。
        if let DelayState::Paused(remaining) = self.delay_state {
            self.delay_state = DelayState::Waiting(deadline_after(now, remaining));
        }
        self.motion.resume();
    }

    pub(super) fn stop(&mut self) {
        // 停止：清除延迟状态并停住底层动画。
        self.delay_state = DelayState::None;
        self.motion.stop();
    }

    pub(super) fn value(&self) -> T {
        // 延迟期未声明 backwards 时显示调用方基础值。
        if matches!(
            self.delay_state,
            DelayState::Waiting(_) | DelayState::Paused(_)
        ) && !self.fill_backwards
        {
            // 返回未应用动画的基础值。
            return self.underlying;
        }
        // 完成后未声明 forwards 时恢复调用方基础值。
        if self.is_finished() && !self.fill_forwards {
            // 返回未应用动画的基础值。
            return self.underlying;
        }
        // 其余阶段返回当前动画采样值。
        self.motion.value()
    }

    fn arm_delay(&mut self, now: Instant) {
        // 无延迟直接进入播放态，否则登记等待截止时刻。
        self.delay_state = if self.delay.is_zero() {
            DelayState::None
        } else {
            DelayState::Waiting(deadline_after(now, self.delay))
        };
    }

    pub(super) fn replace_finish_callback(
        &mut self,
        callback: Box<dyn FnOnce() + Send + 'static>,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        // 已完成的播放立即触发新回调，否则替换待触发回调并返回旧值。
        self.finish_callback = None;
        if self.is_finished() {
            Some(callback)
        } else {
            self.finish_callback = Some(callback);
            None
        }
    }

    pub(super) fn take_finished_callback(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        // 仅在播放已完成时取出回调，避免提前触发。
        if self.is_finished() {
            self.finish_callback.take()
        } else {
            None
        }
    }
}

impl<T> fmt::Debug for AnimatedPlayback<T>
where
    T: Animatable + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnimatedPlayback")
            .field("motion", &self.motion)
            .field("original_from", &self.original_from)
            .field("original_to", &self.original_to)
            .field("loop_mode", &self.loop_mode)
            .field("completed_plays", &self.completed_plays)
            .field("delay", &self.delay)
            .field("delay_state", &self.delay_state)
            .field("has_finish_callback", &self.finish_callback.is_some())
            .finish()
    }
}

pub(super) fn normalized_delay(seconds: f64) -> Duration {
    if seconds.is_nan() || seconds <= 0.0 {
        return Duration::ZERO;
    }
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX)
}

fn deadline_after(now: Instant, delay: Duration) -> Instant {
    now.checked_add(delay).unwrap_or_else(|| {
        // `Instant` 没有公开最大值；极端输入取当前平台仍可表达的最远时刻。
        let mut candidate = delay;
        loop {
            candidate /= 2;
            if let Some(deadline) = now.checked_add(candidate) {
                return deadline;
            }
        }
    })
}
