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

#[test]
fn avatar_presets_have_documented_size_and_real_circle_geometry() {
    assert_eq!(Size::Small, Size::new(32.0, 32.0));
    assert_eq!(Size::Default, Size::new(40.0, 40.0));
    assert_eq!(Size::Medium, Size::Default);
    assert_eq!(Size::Large, Size::new(56.0, 56.0));

    for size in [Size::Small, Size::Default, Size::Large] {
        let skeleton = Skeleton::new().avatar(size);
        assert_eq!(skeleton.measure(Constraints::unconstrained()), size);

        let frame = Rect::new(10.0, 20.0, 80.0, 80.0);
        let avatar = skeleton.avatar_rect_for_test(frame);
        assert_eq!(avatar.w, size.w);
        assert_eq!(avatar.h, size.h);
        assert_eq!(avatar.x, frame.x + (frame.w - size.w) * 0.5);
        assert_eq!(avatar.y, frame.y + (frame.h - size.h) * 0.5);

        let display_list = render_skeleton(&skeleton, frame, (120, 120));
        assert!(display_list.contains("FillCircle"), "{display_list}");
        assert!(
            display_list.contains(&format!("r: {}", size.w * 0.5)),
            "{display_list}"
        );
    }
}

#[test]
fn paragraph_uses_requested_rows_and_sixty_percent_last_line() {
    let frame = Rect::new(8.0, 10.0, 120.0, 48.0);
    let skeleton = Skeleton::new().paragraph(3);
    let lines = skeleton.paragraph_rects_for_test(frame);

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].w, 120.0);
    assert_eq!(lines[1].w, 120.0);
    assert_eq!(lines[2].w, 72.0);
    assert!(lines.windows(2).all(|pair| pair[0].y < pair[1].y));
    assert!(lines
        .iter()
        .all(|line| frame.contains(Point::new(line.x, line.y))));

    let display_list = render_skeleton(&skeleton, frame, (160, 80));
    assert_eq!(
        display_list.matches("FillRect").count(),
        3,
        "{display_list}"
    );

    let one_line = Skeleton::new().paragraph(0);
    let one_line_rects = one_line.paragraph_rects_for_test(Rect::new(0.0, 0.0, 100.0, 16.0));
    assert_eq!(one_line_rects.len(), 1);
    assert!((one_line_rects[0].w - 60.0).abs() < 0.001);

    let bounded = Skeleton::new().paragraph(usize::MAX);
    let bounded_rects = bounded.paragraph_rects_for_test(Rect::new(0.0, 0.0, 100.0, 1024.0));
    assert_eq!(bounded_rects.len(), 64);
    assert!(bounded_rects.iter().all(|rect| rect.x.is_finite()
        && rect.y.is_finite()
        && rect.w.is_finite()
        && rect.h.is_finite()));
}

#[test]
fn skeleton_reconcile_updates_avatar_and_paragraph_geometry() {
    let mut skeleton = Skeleton::new().avatar(Size::Small);
    skeleton.sync_from(Skeleton::new().avatar(Size::Large));
    assert_eq!(skeleton.measure(Constraints::unconstrained()), Size::Large);
    assert_eq!(
        skeleton
            .avatar_rect_for_test(Rect::new(0.0, 0.0, 80.0, 80.0))
            .w,
        56.0
    );

    skeleton.sync_from(Skeleton::new().paragraph(4));
    let lines = skeleton.paragraph_rects_for_test(Rect::new(0.0, 0.0, 100.0, 64.0));
    assert_eq!(lines.len(), 4);
    assert!((lines.last().expect("last line").w - 60.0).abs() < 0.001);
}

#[test]
fn active_shimmer_advances_dirties_and_stops_across_reconcile() {
    let frame = Rect::new(0.0, 0.0, 200.0, 16.0);
    let mut skeleton = Skeleton::new().size(200.0, 16.0).active(true);
    assert_eq!(skeleton.phase_for_test(), 0.0);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&skeleton, frame),
        Rect::zero()
    );
    let initial = render_skeleton(&skeleton, frame, (220, 40));

    assert!(WidgetAnimation::update_animation(&mut skeleton, 0.25));
    assert!((skeleton.phase_for_test() - 0.2).abs() < 0.0001);
    assert_eq!(WidgetAnimation::dirty_bounds(&skeleton, frame), frame);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&skeleton, frame),
        Rect::zero()
    );
    let advanced = render_skeleton(&skeleton, frame, (220, 40));
    assert_ne!(
        advanced, initial,
        "shimmer clip should move every advancing frame"
    );

    let midpoint = skeleton.phase_for_test();
    skeleton.sync_from(Skeleton::new().size(180.0, 16.0).active(true));
    assert_eq!(skeleton.phase_for_test(), midpoint);

    skeleton.sync_from(Skeleton::new().size(180.0, 16.0).active(false));
    assert_eq!(skeleton.phase_for_test(), 0.0);
    assert!(!WidgetAnimation::update_animation(&mut skeleton, 0.25));
    assert_eq!(WidgetAnimation::dirty_bounds(&skeleton, frame), frame);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&skeleton, frame),
        Rect::zero()
    );

    skeleton.sync_from(Skeleton::new().active(true));
    assert_eq!(WidgetAnimation::dirty_bounds(&skeleton, frame), frame);
    assert!(WidgetAnimation::update_animation(&mut skeleton, f64::NAN));
    assert_eq!(skeleton.phase_for_test(), 0.0);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&skeleton, frame),
        Rect::zero()
    );
    assert!(WidgetAnimation::update_animation(&mut skeleton, -1.0));
    assert_eq!(skeleton.phase_for_test(), 0.0);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&skeleton, frame),
        Rect::zero()
    );
    assert!(WidgetAnimation::update_animation(&mut skeleton, f64::MAX));
    assert!(skeleton.phase_for_test().is_finite());
}
