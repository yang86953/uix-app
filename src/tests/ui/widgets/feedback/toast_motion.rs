use std::time::Duration;

use crate::ui::widgets::feedback::toast_motion::{ToastMotion, ToastQueue};
use crate::ui::{AnimationConfig, Placement};

#[test]
fn motion_enters_then_idles_on_one_deadline() {
    let queue = ToastQueue::new();
    queue.push("first".to_string(), 3000);
    queue.push("second".to_string(), 5000);
    let mut motion = ToastMotion::default();

    assert!(motion.sync(
        &queue,
        AnimationConfig::fade_in(0.2),
        AnimationConfig::fade_out(0.1),
    ));
    let update = motion.update(0.2);

    assert!(!update.active);
    assert!(update.changed);
    assert_eq!(
        motion.active_timer().map(|(_, delay)| delay),
        Some(Duration::from_secs(3))
    );
}

#[test]
fn nearest_deadline_starts_only_due_items_and_rearms_the_rest() {
    let queue = ToastQueue::new();
    let first = queue.push("first".to_string(), 3000);
    queue.push("second".to_string(), 5000);
    let mut motion = ToastMotion::default();
    motion.sync(
        &queue,
        AnimationConfig::fade_in(0.0),
        AnimationConfig::fade_out(0.1),
    );
    motion.update(0.0);
    let (timer_id, _) = motion.active_timer().expect("hold timer");

    let expired = motion.fire_timer(timer_id as u32, AnimationConfig::fade_out(0.1));
    queue.remove_keys(&expired);

    assert_eq!(queue.values(), vec!["second".to_string()]);
    assert!(!queue.remove_local(first));
    assert_eq!(
        motion.active_timer().map(|(_, delay)| delay),
        Some(Duration::from_secs(2))
    );
    assert!(!motion.update(0.1).active);
    assert_eq!(motion.len(), 1);
}

#[test]
fn zero_duration_holds_without_timer_until_explicit_dismissal() {
    let queue = ToastQueue::new();
    let id = queue.push("persistent".to_string(), 0);
    let mut motion = ToastMotion::default();
    motion.sync(
        &queue,
        AnimationConfig::slide_in(Placement::Top, 0.0),
        AnimationConfig::slide_out(Placement::Top, 0.1),
    );
    motion.update(0.0);

    assert_eq!(motion.active_timer(), None);
    assert!(queue.remove_local(id));
    assert!(motion.sync(
        &queue,
        AnimationConfig::slide_in(Placement::Top, 0.0),
        AnimationConfig::slide_out(Placement::Top, 0.1),
    ));
    assert!(motion.update(0.05).active);
    assert!(!motion.update(0.05).active);
    assert!(motion.is_empty());
}

#[test]
fn external_identity_updates_content_without_restarting_hold() {
    let queue = ToastQueue::new();
    queue.replace_external([(7, "old".to_string(), 4000)]);
    let mut motion = ToastMotion::default();
    motion.sync(
        &queue,
        AnimationConfig::fade_in(0.0),
        AnimationConfig::fade_out(0.1),
    );
    motion.update(0.0);
    let timer_before = motion.active_timer();

    queue.replace_external([(7, "new".to_string(), 9000)]);
    assert!(!motion.sync(
        &queue,
        AnimationConfig::fade_in(0.0),
        AnimationConfig::fade_out(0.1),
    ));

    assert_eq!(motion.entries()[0].item(), "new");
    assert_eq!(motion.active_timer(), timer_before);
}
