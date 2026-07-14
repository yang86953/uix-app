use crate::app::frame_scheduler::{FrameScheduler, SurfaceState, SurfaceSuspendReason};
use crate::core::{Errc, Error};
use crate::draw::engine::GraphicsFailure;
use crate::native::traits::event::FrameRequestToken;
use std::time::{Duration, Instant};

#[test]
fn one_shot_requests_coalesce_and_preserve_actual_frame_time() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);

    assert!(scheduler
        .request_frame(
            start + Duration::from_millis(20),
            Some(start + Duration::from_millis(20)),
        )
        .is_some());
    assert!(scheduler
        .request_frame(
            start + Duration::from_millis(30),
            Some(start + Duration::from_millis(30)),
        )
        .is_none());
    assert!(scheduler.request_immediate(start).is_none());
    assert!(scheduler.has_outstanding_request());
    assert_eq!(scheduler.next_deadline(), Some(start));

    let actual = start + Duration::from_millis(7);
    let opportunity = scheduler.take_due_opportunity(actual).unwrap();
    assert_eq!(opportunity.frame_time(), actual);
    assert_eq!(opportunity.target_present_time(), None);
    assert!(!scheduler.has_outstanding_request());
}

#[test]
fn suspension_cancels_request_and_rejects_stale_generation() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    let stale_token = scheduler.request_immediate(start).unwrap();
    let stale_generation = scheduler.generation();

    scheduler.suspend(SurfaceSuspendReason::Minimized);
    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::Minimized)
    );
    assert_eq!(scheduler.next_deadline(), None);
    assert!(!scheduler.notify_opportunity(stale_token, start, None));

    scheduler.resume();
    assert!(scheduler.is_renderable());
    assert_ne!(scheduler.generation(), stale_generation);
    assert_eq!(scheduler.animation_delta(start, true), Duration::ZERO);
}

#[test]
fn hidden_suspend_reason_is_exact_and_resume_rebases_animation() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.animation_advanced(start);

    scheduler.suspend(SurfaceSuspendReason::Hidden);
    assert_eq!(
        scheduler.suspended_reason(),
        Some(SurfaceSuspendReason::Hidden)
    );
    assert_eq!(scheduler.next_deadline(), None);

    scheduler.resume();
    assert_eq!(scheduler.suspended_reason(), None);
    assert_eq!(
        scheduler.animation_delta(start + Duration::from_secs(3), true),
        Duration::ZERO
    );
}

#[test]
fn late_native_callback_cannot_consume_next_request_on_same_surface_generation() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);

    let first = scheduler.request_immediate(start).unwrap();
    assert!(scheduler.mark_native_armed(first));
    let fallback = scheduler.take_due_opportunity(start).unwrap();
    assert_eq!(fallback.fallback_token(), Some(first));

    let second = scheduler
        .request_animation_frame(start + Duration::from_millis(1))
        .unwrap();
    assert!(scheduler.mark_native_armed(second));
    assert_eq!(first.surface_generation, second.surface_generation);
    assert_ne!(first.request_id, second.request_id);

    let late_time = start + Duration::from_millis(3);
    assert!(!scheduler.notify_opportunity(first, late_time, None));
    assert_eq!(scheduler.outstanding_token(), Some(second));

    let native_time = start + Duration::from_millis(8);
    assert!(scheduler.notify_opportunity(second, native_time, None));
    assert!(scheduler.request_immediate(native_time).is_none());
    let native = scheduler.take_due_opportunity(native_time).unwrap();
    assert_eq!(native.frame_time(), native_time);
    assert_eq!(native.fallback_token(), None);
    assert_eq!(scheduler.outstanding_token(), None);
}

#[test]
fn request_tokens_are_scoped_to_current_surface_generation() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    let first = scheduler.request_immediate(start).unwrap();
    assert!(scheduler.mark_native_armed(first));

    scheduler.surface_changed();
    let second = scheduler.request_immediate(start).unwrap();
    assert!(scheduler.mark_native_armed(second));

    assert_ne!(first.surface_generation, second.surface_generation);
    assert_eq!(
        second,
        FrameRequestToken::new(scheduler.generation(), second.request_id)
    );
    assert!(!scheduler.notify_opportunity(first, start, None));
    assert_eq!(scheduler.outstanding_token(), Some(second));
}

#[test]
fn animation_delta_uses_opportunity_timestamp_and_rebases_after_resume() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.animation_advanced(start);

    let variable_frame_time = start + Duration::from_millis(23);
    assert_eq!(
        scheduler.animation_delta(variable_frame_time, true),
        Duration::from_millis(23)
    );

    scheduler.suspend(SurfaceSuspendReason::Minimized);
    scheduler.resume();
    assert_eq!(
        scheduler.animation_delta(start + Duration::from_secs(5), true),
        Duration::ZERO
    );
}

#[test]
fn recoverable_failure_backs_off_and_terminal_failure_has_no_deadline() {
    let start = Instant::now();
    let failure = GraphicsFailure::SurfaceLost(Error::new(
        Errc::GraphicsSurfaceLost,
        "injected surface loss",
    ));
    let mut scheduler = FrameScheduler::new(true);

    scheduler.frame_failed(&failure, start);
    let first_retry = match scheduler.surface_state() {
        SurfaceState::Recovering { retry_at, attempt } => {
            assert_eq!(attempt, 1);
            retry_at
        }
        state => panic!("expected recovering state, got {state:?}"),
    };
    assert!(first_retry > start);
    assert!(!scheduler.has_due_opportunity(start));
    assert!(scheduler
        .take_due_opportunity(first_retry - Duration::from_nanos(1))
        .is_none());
    assert!(scheduler.take_due_opportunity(first_retry).is_some());

    for attempt in 2..=8 {
        scheduler.frame_failed(&failure, first_retry);
        if attempt < 8 {
            assert!(matches!(
                scheduler.surface_state(),
                SurfaceState::Recovering {
                    attempt: current,
                    ..
                } if current == attempt
            ));
            let retry_at = scheduler.next_deadline().unwrap();
            assert!(scheduler.take_due_opportunity(retry_at).is_some());
        }
    }
    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure)
    );
    assert_eq!(scheduler.next_deadline(), None);
}

#[test]
fn out_of_memory_suspends_without_retry() {
    let mut scheduler = FrameScheduler::new(true);
    scheduler.frame_failed(
        &GraphicsFailure::OutOfMemory(Error::new(Errc::GraphicsOutOfMemory, "injected oom")),
        Instant::now(),
    );

    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure)
    );
    assert_eq!(scheduler.next_deadline(), None);
}

#[test]
fn terminal_failure_is_absorbing_across_lifecycle_signals() {
    let now = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.frame_failed(
        &GraphicsFailure::OutOfMemory(Error::new(Errc::GraphicsOutOfMemory, "injected oom")),
        now,
    );
    let terminal_generation = scheduler.generation();

    scheduler.suspend(SurfaceSuspendReason::Minimized);
    scheduler.suspend(SurfaceSuspendReason::Hidden);
    scheduler.suspend(SurfaceSuspendReason::ZeroExtent);
    scheduler.resume();
    scheduler.surface_changed();
    scheduler.presented(now, false);
    scheduler.frame_failed(
        &GraphicsFailure::Occluded(Error::new(Errc::GraphicsOccluded, "late occlusion")),
        now,
    );

    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure)
    );
    assert_eq!(scheduler.generation(), terminal_generation);
    assert_eq!(scheduler.next_deadline(), None);
    assert!(scheduler.request_immediate(now).is_none());
}

#[test]
fn occlusion_uses_non_visual_bounded_probes_and_resume_rebases_animation() {
    let start = Instant::now();
    let mut scheduler = FrameScheduler::new(true);
    scheduler.animation_advanced(start);
    scheduler.frame_failed(
        &GraphicsFailure::Occluded(Error::new(Errc::GraphicsOccluded, "injected occlusion")),
        start,
    );

    assert_eq!(
        scheduler.surface_state(),
        SurfaceState::Suspended(SurfaceSuspendReason::Occluded)
    );
    let first_probe = scheduler.next_deadline().expect("first probe deadline");
    assert_eq!(first_probe, start + Duration::from_millis(100));
    assert!(!scheduler.has_due_opportunity(first_probe));
    assert!(!scheduler.take_due_occlusion_probe(first_probe - Duration::from_nanos(1)));
    assert!(scheduler.take_due_occlusion_probe(first_probe));
    assert_eq!(scheduler.next_deadline(), None);

    assert!(scheduler.occlusion_still_present(first_probe));
    let second_probe = scheduler.next_deadline().expect("second probe deadline");
    assert_eq!(second_probe, first_probe + Duration::from_millis(200));
    assert!(scheduler.take_due_occlusion_probe(second_probe));

    scheduler.resume();
    assert!(scheduler.is_renderable());
    assert_eq!(scheduler.next_deadline(), None);
    assert_eq!(
        scheduler.animation_delta(start + Duration::from_secs(5), true),
        Duration::ZERO
    );
}

#[test]
fn one_window_occlusion_does_not_block_another_window_scheduler() {
    let now = Instant::now();
    let mut occluded = FrameScheduler::new(true);
    let mut visible = FrameScheduler::new(true);

    occluded.frame_failed(
        &GraphicsFailure::Occluded(Error::new(Errc::GraphicsOccluded, "window one occluded")),
        now,
    );
    visible.request_immediate(now);

    assert!(!occluded.has_due_opportunity(now));
    assert!(visible.has_due_opportunity(now));
    assert!(visible.take_due_opportunity(now).is_some());
    assert_eq!(
        occluded.suspended_reason(),
        Some(SurfaceSuspendReason::Occluded)
    );
}
