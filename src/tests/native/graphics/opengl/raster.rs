use crate::native::graphics::opengl::raster::*;
use crate::tests::common::*;

#[test]
fn soft_tile_validation_uses_compact_payload_at_destination() {
    let pixels = vec![0u32; 4 * 3];
    assert!(validate_tile(&pixels, 8, 6, SoftFallbackTile::at_destination(2, 1, 4, 3)).is_ok());
    assert!(validate_tile(&pixels, 8, 6, SoftFallbackTile::at_destination(6, 1, 4, 3)).is_err());
    assert!(validate_tile(
        &pixels[..10],
        8,
        6,
        SoftFallbackTile::at_destination(0, 0, 4, 3)
    )
    .is_err());
}

#[test]
fn target_state_keeps_swapchain_dpr_and_offscreen_at_one() {
    let swapchain = TargetState::swapchain(800, 600, 1600, 1200);
    assert_eq!(swapchain.dpr, 2.0);
    assert_eq!(swapchain.logical_width, 800);
    assert_eq!(swapchain.logical_height, 600);
}

#[test]
fn scissor_maps_logical_top_left_coordinates_to_each_target_drawable() {
    assert_eq!(
        logical_scissor_to_drawable(
            TargetState::swapchain(800, 600, 1600, 1200),
            (10, 20, 30, 40),
        ),
        (20, 1080, 60, 80)
    );
    assert_eq!(
        logical_scissor_to_drawable(
            TargetState {
                framebuffer: None,
                logical_width: 80,
                logical_height: 60,
                drawable_width: 80,
                drawable_height: 60,
                dpr: 1.0,
            },
            (10, 20, 30, 40),
        ),
        (10, 0, 30, 40)
    );
}

#[test]
fn scissor_converts_both_endpoints_at_fractional_dpr() {
    for (drawable_width, drawable_height, expected) in [
        (8, 6, (2, 3, 4, 2)),
        (10, 8, (2, 4, 6, 3)),
        (12, 9, (3, 4, 6, 4)),
        (16, 12, (4, 6, 8, 4)),
    ] {
        assert_eq!(
            logical_scissor_to_drawable(
                TargetState::swapchain(8, 6, drawable_width, drawable_height),
                (2, 1, 4, 2),
            ),
            expected
        );
    }

    let dpr_1_5 = TargetState::swapchain(8, 6, 12, 9);
    assert_eq!(
        logical_scissor_to_drawable(dpr_1_5, (1, 1, 2, 2)),
        (1, 4, 4, 4),
        "endpoint subtraction must retain the fourth pixel old ceil(extent) lost"
    );
    assert_eq!(
        logical_scissor_to_drawable(dpr_1_5, (7, 5, 1, 1)),
        (10, 0, 2, 2)
    );
    assert_eq!(
        logical_scissor_to_drawable(dpr_1_5, (8, 6, 4, 4)),
        (12, 0, 0, 0)
    );
    assert_eq!(
        logical_scissor_to_drawable(dpr_1_5, (2, 2, -1, -1)),
        (3, 6, 0, 0)
    );
}
