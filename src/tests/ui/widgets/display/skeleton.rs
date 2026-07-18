use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{Skeleton, SkeletonShape};

fn render_skeleton(skeleton: &Skeleton, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(skeleton, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn skeleton_size_normalizes_invalid_dimensions() {
    let constraints = Constraints::loose(Size::new(500.0, 500.0));

    assert_eq!(
        Skeleton::new().size(f32::NAN, -20.0).measure(constraints),
        Size::zero()
    );
    assert_eq!(
        Skeleton::new()
            .size(f32::INFINITY, 24.0)
            .measure(constraints),
        Size::new(0.0, 24.0)
    );
}

#[test]
fn constrained_text_skeleton_uses_actual_frame_and_stays_clipped() {
    let skeleton = Skeleton::new().shape(SkeletonShape::Text).size(240.0, 80.0);
    let display_list = render_skeleton(&skeleton, Rect::new(10.0, 8.0, 72.0, 12.0), (100, 40));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 72.0, h: 12.0 } }"),
        "{display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
    assert!(!display_list.contains("NaN"), "{display_list}");
}

#[test]
fn circle_and_rect_skeletons_normalize_invalid_render_frames() {
    for shape in [SkeletonShape::Rect, SkeletonShape::Circle] {
        let skeleton = Skeleton::new().shape(shape);
        for frame in [
            Rect::new(0.0, 0.0, 3.0, 2.0),
            Rect::new(0.0, 0.0, f32::NAN, -10.0),
        ] {
            let display_list = render_skeleton(&skeleton, frame, (20, 20));
            assert!(!display_list.contains("NaN"), "{display_list}");
            assert!(
                !display_list.contains("w: -")
                    && !display_list.contains("h: -")
                    && !display_list.contains("radius: -"),
                "{display_list}"
            );
        }
    }
}
