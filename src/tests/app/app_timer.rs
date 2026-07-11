use super::*;
use crate::app::test_clock::TestClock;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[test]
fn run_after_fires_once_and_unregisters() {
    let timers = AppTimerQueue::new();
    let fired = Arc::new(AtomicUsize::new(0));
    let _handle = timers.run_after(Duration::ZERO, {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    let id = timers.deadlines()[0].0;

    assert!(timers.fire(id, Instant::now()));
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(timers.len(), 0);
}

#[test]
fn run_interval_reschedules_until_handle_drops() {
    let timers = AppTimerQueue::new();
    let fired = Arc::new(AtomicUsize::new(0));
    let handle = timers.run_interval(Duration::from_millis(10), {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    let id = timers.deadlines()[0].0;

    assert!(timers.fire(id, Instant::now()));
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(timers.len(), 1);

    drop(handle);
    assert_eq!(timers.len(), 0);
}

#[test]
fn run_interval_detach_keeps_timer_after_handle_drop() {
    let timers = AppTimerQueue::new();
    let fired = Arc::new(AtomicUsize::new(0));
    let handle = timers.run_interval(Duration::from_millis(10), {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    let id = timers.deadlines()[0].0;
    handle.detach();

    assert_eq!(timers.len(), 1);
    assert!(timers.fire(id, Instant::now()));
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(timers.len(), 1);
}

#[test]
fn cancel_removes_pending_timer() {
    let timers = AppTimerQueue::new();
    let handle = timers.run_after(Duration::from_secs(1), || {});

    assert_eq!(timers.len(), 1);
    handle.cancel();
    assert_eq!(timers.len(), 0);
}

#[test]
fn run_after_deadline_uses_injected_test_clock() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let timers = AppTimerQueue::with_clock(clock.clone());

    let _handle = timers.run_after(Duration::from_millis(25), || {});

    assert_eq!(timers.deadlines()[0].1, start + Duration::from_millis(25));
}

#[test]
fn run_interval_reschedules_from_injected_fire_time() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let timers = AppTimerQueue::with_clock(clock);
    let fired = Arc::new(AtomicUsize::new(0));
    let _handle = timers.run_interval(Duration::from_millis(10), {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    let id = timers.deadlines()[0].0;
    let fire_time = start + Duration::from_millis(50);

    assert!(timers.fire(id, fire_time));

    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(
        timers.deadlines()[0].1,
        fire_time + Duration::from_millis(10)
    );
}
