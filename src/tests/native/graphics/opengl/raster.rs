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
