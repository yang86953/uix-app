use crate::draw::api::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken, ThemeTokens,
};
use crate::draw::command::{DisplayList, PaintOp};
use crate::draw::renderer::RenderTarget;
use crate::draw::scene::picture::*;
use crate::draw::scene::ScenePaint;
use crate::draw::Canvas2D;
use crate::draw::Orientation;
use crate::draw::UpdateStrategy;
use crate::tests::common::*;

struct TestTokens;

macro_rules! transparent_color_tokens {
    ($($name:ident),+ $(,)?) => {
        $(
            fn $name(&self) -> crate::draw::Color {
                crate::draw::Color::transparent()
            }
        )+
    };
}

impl IColorTokens for TestTokens {
    transparent_color_tokens!(
        color_primary,
        color_primary_hover,
        color_primary_active,
        color_primary_bg,
        color_primary_border,
        color_bg_container,
        color_bg_elevated,
        color_bg_raised,
        color_bg_overlay,
        color_bg_layout,
        color_bg_spotlight,
        color_bg_mask,
        color_border,
        color_border_secondary,
        color_fill,
        color_fill_secondary,
        color_fill_tertiary,
        color_fill_quaternary,
        color_text,
        color_text_secondary,
        color_text_tertiary,
        color_text_quaternary,
        color_white,
        color_black,
        color_shadow,
        color_shadow_secondary,
        color_success,
        color_success_bg,
        color_success_border,
        color_warning,
        color_warning_bg,
        color_warning_border,
        color_error,
        color_error_bg,
        color_error_border,
        color_info,
        color_info_bg,
        color_info_border,
        color_link,
        color_link_hover,
        color_link_active,
    );
}

impl ITypographyTokens for TestTokens {
    fn font_family(&self) -> &str {
        "test"
    }
}

impl ISpacingTokens for TestTokens {}

impl IBoxShadowTokens for TestTokens {
    fn box_shadow(&self) -> ShadowToken {
        ShadowToken::none()
    }

    fn box_shadow_secondary(&self) -> ShadowToken {
        ShadowToken::none()
    }
}

impl ThemeTokens for TestTokens {}

struct CachedPictureScene;

impl ScenePaint for CachedPictureScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(NodeId::new(1))
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, _id: NodeId) -> Rect {
        Rect::new(10.0, 10.0, 16.0, 16.0)
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        false
    }

    fn node_z_index(&self, _id: NodeId) -> i32 {
        0
    }

    fn node_children(&self, _id: NodeId) -> &[NodeId] {
        &[]
    }

    fn children_clip(&self, _id: NodeId, _frame: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _id: NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<NodeId> {
        None
    }

    fn node_focusable(&self, _id: NodeId) -> bool {
        false
    }

    fn hit_test(&self, _pos: Point) -> Option<NodeId> {
        None
    }

    fn parent(&self, _id: NodeId) -> Option<NodeId> {
        None
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

struct CountingEngine {
    inner: Renderer,
    encoded_picture_executions: usize,
    destroy_error: Option<Error>,
    fail_create: bool,
}

impl CountingEngine {
    fn new() -> Self {
        Self {
            inner: Renderer::cpu(),
            encoded_picture_executions: 0,
            destroy_error: None,
            fail_create: false,
        }
    }
}

impl RenderTarget for CountingEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()?;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
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

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if self.fail_create {
            return None;
        }
        self.inner.create_offscreen(width, height)
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        if let Some(error) = &self.destroy_error {
            return Err(error.clone());
        }
        self.inner.destroy_offscreen(handle);
        Ok(())
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.inner.offscreen_canvas(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<crate::draw::command::EncodedPictureExecution, Error> {
        self.encoded_picture_executions += 1;
        self.inner.try_execute_encoded_picture(handle, encoder)
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.inner.try_begin_offscreen_paint(handle)
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.inner.try_flush_offscreen_paint(handle)
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.inner.try_end_offscreen_paint()
    }
}

#[test]
fn cached_picture_frame_encoder_preserves_non_rect_display_list_pixels() {
    let mut list = DisplayList::new();
    list.push(PaintOp::FillCircle {
        cx: 18.0,
        cy: 18.0,
        r: 4.0,
        color: crate::draw::Color::red(),
    });

    let fonts = FontService::new();
    let images = ImageService::new();
    let encoder = encode_cached_picture(
        &list,
        16,
        16,
        Point::new(10.0, 10.0),
        FontHandle::default(),
        &fonts,
        &images,
    )
    .expect("non-rect cached Picture must have an ordered FrameEncoder");
    let frame = encoder.render_reference();

    assert_eq!(
        frame.pixel(8, 8),
        Some(crate::draw::Color::red().premultiplied())
    );
    assert_eq!(
        frame.pixel(1, 1),
        Some(crate::draw::Color::transparent().premultiplied())
    );
}

#[test]
fn picture_resize_preserves_its_handle_when_checked_destroy_fails() {
    let mut engine = CountingEngine::new();
    engine.initialize(16, 16).expect("CPU renderer");
    let original = engine.create_offscreen(4, 4).expect("initial Picture");
    let mut handle = Some(original);
    engine.fail_create = true;
    assert!(!ensure_offscreen(&mut engine, &mut handle, 8, 8).expect("failed replacement create"));
    assert_eq!(handle, Some(original));
    assert!(engine.inner.offscreen_canvas(&original).is_some());
    engine.fail_create = false;
    engine.destroy_error = Some(Error::new(
        Errc::GraphicsDeviceLost,
        "injected Picture destroy failure",
    ));

    let error = ensure_offscreen(&mut engine, &mut handle, 8, 8)
        .expect_err("Picture resize must surface a checked destroy failure");
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert_eq!(handle, Some(original));
    assert!(
        engine.inner.offscreen_canvas(&original).is_some(),
        "failed destruction must retain the original Picture target"
    );

    engine.destroy_error = None;
    assert!(ensure_offscreen(&mut engine, &mut handle, 8, 8).expect("retry resize"));
    let replacement = handle.expect("replacement Picture");
    let canvas = engine
        .inner
        .offscreen_canvas(&replacement)
        .expect("replacement Picture target");
    assert_eq!((canvas.width(), canvas.height()), (8, 8));
}

#[test]
fn cached_complete_picture_is_executed_by_the_production_compositor() {
    let mut engine = CountingEngine::new();
    engine.initialize(16, 16).expect("CPU renderer");

    let mut list = DisplayList::new();
    list.push(PaintOp::FillCircle {
        cx: 18.0,
        cy: 18.0,
        r: 4.0,
        color: crate::draw::Color::red(),
    });
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = TestTokens;
    let env = LayerRenderEnv {
        font: FontHandle::default(),
        font_service: &fonts,
        image_service: &images,
        tokens: &tokens,
        dpi: 96.0,
        dpr: 1.0,
        orientation: Orientation::YDown,
    };
    let scene = CachedPictureScene;
    let mut offscreen = None;
    let mut display_list = Some(list);
    let mut children = [];
    let mut is_dirty = true;
    let mut retry_count = 0;

    rasterize_picture_to_offscreen(
        &mut engine,
        NodeId::new(1),
        &Rect::new(10.0, 10.0, 16.0, 16.0),
        &mut offscreen,
        &mut display_list,
        &mut children,
        &mut is_dirty,
        &scene,
        &DirtyRegion::full(),
        16,
        16,
        &mut retry_count,
        &env,
    )
    .expect("cached Picture must rasterize");

    assert_eq!(engine.encoded_picture_executions, 1);
    assert!(!is_dirty);
    let handle = offscreen.expect("Picture target");
    let (pixels, stride) = engine
        .inner
        .copy_offscreen_pixels(&handle)
        .expect("encoded Picture pixels");
    assert_eq!(
        pixels[8 * stride as usize + 8],
        crate::draw::Color::red().premultiplied()
    );
}

#[test]
fn production_compositor_flattens_cached_picture_glyph_ir_into_main_frame() {
    use crate::draw::command::recorder::CommandRecorder;
    use crate::draw::command::{FrameCommand, FrameRasterOp, FrameRect};
    use crate::draw::resources::font::text_backend::{PositionedGlyph, TextLayout};
    use std::sync::Arc;

    let mut engine = CommandRecorder::new();
    engine.initialize(96, 48).expect("recorder engine");
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic glyph font");
    let font_size = 18.0;
    let glyph_id = 65;
    let raster = fonts.rasterize_glyph(&font, glyph_id, font_size);
    assert!(raster.width > 0 && raster.height > 0);
    let mut list = DisplayList::new();
    list.push(PaintOp::BlitGlyphLayout {
        layout: TextLayout {
            glyphs: vec![PositionedGlyph {
                x: 0.0,
                y: 0.0,
                width: raster.width as f32,
                height: raster.height as f32,
                glyph_id,
                char_index: 0,
                font,
            }],
            lines: Vec::new(),
            width: raster.width as f32,
            height: raster.height as f32,
        },
        pos: Point::new(16.0, 12.0),
        color: crate::draw::Color::from_rgba(220, 96, 40, 160),
        font_size,
    });

    let images = ImageService::new();
    let tokens = TestTokens;
    let env = LayerRenderEnv {
        font,
        font_service: &fonts,
        image_service: &images,
        tokens: &tokens,
        dpi: 96.0,
        dpr: 1.0,
        orientation: Orientation::YDown,
    };
    let scene = CachedPictureScene;
    let bounds = Rect::new(10.0, 8.0, 64.0, 32.0);
    let mut offscreen = None;
    let mut display_list = Some(list);
    let mut children = [];
    let mut is_dirty = true;
    let mut retry_count = 0;
    rasterize_picture_to_offscreen(
        &mut engine,
        NodeId::new(1),
        &bounds,
        &mut offscreen,
        &mut display_list,
        &mut children,
        &mut is_dirty,
        &scene,
        &DirtyRegion::full(),
        64,
        32,
        &mut retry_count,
        &env,
    )
    .expect("rasterize cached glyph Picture");
    assert!(!is_dirty);

    engine.begin_recording(true).expect("begin main frame");
    blit_picture_cache(
        &mut engine,
        &offscreen.expect("Picture handle"),
        &bounds,
        64,
        32,
    )
    .expect("flatten Picture cache");
    let encoder = engine.finish_recording().expect("finish main frame");
    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, clip },
            } if *clip == FrameRect::new(10, 8, 64, 32) => Some(glyphs),
            _ => None,
        })
        .expect("Picture glyph must reach the final main encoder");
    assert_eq!(glyphs.len(), 1);
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &raster.coverage));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::CpuSegment { .. } | FrameCommand::PictureBlit { .. }
    )));
}
