use crate::draw::backend::cpu::rasterizer::blur::{gaussian_blur, kernel_cache_len_for_test};
use crate::tests::common::*;

#[test]
fn gaussian_blur_rejects_non_finite_or_undersized_inputs_without_mutation() {
    let original = [1, 2, 3, 4];

    let mut pixels = original;
    gaussian_blur(
        &mut pixels,
        2,
        2,
        Rect {
            x: f32::NAN,
            y: 0.0,
            w: 2.0,
            h: 2.0,
        },
        1.0,
    );
    assert_eq!(pixels, original);

    gaussian_blur(
        &mut pixels,
        2,
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        f32::INFINITY,
    );
    assert_eq!(pixels, original);

    gaussian_blur(
        &mut pixels,
        i32::MAX,
        i32::MAX,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        1.0,
    );
    assert_eq!(pixels, original);
}

#[test]
fn gaussian_blur_distinguishes_fractional_radii() {
    let mut narrow = [0, 0, 0xFFFF_FFFF, 0, 0];
    let mut wide = narrow;
    let rect = Rect::new(0.0, 0.0, 5.0, 1.0);

    gaussian_blur(&mut narrow, 5, 1, rect, 1.1);
    gaussian_blur(&mut wide, 5, 1, rect, 1.9);

    assert_ne!(narrow, wide);
}

#[test]
fn gaussian_blur_kernel_cache_stays_bounded() {
    for index in 0..64 {
        let mut pixel = [0xFFFF_FFFF];
        gaussian_blur(
            &mut pixel,
            1,
            1,
            Rect::new(0.0, 0.0, 1.0, 1.0),
            1.0 + index as f32 / 10.0,
        );
    }

    assert!(kernel_cache_len_for_test() <= 32);
}
