use crate::draw::rasterizer::core::{
    apply_opacity, color_with_glyph_opacity, color_with_premultiplied_opacity, fill_rect_raw,
    put_pixel, put_pixel_aa,
};
use crate::draw::Color;

#[test]
fn straight_surrogate_round_trips_every_premultiplied_channel_after_opacity() {
    for opacity in [
        f32::NAN,
        0.0,
        1.0 / 255.0,
        0.37,
        0.5,
        0.999_998,
        0.999_999,
        1.0,
    ] {
        for alpha in 0..=u8::MAX {
            for channel in 0..=u8::MAX {
                let color =
                    Color::from_rgba(channel, u8::MAX - channel, channel.wrapping_mul(73), alpha);
                let encoded = color_with_premultiplied_opacity(color, opacity);
                assert_eq!(
                    encoded.premultiplied(),
                    apply_opacity(color.premultiplied(), opacity),
                    "opacity={opacity:?}, alpha={alpha}, channel={channel}"
                );
            }
        }
    }
}

#[test]
fn glyph_opacity_preserves_rgb_and_quantizes_alpha_before_coverage() {
    for opacity in [
        f32::NAN,
        0.0,
        1.0 / 255.0,
        0.37,
        0.5,
        0.999_998,
        0.999_999,
        1.0,
    ] {
        for alpha in 0..=u8::MAX {
            let color = Color::from_rgba(17, 83, 201, alpha);
            let encoded = color_with_glyph_opacity(color, opacity);
            assert_eq!((encoded.r, encoded.g, encoded.b), (17, 83, 201));
            assert_eq!(
                encoded.a,
                (alpha as f32 * opacity) as u8,
                "opacity={opacity:?}, alpha={alpha}"
            );
        }
    }
}

#[test]
fn pixel_writes_reject_negative_and_overflowing_indices() {
    let mut pixels = [0xDEAD_BEEF; 4];
    let original = pixels;

    put_pixel(
        &mut pixels,
        i32::MAX,
        i32::MAX - 1,
        i32::MAX - 1,
        0,
        0,
        i32::MAX,
        i32::MAX,
        0xFFFF_FFFF,
    );
    put_pixel_aa(
        &mut pixels,
        2,
        -1,
        -1,
        i32::MIN,
        i32::MIN,
        i32::MAX,
        i32::MAX,
        0xFFFF_FFFF,
        0.5,
    );

    assert_eq!(pixels, original);
}

#[test]
fn fill_rect_clips_before_iteration_and_handles_extreme_extents() {
    let mut pixels = [0u32; 6];
    fill_rect_raw(
        &mut pixels,
        3,
        -1,
        0,
        i32::MAX,
        i32::MAX,
        0,
        0,
        3,
        2,
        0xFFFF_FFFF,
    );

    assert_eq!(pixels, [0xFFFF_FFFF; 6]);
}
