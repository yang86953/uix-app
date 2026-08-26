use super::*;

#[test]
fn transition_converts_to_animation_config() {
    let config: AnimationConfig = Transition::fade_in(0.3).into();
    assert!(config.is_enter());
    assert_eq!(config.duration(), 0.3);
    assert_eq!(config.delay(), 0.0);
}

#[test]
fn transition_stagger_sets_delay() {
    let config: AnimationConfig = Transition::stagger(0.25, Transition::slide_up(0.2)).into();
    assert_eq!(config.delay(), 0.25);
    assert!(config.is_enter());
    assert_eq!(config.duration(), 0.2);
}

#[test]
fn transition_player_waits_for_delay_before_animating() {
    let config = Transition::stagger(0.5, Transition::fade_in(0.1)).into();
    let mut player = TransitionPlayer::new(config);
    player.update(0.2);
    assert!(!player.finished);
    assert_eq!(player.opacity_progress, 0.0, "delay 期间不推进动画");
    player.update(0.4);
    assert!(player.finished, "延迟耗尽后动画推进完成");
}
