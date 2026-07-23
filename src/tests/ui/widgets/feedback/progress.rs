use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::command::PaintPass;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::ProgressMode;
use crate::ui::{AccessibilityRole, ProgressBar};

fn render_progress(progress: &ProgressBar, frame: Rect, pass: PaintPass) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(160, 80));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::command::DisplayList::new();
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
            160,
            80,
        );
        ctx.set_paint_pass(pass);
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(progress, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn progress_bar_advertises_animation_capability() {
    let progress = ProgressBar::new().indeterminate();

    assert!(progress
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(progress.as_animation().is_some());
}

#[test]
fn indeterminate_progress_advances_phase_and_marks_paint_dirty() {
    let mut progress = ProgressBar::new().indeterminate();
    let initial_phase = progress.animation_phase();

    assert!(WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_ne!(progress.animation_phase(), initial_phase);
    let frame = Rect::new(4.0, 5.0, 120.0, 8.0);
    let dirty = WidgetAnimation::dirty_bounds(&progress, frame);
    assert_eq!(dirty.y, frame.y);
    assert_eq!(dirty.h, frame.h);
    assert!(dirty.w < frame.w);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().indeterminate()));
    let root = tree.get_mut(id).expect("progress root");
    root.set_frame(frame);
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
    let region = queue.dirty_region();
    assert_eq!(region.rects().len(), 1);
    assert!(region.rects()[0].w < frame.w);
}

#[test]
fn determinate_progress_does_not_keep_animation_pending() {
    let mut progress = ProgressBar::new().progress(0.4);
    let initial_phase = progress.animation_phase();

    assert!(!WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_eq!(progress.animation_phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().progress(0.4)));
    let root = tree.get_mut(id).expect("progress root");
    root.set_frame(Rect::new(4.0, 5.0, 120.0, 8.0));
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(!tree.update(1.0 / 60.0));
    assert!(tree.invalidation().lock().unwrap().is_empty());
}

#[test]
fn progress_normalizes_non_finite_values_and_dimensions() {
    let progress = ProgressBar::new()
        .progress(f32::NAN)
        .size(f32::INFINITY, -10.0)
        .round(false);

    assert!(matches!(
        progress.snapshot_fields(),
        SnapshotFields::ProgressBar {
            progress: 0.0,
            mode: ProgressMode::Determinate(0.0),
            width: 0.0,
            height: 0.0,
            round: false,
            ..
        }
    ));
    assert_eq!(
        progress.measure(Constraints::loose(Size::new(200.0, 40.0))),
        Size::zero()
    );
}

#[test]
fn indeterminate_accessibility_does_not_invent_a_numeric_value() {
    let indeterminate = ProgressBar::new().indeterminate();
    let accessibility = indeterminate.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::ProgressBar);
    assert_eq!(accessibility.state.value_now, None);
    assert_eq!(accessibility.state.value_min, None);
    assert_eq!(accessibility.state.value_max, None);

    let determinate = ProgressBar::new().progress(0.65);
    let accessibility = determinate.snapshot_fields().accessibility();
    assert!(accessibility
        .state
        .value_now
        .is_some_and(|value| (value - 0.65).abs() < 1.0e-6));
    assert_eq!(accessibility.state.value_min, Some(0.0));
    assert_eq!(accessibility.state.value_max, Some(1.0));
}

#[test]
fn determinate_line_uses_fractional_width_square_corners_and_one_paint_pass() {
    let progress = ProgressBar::new().progress(0.45).round(false);
    let display = render_progress(
        &progress,
        Rect::new(2.0, 3.0, 100.0, 10.0),
        PaintPass::Content,
    );

    assert!(display.contains("PushClip { rect: Rect { x: 2.0, y: 3.0, w: 100.0, h: 10.0 } }"));
    assert!(display.contains("w: 45.0, h: 10.0"), "{display}");
    assert!(display.matches("radius: None").count() >= 2, "{display}");
    assert_eq!(
        render_progress(
            &progress,
            Rect::new(2.0, 3.0, 100.0, 10.0),
            PaintPass::AfterChildren,
        ),
        "DisplayList { ops: [] }"
    );
}

#[test]
fn progress_normalizes_and_clips_actual_render_geometry() {
    let invalid = render_progress(
        &ProgressBar::new().indeterminate().circle(),
        Rect::new(f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -12.0),
        PaintPass::Content,
    );
    assert_eq!(invalid, "DisplayList { ops: [] }");

    let frame = Rect::new(8.0, 9.0, 12.0, 4.0);
    let display = render_progress(
        &ProgressBar::new().indeterminate(),
        frame,
        PaintPass::Content,
    );
    assert!(display.contains("PushClip { rect: Rect { x: 8.0, y: 9.0, w: 12.0, h: 4.0 } }"));
    let dirty = WidgetAnimation::dirty_bounds(&ProgressBar::new().indeterminate(), frame);
    assert!(dirty.x >= frame.x && dirty.y >= frame.y);
    assert!(dirty.x + dirty.w <= frame.x + frame.w);
    assert!(dirty.y + dirty.h <= frame.y + frame.h);
}

#[test]
fn determinate_gradient_uses_real_endpoint_colors_instead_of_a_midpoint_fill() {
    let progress = ProgressBar::new()
        .progress(0.75)
        .round(false)
        .gradient(Color::red(), Color::blue());
    let display = render_progress(
        &progress,
        Rect::new(0.0, 0.0, 100.0, 10.0),
        PaintPass::Content,
    );

    assert!(display.contains("FillLinearGradient"), "{display}");
    assert!(
        display.contains("color_a: Color { r: 255, g: 0, b: 0"),
        "{display}"
    );
    assert!(
        display.contains("color_b: Color { r: 64, g: 0, b: 191"),
        "the visible gradient endpoint must represent 75% of the full track: {display}"
    );
}

#[test]
fn dashboard_uses_a_semicircular_arc_and_renders_zero_percent_format_text() {
    let dashboard = ProgressBar::new()
        .dashboard()
        .progress(0.0)
        .format(|progress| format!("{:.0}%", progress * 100.0));
    let display = render_progress(
        &dashboard,
        Rect::new(0.0, 0.0, 120.0, 80.0),
        PaintPass::Content,
    );

    assert!(display.contains("StrokePath"), "{display}");
    assert!(
        !display.contains("FillCircle"),
        "dashboard must not reuse the full-circle donut track: {display}"
    );
    assert!(display.contains("text: \"0%\""), "{display}");
}
