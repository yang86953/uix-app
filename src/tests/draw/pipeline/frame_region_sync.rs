use std::cell::Cell;

use crate::draw::compositor::ScenePaint;
use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::painting::PaintContext;
use crate::draw::pipeline::render_frame::{FrameRenderInput, FrameRenderer};
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::{DamageRegion, RenderOutcome, SoftwareEngine};
use crate::tests::common::*;

struct ResizeSiblingScene {
    partial: Cell<bool>,
}

impl ResizeSiblingScene {
    fn new() -> Self {
        Self {
            partial: Cell::new(false),
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        if self.partial.get() {
            DirtyRegion::area(Rect::new(0.0, 0.0, 40.0, 40.0))
        } else {
            DirtyRegion::full()
        }
    }
}

impl ScenePaint for ResizeSiblingScene {
    fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
        Some(crate::draw::pipeline::NodeId::new(1))
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        self.dirty_region()
    }

    fn node_visible(&self, id: crate::draw::pipeline::NodeId) -> bool {
        (1..=3).contains(&id.slot())
    }

    fn node_frame(&self, id: crate::draw::pipeline::NodeId) -> Rect {
        match id.slot() {
            1 => Rect::new(0.0, 0.0, 100.0, 40.0),
            2 => Rect::new(0.0, 0.0, 40.0, 40.0),
            3 => Rect::new(60.0, 0.0, 40.0, 40.0),
            _ => Rect::zero(),
        }
    }

    fn node_dirty(&self, id: crate::draw::pipeline::NodeId) -> bool {
        !self.partial.get() || id.slot() == 2
    }

    fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
        0
    }

    fn node_children(&self, id: crate::draw::pipeline::NodeId) -> &[crate::draw::pipeline::NodeId] {
        static CHILDREN: [crate::draw::pipeline::NodeId; 2] = [
            crate::draw::pipeline::NodeId::new(2),
            crate::draw::pipeline::NodeId::new(3),
        ];
        if id.slot() == 1 {
            &CHILDREN
        } else {
            &[]
        }
    }

    fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
        false
    }

    fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
        None
    }

    fn parent(&self, id: crate::draw::pipeline::NodeId) -> Option<crate::draw::pipeline::NodeId> {
        (id.slot() != 1).then(|| crate::draw::pipeline::NodeId::new(1))
    }

    fn paint(&self, id: crate::draw::pipeline::NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        let color = match id.slot() {
            1 => Color::from_rgba(200, 200, 200, 255),
            2 => Color::from_rgba(255, 0, 0, 255),
            3 => Color::from_rgba(0, 0, 255, 255),
            _ => return,
        };
        ctx.fill_rect(frame, color, None);
    }
}

struct NarrowBeginEngine {
    canvas: NoopCanvas2D,
}

impl GraphicsEngine for NarrowBeginEngine {
    fn initialize(&mut self, _: i32, _: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _: i32, _: i32) -> Result<(), Error> {
        Ok(())
    }

    fn begin_frame(&mut self, _: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::FrameReady(DamageRegion::partial(vec![Rect::new(0.0, 0.0, 5.0, 5.0)]))
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Present(damage.clone())
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::cpu_pixels()
    }
}

#[test]
fn resize_promoted_full_clear_also_promotes_paint_and_present_damage() {
    let tokens = DesignTokens::antd_light();
    let fonts = FontService::new();
    let images = ImageService::new();
    let scene = ResizeSiblingScene::new();
    let mut engine = SoftwareEngine::new();
    engine
        .initialize(100, 40)
        .expect("initialize software engine");
    let mut renderer = FrameRenderer::new();

    let first_region = scene.dirty_region();
    let first = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &first_region,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert_eq!(
        first.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );

    engine
        .resize(100, 40)
        .expect("force retained surface rebuild");
    scene.partial.set(true);
    let partial_region = scene.dirty_region();
    let second = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &partial_region,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    assert_eq!(
        second.outcome,
        RenderOutcome::PresentPending(DamageRegion::full())
    );
    let blue = Color::from_rgba(0, 0, 255, 255).premultiplied();
    assert_eq!(engine.canvas_2d().pixels_mut()[20 * 100 + 80], blue);
}

#[test]
fn begin_frame_region_smaller_than_requested_is_typed_failure() {
    let tokens = DesignTokens::antd_light();
    let fonts = FontService::new();
    let images = ImageService::new();
    let scene = ResizeSiblingScene::new();
    scene.partial.set(true);
    let region = scene.dirty_region();
    let mut engine = NarrowBeginEngine {
        canvas: NoopCanvas2D,
    };
    let mut renderer = FrameRenderer::new();

    let output = renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: 1,
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let RenderOutcome::Failed(failure) = output.outcome else {
        panic!("undersized begin_frame region must fail");
    };
    assert_eq!(failure.error().code(), Errc::InvalidState);
    assert!(failure
        .error()
        .what()
        .contains("does not cover the requested paint region"));
}
