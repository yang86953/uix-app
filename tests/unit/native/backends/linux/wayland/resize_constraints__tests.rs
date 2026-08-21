use super::*;

fn commit(
    state: &mut WaylandResizeConstraintState,
    transition: WaylandResizeConstraintTransition,
) -> WaylandResizeConstraintRequest {
    let request = transition.request();
    state
        .commit_after(transition, |_| Ok(()))
        .expect("纯状态机提交必须成功");
    request
}

#[test]
fn initial_state_is_resizable_and_has_no_user_constraints() {
    let state = WaylandResizeConstraintState::new(640, 480);

    assert!(state.resizable);
    assert_eq!(state.effective_extent, Some((640, 480)));
    assert_eq!(state.user_minimum, None);
    assert_eq!(state.user_maximum, None);
}

#[test]
fn locking_uses_the_current_positive_effective_extent() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("正的当前尺寸必须允许锁定");

    assert_eq!(
        commit(&mut state, transition),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((640, 480)),
            maximum: Some((640, 480)),
        }
    );
    assert!(!state.resizable);
}

#[test]
fn unlocking_restores_both_user_constraints() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let transition = state.plan_set_user_minimum(320, 240);
    commit(&mut state, transition);
    let transition = state.plan_set_user_maximum(1280, 960);
    commit(&mut state, transition);
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("锁定必须成功");
    commit(&mut state, transition);

    let transition = state
        .plan_set_resizable("test_unlock", true)
        .expect("解锁必须成功");

    assert_eq!(
        commit(&mut state, transition),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((320, 240)),
            maximum: Some((1280, 960)),
        }
    );
    assert!(state.resizable);
}

#[test]
fn unlocking_preserves_an_unset_constraint_side_for_protocol_clear() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let transition = state.plan_set_user_minimum(320, 240);
    commit(&mut state, transition);
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("锁定必须成功");
    commit(&mut state, transition);

    let transition = state
        .plan_set_resizable("test_unlock", true)
        .expect("解锁必须成功");

    assert_eq!(
        commit(&mut state, transition),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((320, 240)),
            maximum: None,
        }
    );
}

#[test]
fn locked_user_constraint_updates_do_not_replace_fixed_constraints() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("锁定必须成功");
    commit(&mut state, transition);

    let minimum = state.plan_set_user_minimum(400, 300);
    assert_eq!(
        commit(&mut state, minimum),
        WaylandResizeConstraintRequest::Unchanged
    );
    let maximum = state.plan_set_user_maximum(1600, 1200);
    assert_eq!(
        commit(&mut state, maximum),
        WaylandResizeConstraintRequest::Unchanged
    );

    let transition = state
        .plan_set_resizable("test_unlock", true)
        .expect("解锁必须成功");
    assert_eq!(
        transition.request(),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((400, 300)),
            maximum: Some((1600, 1200)),
        }
    );
}

#[test]
fn locked_programmatic_resize_and_configure_follow_each_new_effective_extent() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("锁定必须成功");
    commit(&mut state, transition);

    let programmatic_resize = state
        .plan_effective_extent(800, 600)
        .expect("程序化 resize 必须提供正尺寸");
    assert_eq!(
        commit(&mut state, programmatic_resize),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((800, 600)),
            maximum: Some((800, 600)),
        }
    );
    let compositor_configure = state
        .plan_effective_extent(1024, 768)
        .expect("configure 必须提供正尺寸");
    assert_eq!(
        commit(&mut state, compositor_configure),
        WaylandResizeConstraintRequest::Replace {
            minimum: Some((1024, 768)),
            maximum: Some((1024, 768)),
        }
    );
}

#[test]
fn adapter_failure_does_not_commit_planned_state() {
    let mut state = WaylandResizeConstraintState::new(640, 480);
    let before = state;
    let transition = state
        .plan_set_resizable("test_lock", false)
        .expect("规划锁定必须成功");

    let error = state
        .commit_after(transition, |_| {
            Err(Error::new(
                Errc::InvalidState,
                "simulated missing xdg_toplevel",
            ))
        })
        .expect_err("Adapter 失败必须拒绝状态提交");

    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(state, before);
}
