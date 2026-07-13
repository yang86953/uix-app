use crate::draw::backend::offscreen_pool::*;
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
