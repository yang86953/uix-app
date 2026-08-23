use super::*;
use crate::app::queues::active_work_registry::ActiveWorkKind;

#[test]
fn empty_registry_without_deadline_enters_deep_idle() {
    let active_work = ActiveWorkRegistry::new();

    assert_eq!(
        wait_loop_state(&active_work, None),
        WindowLoopState::DeepIdle
    );
}

#[test]
fn registered_work_or_deadline_uses_registered_wait_state() {
    let now = Instant::now();
    let mut active_work = ActiveWorkRegistry::new();
    active_work.register_open(ActiveWorkKind::GraphicsMaintenance);

    assert_eq!(
        wait_loop_state(&active_work, None),
        WindowLoopState::RegisteredActive
    );

    active_work.unregister(ActiveWorkKind::GraphicsMaintenance);
    assert_eq!(
        wait_loop_state(&active_work, Some(now)),
        WindowLoopState::RegisteredActive
    );
}
