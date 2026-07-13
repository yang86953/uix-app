use super::*;

#[test]
fn fake_graphics_context_records_swap_damage() {
    let mut context = FakeGraphicsContext::new();
    let damage = PresentDamage::Partial(vec![(1, 2, 3, 4)]);

    context
        .swap_buffers(damage.clone())
        .expect("fake swap_buffers must succeed");

    assert_eq!(context.state.swap_buffers_calls, 1);
    assert_eq!(context.state.last_swap_damage, Some(damage));
}
