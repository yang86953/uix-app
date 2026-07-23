use crate::draw::backend::cpu::offscreen::*;
use crate::tests::common::*;

#[test]
fn pool_create_draw_copy_and_reuse() {
    let mut pool = CpuOffscreenPool::new();
    let a = pool.create(4, 4).expect("a");
    {
        let canvas = pool.canvas_mut(&a).expect("canvas");
        canvas.fill_rect(
            crate::core::Rect::new(0.0, 0.0, 4.0, 4.0),
            Color::from_rgb(255, 0, 0),
            None,
        );
    }
    let (pixels, w) = pool.copy_pixels(&a).expect("pixels");
    assert_eq!(w, 4);
    assert_eq!(pixels.len(), 16);
    assert_ne!(pixels[0] >> 24, 0);

    pool.destroy(a);
    let b = pool.create(2, 2).expect("b");
    assert_eq!(b.0, a.0);
    assert_eq!(pool.slot_len(), 1);
}

#[test]
fn rejected_allocation_does_not_consume_a_slot_or_corrupt_memory_accounting() {
    let mut pool = CpuOffscreenPool::new();

    assert!(pool.create(i32::MAX, i32::MAX).is_none());
    assert_eq!(pool.slot_len(), 0);
    assert_eq!(pool.memory_usage(), 0);

    let handle = pool.create(2, 3).expect("small surface");
    assert_eq!(handle.0, 0);
    assert_eq!(pool.memory_usage(), 2 * 3 * 4);
}
