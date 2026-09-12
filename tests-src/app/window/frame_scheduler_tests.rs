//! `app/window/frame_scheduler.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

fn period() -> Duration {
    INITIAL_FALLBACK_PERIOD
}

#[test]
fn native_armed_deadline_backs_off_by_one_period() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    let token = scheduler.request_animation_frame(now).expect("token");
    assert_eq!(scheduler.next_deadline(), Some(now + period()));
    assert!(scheduler.mark_native_armed(token));
    // 安全网 deadline 必须晚于 cadence 目标一个周期，避免与在途
    // 原生回调竞速抢跑。
    assert_eq!(scheduler.next_deadline(), Some(now + period() * 2));
}

#[test]
fn cadence_continuation_never_tightens_native_armed_request() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    let token = scheduler.request_animation_frame(now).expect("token");
    assert!(scheduler.mark_native_armed(token));
    // 帧尾续帧按更早的 frame_time 重新武装 cadence，也不得收紧宽限。
    scheduler.request_animation_frame(now - Duration::from_millis(5));
    assert_eq!(scheduler.next_deadline(), Some(now + period() * 2));
}

#[test]
fn external_immediate_request_still_tightens() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    let token = scheduler.request_animation_frame(now).expect("token");
    assert!(scheduler.mark_native_armed(token));
    // 外部唤醒（输入等）仍可立即收紧，保持响应语义。
    assert!(scheduler.request_immediate(now - Duration::from_millis(1)).is_none());
    assert_eq!(scheduler.next_deadline(), Some(now - Duration::from_millis(1)));
}

#[test]
fn deadline_wake_preempting_native_callback_skips_cadence_sampling() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    scheduler.presented(now, false);
    let token = scheduler.request_animation_frame(now).expect("token");
    assert!(scheduler.mark_native_armed(token));
    // deadline 在宽限后到期抢跑在途回调：该帧间隔不得污染 cadence 学习。
    let wake = now + period() * 2 + Duration::from_millis(1);
    let opportunity = scheduler.take_due_opportunity(wake).expect("opportunity");
    assert!(opportunity.fallback_token().is_some());
    scheduler.presented(wake, true);
    assert_eq!(scheduler.fallback_period, INITIAL_FALLBACK_PERIOD);
}

#[test]
fn native_callback_wake_samples_cadence() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    scheduler.presented(now, false);
    let token = scheduler.request_animation_frame(now).expect("token");
    assert!(scheduler.mark_native_armed(token));
    let observed = Duration::from_millis(6);
    assert!(scheduler.notify_opportunity(token, now + observed, None));
    let opportunity = scheduler
        .take_due_opportunity(now + observed)
        .expect("opportunity");
    assert!(opportunity.fallback_token().is_none());
    scheduler.presented(now + observed, true);
    assert_eq!(scheduler.fallback_period, observed);
}

#[test]
fn pure_fallback_deadline_wake_does_not_sample() {
    let mut scheduler = FrameScheduler::new(true);
    let now = Instant::now();
    scheduler.presented(now, false);
    assert!(scheduler.request_animation_frame(now).is_some());
    // 无原生回调平台：deadline 自身决定唤醒间隔，采样它只会形成
    // 单向棘轮；学习必须保持关闭，安全网停留在默认节奏。
    let wake = now + period() + Duration::from_millis(1);
    assert!(scheduler.take_due_opportunity(wake).is_some());
    scheduler.presented(wake, true);
    assert_eq!(scheduler.fallback_period, INITIAL_FALLBACK_PERIOD);
}

// —— 自源文件移入的扩展 impl（impl FrameScheduler） ——

impl FrameScheduler {
    // 测试目标保留 surface generation 观测入口，供帧调度契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    // 测试目标保留 surface 状态观测入口，供帧调度契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn surface_state(&self) -> SurfaceState {
        self.surface_state
    }

    // 测试目标保留请求 token 观测入口，供帧调度契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn outstanding_token(&self) -> Option<FrameRequestToken> {
        self.outstanding.map(|request| request.token)
    }
}
