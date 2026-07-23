use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::scene::ScenePaint;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{canvas, ViewAdapter};

#[test]
fn canvas_uses_fixed_logical_size_clamped_by_parent_constraints() {
    let node = canvas(80.0, 40.0, |_frame, _ctx| {});
    let layout = node.widget.as_layout().expect("canvas layout capability");

    assert_eq!(
        layout.measure(Constraints::unconstrained()),
        Size::new(80.0, 40.0)
    );
    assert_eq!(
        layout.measure(Constraints::loose(Size::new(60.0, 24.0))),
        Size::new(60.0, 24.0)
    );

    let invalid = canvas(f32::NAN, -10.0, |_frame, _ctx| {});
    assert_eq!(
        invalid
            .widget
            .as_layout()
            .expect("canvas layout capability")
            .measure(Constraints::unconstrained()),
        Size::zero()
    );
}

#[test]
fn canvas_render_state_dependency_invalidates_only_its_paint_site() {
    let color = State::new(Color::red());
    let render_color = color.clone();
    let seen_frame = Arc::new(Mutex::new(None));
    let render_frame = seen_frame.clone();
    let mut tree = ViewAdapter::build_nodes(canvas(48.0, 24.0, move |frame, ctx| {
        ctx.fill_rect(frame, render_color.get(), None);
        *render_frame
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(frame);
    }));
    let root = tree.root_id().expect("canvas root");
    let frame = Rect::new(12.0, 18.0, 48.0, 24.0);
    tree.get_mut(root).expect("canvas node").set_frame(frame);

    let mut target = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut target,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        80,
    );
    ScenePaint::paint(&tree, root, frame, &mut ctx);
    assert_eq!(
        *seen_frame
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        Some(frame)
    );

    tree.reset_invalidation();
    color.set(Color::blue());
    let queue = tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(queue.node_needs_paint(root));
    assert_eq!(queue.dirty_region().bounds(), frame);
}
