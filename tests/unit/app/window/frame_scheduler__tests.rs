use super::*;

#[test]
fn one_shot_request_coalesces_without_creating_a_periodic_tick() {
    let now = Instant::now();
    let later = now + Duration::from_millis(32);
    let earlier = now + Duration::from_millis(8);
    let mut scheduler = FrameScheduler::new(true);

    let token = scheduler
        .request_frame(later, Some(later))
        .expect("首次请求必须分配 one-shot token");
    assert!(
        scheduler
            .request_frame(now + Duration::from_millis(48), None)
            .is_none()
    );
    assert_eq!(scheduler.outstanding_token(), Some(token));
    assert_eq!(scheduler.next_deadline(), Some(later));

    assert!(scheduler.request_frame(earlier, Some(earlier)).is_none());
    assert_eq!(scheduler.outstanding_token(), Some(token));
    assert_eq!(scheduler.next_deadline(), Some(earlier));
    assert!(!scheduler.has_due_opportunity(now));

    let opportunity = scheduler
        .take_due_opportunity(earlier)
        .expect("到期请求必须只产生一次帧机会");
    assert_eq!(opportunity.frame_time(), earlier);
    assert_eq!(opportunity.target_present_time(), Some(earlier));
    assert_eq!(opportunity.fallback_token(), None);
    assert!(!scheduler.has_outstanding_request());
    assert_eq!(scheduler.next_deadline(), None);
    assert!(scheduler.take_due_opportunity(earlier).is_none());
}

#[test]
fn suspend_cancels_deadline_rejects_stale_callback_and_rebases_animation() {
    let now = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.animation_advanced(now);
    assert_eq!(
        scheduler.animation_delta(now + Duration::from_millis(16), true),
        Duration::from_millis(16)
    );
    let token = scheduler
        .request_animation_frame(now)
        .expect("动画请求必须登记 one-shot token");
    assert!(scheduler.mark_native_armed(token));
    let active_generation = scheduler.generation();

    scheduler.suspend(SurfaceSuspendReason::Hidden);

    assert!(scheduler.generation() > active_generation);
    assert_eq!(scheduler.next_deadline(), None);
    assert!(!scheduler.has_outstanding_request());
    assert!(!scheduler.notify_opportunity(token, now, None));

    scheduler.resume();
    let resumed = now + Duration::from_secs(1);
    scheduler
        .request_immediate(resumed)
        .expect("恢复后必须允许显式新帧请求");
    let opportunity = scheduler
        .take_due_opportunity(resumed)
        .expect("恢复请求必须产生帧机会");
    assert_eq!(
        scheduler.animation_delta(opportunity.frame_time(), true),
        Duration::ZERO
    );
    scheduler.animation_advanced(opportunity.frame_time());
    assert_eq!(
        scheduler.animation_delta(resumed + Duration::from_millis(16), true),
        Duration::from_millis(16)
    );
}

#[test]
fn terminal_failure_is_an_absorbing_state_without_deadline() {
    let now = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.request_immediate(now);

    scheduler.mark_terminal_failure();
    let terminal_generation = scheduler.generation();

    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure)
    );
    assert_eq!(scheduler.next_deadline(), None);
    assert!(!scheduler.has_outstanding_request());
    assert!(scheduler.request_immediate(now).is_none());

    scheduler.resume();
    scheduler.surface_changed();
    assert_eq!(scheduler.generation(), terminal_generation);
    assert_eq!(scheduler.next_deadline(), None);
    assert!(!scheduler.is_renderable());
}
