use crate::app::window_driver::{ensure_surface_matches_window, sync_root_frame_exactly_to_engine};
use crate::draw::backend::DamageRegion;
use crate::draw::{Canvas2D, RenderTarget, UpdateStrategy};
use crate::tests::common::*;
use crate::ui::Container;

struct ScaledCanvasEngine {
    inner: Renderer,
    device_pixel_ratio: f32,
    logical_width: i32,
    logical_height: i32,
    resize_calls: usize,
}

impl ScaledCanvasEngine {
    fn new(logical_width: i32, logical_height: i32, device_pixel_ratio: f32) -> Self {
        let mut inner = Renderer::cpu();
        inner
            .initialize(
                (logical_width as f32 * device_pixel_ratio).round() as i32,
                (logical_height as f32 * device_pixel_ratio).round() as i32,
            )
            .expect("initialize scaled software canvas");
        Self {
            inner,
            device_pixel_ratio,
            logical_width,
            logical_height,
            resize_calls: 0,
        }
    }
}

impl RenderTarget for ScaledCanvasEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.resize_calls += 1;
        self.inner.resize(
            (width as f32 * self.device_pixel_ratio).round() as i32,
            (height as f32 * self.device_pixel_ratio).round() as i32,
        )?;
        self.logical_width = width;
        self.logical_height = height;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        (self.logical_width, self.logical_height)
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
    }
}

#[test]
fn scaled_engine_reports_logical_extent_independently_of_canvas() {
    let mut engine = ScaledCanvasEngine::new(100, 60, 1.5);
    assert_eq!(engine.logical_extent(), (100, 60));
    assert_eq!(
        (engine.canvas_2d().width(), engine.canvas_2d().height()),
        (150, 90)
    );
}

#[test]
fn exact_root_sync_uses_logical_extent_instead_of_physical_canvas() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(Container::new()));
    let mut engine = ScaledCanvasEngine::new(320, 200, 1.5);

    sync_root_frame_exactly_to_engine(&mut tree, &mut engine);

    assert_eq!(
        tree.get(root_id).expect("root").frame(),
        Rect::new(0.0, 0.0, 320.0, 200.0)
    );
}

#[test]
fn surface_reconciliation_does_not_resize_an_already_matching_hidpi_canvas() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(Container::new()));
    tree.get_mut(root_id)
        .expect("root")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 200.0));
    let mut engine = ScaledCanvasEngine::new(320, 200, 1.5);

    assert!(!ensure_surface_matches_window(
        &mut tree,
        &mut engine,
        320,
        200
    ));
    assert_eq!(engine.resize_calls, 0);

    assert!(ensure_surface_matches_window(
        &mut tree,
        &mut engine,
        400,
        240
    ));
    assert_eq!(engine.resize_calls, 1);
    assert_eq!(
        tree.get(root_id).expect("root").frame(),
        Rect::new(0.0, 0.0, 400.0, 240.0)
    );
}

#[test]
fn asymmetric_dpi_rounding_does_not_trigger_repeated_resize() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(Container::new()));
    tree.get_mut(root_id)
        .expect("root")
        .set_frame(Rect::new(0.0, 0.0, 333.0, 1000.0));
    let mut engine = ScaledCanvasEngine::new(333, 1000, 1.25);
    engine.device_pixel_ratio = 416.0 / 333.0;

    assert!(!ensure_surface_matches_window(
        &mut tree,
        &mut engine,
        333,
        1000
    ));
    assert_eq!(engine.resize_calls, 0);
}

struct AdoptedClientRectEngine {
    logical_width: i32,
    logical_height: i32,
    resize_calls: usize,
}

impl AdoptedClientRectEngine {
    fn new(logical_width: i32, logical_height: i32) -> Self {
        Self {
            logical_width,
            logical_height,
            resize_calls: 0,
        }
    }
}

impl RenderTarget for AdoptedClientRectEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.logical_width = width.max(1);
        self.logical_height = height.max(1);
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.resize_calls += 1;
        self.logical_width = width.max(1) + 120;
        self.logical_height = height.max(1) + 80;
        Ok(())
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::Idle
    }

    fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Idle
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        panic!("AdoptedClientRectEngine is not used for drawing in this test")
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        (self.logical_width, self.logical_height)
    }
}

#[test]
fn surface_reconciliation_skips_resize_when_engine_already_covers_client_rect() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(Container::new()));
    tree.get_mut(root_id)
        .expect("root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    let mut engine = AdoptedClientRectEngine::new(920, 680);

    assert!(!ensure_surface_matches_window(
        &mut tree,
        &mut engine,
        800,
        600
    ));
    assert_eq!(engine.resize_calls, 0);
    assert_eq!(
        tree.get(root_id).expect("root").frame(),
        Rect::new(0.0, 0.0, 920.0, 680.0)
    );
}
