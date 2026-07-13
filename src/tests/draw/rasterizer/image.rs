use super::*;

#[test]
fn empty_destination_is_a_noop() {
    let source = [0xFF_11_22_33; 4];
    let mut pixels = vec![0xDE_AD_BE_EF; 4];
    let clip = Rect::new(0.0, 0.0, 2.0, 2.0);
    let source_rect = Rect::new(0.0, 0.0, 2.0, 2.0);

    blit_image(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        &source,
        2,
        source_rect,
        Rect::new(0.0, 0.0, 0.0, 2.0),
    );
    blit_image(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        &source,
        2,
        source_rect,
        Rect::new(0.0, 0.0, 2.0, 0.0),
    );

    assert_eq!(pixels, vec![0xDE_AD_BE_EF; 4]);
}
