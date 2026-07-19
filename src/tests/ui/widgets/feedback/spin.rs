use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::painting::PaintPass;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::{AccessibilityRole, LayoutChild, Spin};

fn render_spin(spin: &Spin, frame: Rect, pass: PaintPass) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(160, 100));
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
            160,
            100,
        );
        ctx.set_paint_pass(pass);
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(spin, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn spin_advertises_animation_capability() {
    let spin = Spin::new();

    assert!(spin.capabilities().contains(WidgetCapabilities::ANIMATION));
    assert!(spin.as_animation().is_some());
}

#[test]
fn spinning_spin_advances_phase_and_marks_paint_dirty() {
    let mut spin = Spin::new();
    let initial_phase = spin.phase();

    assert!(WidgetAnimation::update_animation(&mut spin, 0.25));
    assert_ne!(spin.phase(), initial_phase);
    let frame = Rect::new(4.0, 5.0, 24.0, 24.0);
    let dirty = WidgetAnimation::dirty_bounds(&spin, frame);
    assert!(dirty.w < frame.w);
    assert!(dirty.h < frame.h);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Spin::new()));
    let root = tree.get_mut(id).expect("spin root");
    root.set_frame(frame);
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
    assert_eq!(queue.dirty_region().rects(), &[dirty]);
}

#[test]
fn stopped_spin_does_not_keep_animation_pending() {
    let mut spin = Spin::new().spinning(false);
    let initial_phase = spin.phase();

    assert!(!WidgetAnimation::update_animation(&mut spin, 0.25));
    assert_eq!(spin.phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Spin::new().spinning(false)));
    let root = tree.get_mut(id).expect("spin root");
    root.set_frame(Rect::new(4.0, 5.0, 24.0, 24.0));
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(!tree.update(1.0 / 60.0));
    assert!(tree.invalidation().lock().unwrap().is_empty());
}

#[test]
fn constrained_spin_clips_and_scales_dots_inside_actual_frame() {
    let frame = Rect::new(6.0, 7.0, 10.0, 8.0);
    let display = render_spin(&Spin::new().large(), frame, PaintPass::Content);
    assert!(display.contains("PushClip { rect: Rect { x: 6.0, y: 7.0, w: 10.0, h: 8.0 } }"));
    assert!(
        !display.contains("NaN") && !display.contains("inf"),
        "{display}"
    );
    let dirty = WidgetAnimation::dirty_bounds(&Spin::new().large(), frame);
    assert!(dirty.x >= frame.x && dirty.y >= frame.y);
    assert!(dirty.x + dirty.w <= frame.x + frame.w);
    assert!(dirty.y + dirty.h <= frame.y + frame.h);

    let invalid = render_spin(
        &Spin::new(),
        Rect::new(f32::INFINITY, f32::NEG_INFINITY, -4.0, f32::NAN),
        PaintPass::Content,
    );
    assert_eq!(invalid, "DisplayList { ops: [] }");
}

#[test]
fn wrapper_spin_paints_after_children_elides_tip_and_lays_out_child() {
    let spin = Spin::new()
        .large()
        .tip("正在加载超长中英文 mixed loading status")
        .wrapper_mode();
    let frame = Rect::new(4.0, 5.0, 96.0, 64.0);

    assert_eq!(
        render_spin(&spin, frame, PaintPass::Content),
        "DisplayList { ops: [] }"
    );
    let display = render_spin(&spin, frame, PaintPass::AfterChildren);
    assert!(display.contains("PushClip { rect: Rect { x: 4.0, y: 5.0, w: 96.0, h: 64.0 } }"));
    assert!(display.contains('…'), "{display}");
    assert!(!display.contains("mixed loading status"), "{display}");

    let child = ComponentId::new(9);
    assert_eq!(
        spin.layout_children(
            frame,
            &[LayoutChild::new(child, Size::new(12.0, 8.0))],
            &WidgetTree::new(),
        ),
        vec![(child, frame)]
    );
    assert_eq!(WidgetRender::children_clip(&spin, frame), Some(frame));

    let stopped = Spin::new()
        .tip("must remain hidden")
        .spinning(false)
        .wrapper_mode();
    assert_eq!(
        render_spin(&stopped, frame, PaintPass::AfterChildren),
        "DisplayList { ops: [] }"
    );
}

#[test]
fn spin_accessibility_has_a_stable_loading_name_and_preserves_tip() {
    let loading = Spin::new().snapshot_fields().accessibility();
    assert_eq!(loading.role, AccessibilityRole::Status);
    assert_eq!(loading.name.as_deref(), Some("加载中"));

    let tipped = Spin::new()
        .tip("正在同步")
        .wrapper_mode()
        .snapshot_fields()
        .accessibility();
    assert_eq!(tipped.role, AccessibilityRole::Status);
    assert_eq!(tipped.name.as_deref(), Some("正在同步"));
}
