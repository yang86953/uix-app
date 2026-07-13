use crate::tests::common::*;
use crate::draw::primitives::path::{Path, PathBuilder};
use std::any::Any;
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{ EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameRasterOp, ReferenceFrame };
use crate::draw::primitives::tessellator;
use crate::draw::primitives::types::{ BlendMode, GradientDirection, Radius, Transform };
use crate::draw::traits::Canvas2D;
use crate::native::traits::present::{ GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh, GpuSolidRect, GpuStrokeRect, OffscreenTargetId };
use crate::draw::backend::native_gpu::*;

#[test]
fn soft_fallback_tile_is_tight_and_ignores_transparent_rgb() {
    let mut pixels = vec![0u32; 8 * 6];
    pixels[8 + 2] = 0xFF00_0001;
    pixels[3 * 8 + 5] = 0x8000_0002;
    pixels[5 * 8 + 7] = 0x0000_00FF;

    let (packed, tile) = pack_visible_soft_fallback_tile(&pixels, 8, 6)
        .expect("visible pixels must produce one compact tile");
    assert_eq!(tile, SoftFallbackTile::at_destination(2, 1, 4, 3));
    assert_eq!(packed.len(), 12);
    assert_eq!(packed[0], 0xFF00_0001);
    assert_eq!(packed[11], 0x8000_0002);
    assert_eq!(pack_visible_soft_fallback_tile(&[0; 4], 2, 2), None);
}

#[test]
fn native_gpu_canvas_defers_path_clip_failure_to_the_frame_boundary() {
    let mut canvas = NativeGpuCanvas2D::new(16, 16, NativeRasterCaps::d3d11_full());
    let mut builder = crate::draw::primitives::path::PathBuilder::new();
    builder
        .move_to(2.0, 2.0)
        .line_to(14.0, 2.0)
        .line_to(8.0, 14.0)
        .close();
    canvas.push_clip_path(&builder.build());

    let error = canvas
        .take_deferred_error()
        .expect("path clip must retain a deferred failure");
    assert_eq!(error.code(), Errc::NotImplemented);
}

#[test]
fn native_gpu_canvas_defers_scroll_copy_failure_to_the_frame_boundary() {
    let mut canvas = NativeGpuCanvas2D::new(16, 16, NativeRasterCaps::d3d11_full());
    canvas.scroll_region(Rect::new(0.0, 0.0, 16.0, 16.0), 0.0, 1.0);

    let error = canvas
        .take_deferred_error()
        .expect("scroll copy must retain a deferred failure");
    assert_eq!(error.code(), Errc::NotImplemented);
}

#[test]
fn native_gpu_checked_picture_flush_rejects_an_unimplemented_path_clip() {
    let RecordingFixture { mut backend, .. } = recording_backend(FailStage::None);
    backend.resize(16, 16).expect("resize native backend");
    let handle = backend.create_offscreen(16, 16).expect("Picture target");
    backend
        .try_begin_offscreen_paint(&handle)
        .expect("begin Picture paint");
    let mut builder = crate::draw::primitives::path::PathBuilder::new();
    builder
        .move_to(2.0, 2.0)
        .line_to(14.0, 2.0)
        .line_to(8.0, 14.0)
        .close();
    backend
        .offscreen_canvas(&handle)
        .expect("Picture canvas")
        .push_clip_path(&builder.build());

    let error = backend
        .try_flush_offscreen_paint(&handle)
        .expect_err("unsupported Picture path clip must not flush successfully");
    assert_eq!(error.code(), Errc::NotImplemented);
}

#[test]
fn native_gpu_legacy_present_rejects_an_unimplemented_scroll_copy() {
    let RecordingFixture {
        mut backend,
        stages,
        ..
    } = recording_backend(FailStage::None);
    backend.resize(16, 16).expect("resize native backend");
    backend
        .surface()
        .copy_region(Rect::new(0.0, 0.0, 8.0, 8.0), Point::new(0.0, 1.0));

    let error = backend
        .present(&DamageRegion::full())
        .expect_err("legacy final present must not hide an unsupported scroll copy");
    assert_eq!(error.code(), Errc::NotImplemented);
    assert!(
        !stages.borrow().contains(&"present"),
        "unsupported scroll copy must stop before final present"
    );
}

struct FakeD3d11Context {
    clear_calls: Rc<Cell<usize>>,
    clear_rect_calls: Rc<Cell<usize>>,
    draw_calls: Rc<Cell<usize>>,
    stroke_calls: Rc<Cell<usize>>,
    glyph_calls: Rc<Cell<usize>>,
    linear_calls: Rc<Cell<usize>>,
    radial_calls: Rc<Cell<usize>>,
    mesh_calls: Rc<Cell<usize>>,
    shadow_calls: Rc<Cell<usize>>,
    blit_calls: Rc<Cell<usize>>,
    upload_calls: Rc<Cell<usize>>,
    present_calls: Rc<Cell<usize>>,
    last_draw_count: Rc<Cell<usize>>,
    last_stroke_count: Rc<Cell<usize>>,
    last_glyph_count: Rc<Cell<usize>>,
    last_linear_count: Rc<Cell<usize>>,
    last_radial_count: Rc<Cell<usize>>,
    last_mesh_count: Rc<Cell<usize>>,
    last_shadow_count: Rc<Cell<usize>>,
    width: i32,
    height: i32,
}

impl IGraphicsContext for FakeD3d11Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        let mut caps = NativeRasterCaps::d3d11_full();
        // Fake has no GPU RT; do not advertise Picture offscreen.
        caps.offscreen_targets = false;
        caps
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);

        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        self.present_calls.set(self.present_calls.get() + 1);

        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _w: i32,
        _h: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn clear_render_target(
        &mut self,
        _r: f32,
        _g: f32,
        _b: f32,
        _a: f32,
    ) -> crate::core::Result<()> {
        self.clear_calls.set(self.clear_calls.get() + 1);
        Ok(())
    }

    fn draw_solid_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> crate::core::Result<()> {
        self.draw_calls.set(self.draw_calls.get() + 1);
        self.last_draw_count.set(rects.len());
        Ok(())
    }

    fn draw_stroke_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> crate::core::Result<()> {
        self.stroke_calls.set(self.stroke_calls.get() + 1);
        self.last_stroke_count.set(rects.len());
        Ok(())
    }

    fn draw_glyphs(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> crate::core::Result<()> {
        self.glyph_calls.set(self.glyph_calls.get() + 1);
        self.last_glyph_count.set(glyphs.len());
        Ok(())
    }

    fn draw_linear_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> crate::core::Result<()> {
        self.linear_calls.set(self.linear_calls.get() + 1);
        self.last_linear_count.set(rects.len());
        Ok(())
    }

    fn draw_radial_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> crate::core::Result<()> {
        self.radial_calls.set(self.radial_calls.get() + 1);
        self.last_radial_count.set(grads.len());
        Ok(())
    }

    fn draw_solid_meshes(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> crate::core::Result<()> {
        self.mesh_calls.set(self.mesh_calls.get() + 1);
        self.last_mesh_count.set(meshes.len());
        Ok(())
    }

    fn draw_box_shadows(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> crate::core::Result<()> {
        self.shadow_calls.set(self.shadow_calls.get() + 1);
        self.last_shadow_count.set(shadows.len());
        Ok(())
    }

    fn blit_soft_fallback(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        self.blit_calls.set(self.blit_calls.get() + 1);
        Ok(())
    }

    fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        _tile: SoftFallbackTile,
    ) -> crate::core::Result<()> {
        self.blit_calls.set(self.blit_calls.get() + 1);
        let _ = pixels;
        Ok(())
    }

    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> crate::core::Result<()> {
        self.clear_rect_calls
            .set(self.clear_rect_calls.get() + rects.len());
        Ok(())
    }

    fn upload_surface_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> crate::core::Result<()> {
        assert_eq!(pixels.len(), (width * height) as usize);
        self.upload_calls.set(self.upload_calls.get() + 1);
        Ok(())
    }
}

fn fake_ctx(
    clear_calls: &Rc<Cell<usize>>,
    clear_rect_calls: &Rc<Cell<usize>>,
    draw_calls: &Rc<Cell<usize>>,
    stroke_calls: &Rc<Cell<usize>>,
    glyph_calls: &Rc<Cell<usize>>,
    linear_calls: &Rc<Cell<usize>>,
    radial_calls: &Rc<Cell<usize>>,
    mesh_calls: &Rc<Cell<usize>>,
    shadow_calls: &Rc<Cell<usize>>,
    blit_calls: &Rc<Cell<usize>>,
    upload_calls: &Rc<Cell<usize>>,
    present_calls: &Rc<Cell<usize>>,
    last_draw_count: &Rc<Cell<usize>>,
    last_stroke_count: &Rc<Cell<usize>>,
    last_glyph_count: &Rc<Cell<usize>>,
    last_linear_count: &Rc<Cell<usize>>,
    last_radial_count: &Rc<Cell<usize>>,
    last_mesh_count: &Rc<Cell<usize>>,
    last_shadow_count: &Rc<Cell<usize>>,
) -> FakeD3d11Context {
    FakeD3d11Context {
        clear_calls: Rc::clone(clear_calls),
        clear_rect_calls: Rc::clone(clear_rect_calls),
        draw_calls: Rc::clone(draw_calls),
        stroke_calls: Rc::clone(stroke_calls),
        glyph_calls: Rc::clone(glyph_calls),
        linear_calls: Rc::clone(linear_calls),
        radial_calls: Rc::clone(radial_calls),
        mesh_calls: Rc::clone(mesh_calls),
        shadow_calls: Rc::clone(shadow_calls),
        blit_calls: Rc::clone(blit_calls),
        upload_calls: Rc::clone(upload_calls),
        present_calls: Rc::clone(present_calls),
        last_draw_count: Rc::clone(last_draw_count),
        last_stroke_count: Rc::clone(last_stroke_count),
        last_glyph_count: Rc::clone(last_glyph_count),
        last_linear_count: Rc::clone(last_linear_count),
        last_radial_count: Rc::clone(last_radial_count),
        last_mesh_count: Rc::clone(last_mesh_count),
        last_shadow_count: Rc::clone(last_shadow_count),
        width: 1,
        height: 1,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailStage {
    None,
    ClearTarget,
    ClearRects,
    Native,
    Soft,
    Picture,
    Present,
    Shutdown,
    RestoreSwapchainAfterClear,
}

struct RecordingContext {
    fail_stage: Rc<Cell<FailStage>>,
    stages: Rc<RefCell<Vec<&'static str>>>,
    shutdown_calls: Rc<Cell<usize>>,
    make_current_calls: Rc<Cell<usize>>,
    active_offscreen: bool,
    width: i32,
    height: i32,
}

impl RecordingContext {
    fn record(&self, stage: &'static str) {
        self.stages.borrow_mut().push(stage);
    }

    fn fail_if(&self, stage: FailStage) -> crate::core::Result<()> {
        if self.fail_stage.get() == stage {
            Err(Error::new(
                Errc::InvalidState,
                format!("injected {stage:?} failure"),
            ))
        } else {
            Ok(())
        }
    }
}

impl IGraphicsContext for RecordingContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> crate::core::Result<()> {
        self.resize(width, height)?;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);

        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        self.make_current_calls
            .set(self.make_current_calls.get() + 1);

        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.shutdown_calls.set(self.shutdown_calls.get() + 1);
        self.fail_if(FailStage::Shutdown)
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _w: i32,
        _h: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn clear_render_target(
        &mut self,
        _r: f32,
        _g: f32,
        _b: f32,
        _a: f32,
    ) -> crate::core::Result<()> {
        self.record("clear");
        if self.fail_stage.get() == FailStage::RestoreSwapchainAfterClear {
            return Err(Error::new(
                Errc::InvalidState,
                "injected offscreen clear failure",
            ));
        }
        self.fail_if(FailStage::ClearTarget)
    }

    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _rects: &[GpuSolidRect],
    ) -> crate::core::Result<()> {
        self.record("clear_rects");
        self.fail_if(FailStage::ClearRects)
    }

    fn draw_solid_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuSolidRect],
    ) -> crate::core::Result<()> {
        self.record("solid");
        self.fail_if(FailStage::Native)
    }

    fn draw_stroke_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuStrokeRect],
    ) -> crate::core::Result<()> {
        self.record("stroke");
        Ok(())
    }

    fn draw_glyphs(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _glyphs: &[GpuGlyphBlit],
    ) -> crate::core::Result<()> {
        self.record("glyph");
        Ok(())
    }

    fn draw_linear_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuLinearGradientRect],
    ) -> crate::core::Result<()> {
        self.record("linear");
        Ok(())
    }

    fn draw_radial_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _grads: &[GpuRadialGradient],
    ) -> crate::core::Result<()> {
        self.record("radial");
        Ok(())
    }

    fn draw_solid_meshes(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _meshes: &[GpuSolidMesh],
    ) -> crate::core::Result<()> {
        self.record("mesh");
        Ok(())
    }

    fn draw_box_shadows(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _shadows: &[GpuBoxShadow],
    ) -> crate::core::Result<()> {
        self.record("shadow");
        Ok(())
    }

    fn blit_soft_fallback(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        self.record("soft");
        self.fail_if(FailStage::Soft)
    }

    fn blit_soft_fallback_tile(
        &mut self,
        _pixels: &[u32],
        _tile: SoftFallbackTile,
    ) -> crate::core::Result<()> {
        self.record("soft");
        self.fail_if(FailStage::Soft)
    }

    fn create_offscreen_target(
        &mut self,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<OffscreenTargetId> {
        Ok(OffscreenTargetId(0))
    }

    fn bind_offscreen_target(&mut self, _id: OffscreenTargetId) -> crate::core::Result<()> {
        self.active_offscreen = true;
        Ok(())
    }

    fn bind_swapchain_target(&mut self) -> crate::core::Result<()> {
        if self.active_offscreen
            && self.fail_stage.get() == FailStage::RestoreSwapchainAfterClear
        {
            return Err(Error::new(
                Errc::InvalidState,
                "injected swapchain restore failure",
            ));
        }
        self.active_offscreen = false;
        Ok(())
    }

    fn blit_offscreen_target(
        &mut self,
        _id: OffscreenTargetId,
        _src: Rect,
        _dst: Rect,
    ) -> crate::core::Result<()> {
        self.record("picture");
        self.fail_if(FailStage::Picture)
    }

    fn present(&mut self, _frame: &PresentFrame) -> crate::core::Result<()> {
        self.record("present");
        self.fail_if(FailStage::Present)
    }
}

fn recording_context(
    fail_stage: &Rc<Cell<FailStage>>,
    stages: &Rc<RefCell<Vec<&'static str>>>,
    shutdown_calls: &Rc<Cell<usize>>,
    make_current_calls: &Rc<Cell<usize>>,
) -> RecordingContext {
    RecordingContext {
        fail_stage: Rc::clone(fail_stage),
        stages: Rc::clone(stages),
        shutdown_calls: Rc::clone(shutdown_calls),
        make_current_calls: Rc::clone(make_current_calls),
        active_offscreen: false,
        width: 1,
        height: 1,
    }
}

struct RecordingFixture {
    backend: NativeGpuBackend,
    fail_stage: Rc<Cell<FailStage>>,
    stages: Rc<RefCell<Vec<&'static str>>>,
    shutdown_calls: Rc<Cell<usize>>,
    make_current_calls: Rc<Cell<usize>>,
}

fn recording_backend(fail: FailStage) -> RecordingFixture {
    let fail_stage = Rc::new(Cell::new(fail));
    let stages = Rc::new(RefCell::new(Vec::new()));
    let shutdown_calls = Rc::new(Cell::new(0));
    let make_current_calls = Rc::new(Cell::new(0));
    let backend = NativeGpuBackend::new(Box::new(recording_context(
        &fail_stage,
        &stages,
        &shutdown_calls,
        &make_current_calls,
    )))
    .expect("recording native backend");
    RecordingFixture {
        backend,
        fail_stage,
        stages,
        shutdown_calls,
        make_current_calls,
    }
}

/// 模拟 D3D11 `GetClientRect` 大于 WM_SIZE 事件尺寸。
struct ClientRectLargerContext {
    width: i32,
    height: i32,
    bias_w: i32,
    bias_h: i32,
    dpr: f32,
}

impl IGraphicsContext for ClientRectLargerContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, true, self.dpr)
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> crate::core::Result<()> {
        self.width = (width + self.bias_w).max(1);
        self.height = (height + self.bias_h).max(1);
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        self.width = (width + self.bias_w).max(1);
        self.height = (height + self.bias_h).max(1);

        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }
    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }
    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        Ok(())
    }
    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _w: i32,
        _h: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn device_pixel_ratio(&self) -> f32 {
        self.dpr
    }
    fn clear_render_target(
        &mut self,
        _r: f32,
        _g: f32,
        _b: f32,
        _a: f32,
    ) -> crate::core::Result<()> {
        Ok(())
    }
    fn present(&mut self, _frame: &PresentFrame) -> crate::core::Result<()> {
        Ok(())
    }
}

#[test]
fn native_gpu_resize_aligns_canvas_to_actual_gpu_client_size() {
    let mut backend = NativeGpuBackend::new(Box::new(ClientRectLargerContext {
        width: 800,
        height: 600,
        bias_w: 120,
        bias_h: 80,
        dpr: 1.0,
    }))
    .expect("backend");
    backend.resize(800, 600).expect("resize");
    assert_eq!(backend.width, 920);
    assert_eq!(backend.height, 680);
    assert_eq!(backend.surface.width, 920);
    assert_eq!(backend.surface.height, 680);
    let sz = backend.surface.canvas.surface_size();
    assert_eq!((sz.w as i32, sz.h as i32), (920, 680));
}

#[test]
fn native_gpu_canvas_uses_logical_extent_when_drawable_has_dpr() {
    let backend = NativeGpuBackend::new(Box::new(ClientRectLargerContext {
        width: 1600,
        height: 1200,
        bias_w: 0,
        bias_h: 0,
        dpr: 2.0,
    }))
    .expect("backend");

    assert_eq!((backend.width, backend.height), (800, 600));
    assert_eq!((backend.surface.width, backend.surface.height), (800, 600));
    let size = backend.surface.canvas.surface_size();
    assert_eq!((size.w as i32, size.h as i32), (800, 600));
}

fn assert_soft_offset<F>(draw: F, hit: (usize, usize), miss: (usize, usize))
where
    F: FnOnce(&mut NativeGpuCanvas2D),
{
    let caps = NativeRasterCaps {
        clear_target: true,
        soft_blit: true,
        ..NativeRasterCaps::default()
    };
    let mut canvas = NativeGpuCanvas2D::new(64, 64, caps);
    canvas.set_offset(20.0, 20.0);
    draw(&mut canvas);
    let pixels = canvas.ensure_soft().surface().pixels();
    assert_ne!(pixels[hit.1 * 64 + hit.0] >> 24, 0, "shifted pixel");
    assert_eq!(pixels[miss.1 * 64 + miss.0] >> 24, 0, "unshifted pixel");
}

fn assert_soft_only<F>(draw: F)
where
    F: FnOnce(&mut NativeGpuCanvas2D),
{
    let caps = NativeRasterCaps {
        clear_target: true,
        soft_blit: true,
        ..NativeRasterCaps::default()
    };
    let mut canvas = NativeGpuCanvas2D::new(32, 32, caps);
    draw(&mut canvas);
    assert!(canvas.soft_has_content);
    assert!(canvas.pending_native.is_empty());
}

fn add_test_rect(
    builder: &mut crate::draw::primitives::path::PathBuilder,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    positive_area: bool,
) {
    if positive_area {
        builder
            .move_to(x0, y0)
            .line_to(x1, y0)
            .line_to(x1, y1)
            .line_to(x0, y1)
            .close();
    } else {
        builder
            .move_to(x0, y0)
            .line_to(x0, y1)
            .line_to(x1, y1)
            .line_to(x1, y0)
            .close();
    }
}

#[test]
fn native_gpu_backend_offscreen_follows_native_caps() {
    let clear_calls = Rc::new(Cell::new(0));
    let clear_rect_calls = Rc::new(Cell::new(0));
    let draw_calls = Rc::new(Cell::new(0));
    let stroke_calls = Rc::new(Cell::new(0));
    let glyph_calls = Rc::new(Cell::new(0));
    let linear_calls = Rc::new(Cell::new(0));
    let radial_calls = Rc::new(Cell::new(0));
    let mesh_calls = Rc::new(Cell::new(0));
    let shadow_calls = Rc::new(Cell::new(0));
    let blit_calls = Rc::new(Cell::new(0));
    let upload_calls = Rc::new(Cell::new(0));
    let present_calls = Rc::new(Cell::new(0));
    let last_draw_count = Rc::new(Cell::new(0));
    let last_stroke_count = Rc::new(Cell::new(0));
    let last_glyph_count = Rc::new(Cell::new(0));
    let last_linear_count = Rc::new(Cell::new(0));
    let last_radial_count = Rc::new(Cell::new(0));
    let last_mesh_count = Rc::new(Cell::new(0));
    let last_shadow_count = Rc::new(Cell::new(0));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    assert!(
        !backend.capabilities().offscreen,
        "fake D3D11 must not advertise GPU offscreen without RT API"
    );
    assert!(backend.create_offscreen(8, 8).is_none());
}

#[test]
fn native_gpu_backend_offscreen_create_bind_blit_when_caps_prove_support() {
    struct OffscreenFake {
        next_id: Cell<u32>,
        targets: RefCell<Vec<Option<(i32, i32)>>>,
        bound: Cell<Option<u32>>,
        blits: Rc<Cell<usize>>,
        clears: Rc<Cell<usize>>,
        fail_native: Rc<Cell<bool>>,
        fail_destroy: Rc<Cell<bool>>,
        presents: Rc<Cell<usize>>,
    }
    impl IGraphicsContext for OffscreenFake {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
        }
        fn native_raster_caps(&self) -> NativeRasterCaps {
            let mut caps = NativeRasterCaps::d3d11_full();
            // This is a dedicated in-memory RT fixture, not D3D11's
            // advertised production capability.
            caps.offscreen_targets = true;
            caps
        }
        fn initialize(
            &mut self,
            _: *mut std::ffi::c_void,
            _: i32,
            _: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }
        fn resize(&mut self, _: i32, _: i32) -> crate::core::Result<()> {
            Ok(())
        }
        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn swap_buffers(&mut self, _: PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn read_pixels(
            &mut self,
            _: i32,
            _: i32,
            _: i32,
            _: i32,
        ) -> crate::core::Result<Vec<u32>> {
            Ok(Vec::new())
        }
        fn width(&self) -> i32 {
            64
        }
        fn height(&self) -> i32 {
            64
        }
        fn clear_render_target(&mut self, _: f32, _: f32, _: f32, _: f32) -> Result<(), Error> {
            self.clears.set(self.clears.get() + 1);
            Ok(())
        }
        fn draw_solid_rects(
            &mut self,
            _: f32,
            _: f32,
            _: Option<(i32, i32, i32, i32)>,
            _: &[GpuSolidRect],
        ) -> Result<(), Error> {
            if self.fail_native.get() {
                return Err(Error::new(
                    Errc::PlatformError,
                    "injected offscreen native failure",
                ));
            }
            Ok(())
        }
        fn blit_soft_fallback(&mut self, _: &[u32], _: i32, _: i32) -> Result<(), Error> {
            Ok(())
        }
        fn blit_soft_fallback_tile(
            &mut self,
            _pixels: &[u32],
            _tile: SoftFallbackTile,
        ) -> Result<(), Error> {
            Ok(())
        }
        fn create_offscreen_target(
            &mut self,
            width: i32,
            height: i32,
        ) -> Result<OffscreenTargetId, Error> {
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            let mut targets = self.targets.borrow_mut();
            while targets.len() <= id as usize {
                targets.push(None);
            }
            targets[id as usize] = Some((width, height));
            Ok(OffscreenTargetId(id))
        }
        fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
            if let Some(slot) = self.targets.borrow_mut().get_mut(id.0 as usize) {
                *slot = None;
            }
            if self.bound.get() == Some(id.0) {
                self.bound.set(None);
            }
        }
        fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
            if self.fail_destroy.get() {
                return Err(Error::new(
                    Errc::PlatformError,
                    "injected offscreen destroy failure",
                ));
            }
            self.destroy_offscreen_target(id);
            Ok(())
        }
        fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
            if self
                .targets
                .borrow()
                .get(id.0 as usize)
                .and_then(|t| t.as_ref())
                .is_none()
            {
                return Err(Error::new(Errc::InvalidArgument, "unknown offscreen"));
            }
            self.bound.set(Some(id.0));
            Ok(())
        }
        fn bind_swapchain_target(&mut self) -> Result<(), Error> {
            self.bound.set(None);
            Ok(())
        }
        fn blit_offscreen_target(
            &mut self,
            id: OffscreenTargetId,
            _src: Rect,
            _dst: Rect,
        ) -> Result<(), Error> {
            if self
                .targets
                .borrow()
                .get(id.0 as usize)
                .and_then(|t| t.as_ref())
                .is_none()
            {
                return Err(Error::new(Errc::InvalidArgument, "unknown offscreen"));
            }
            self.blits.set(self.blits.get() + 1);
            Ok(())
        }
        fn present(&mut self, _: &PresentFrame<'_>) -> Result<(), Error> {
            self.presents.set(self.presents.get() + 1);
            Ok(())
        }
    }

    let clears = Rc::new(Cell::new(0));
    let blits = Rc::new(Cell::new(0));
    let fail_native = Rc::new(Cell::new(false));
    let fail_destroy = Rc::new(Cell::new(false));
    let presents = Rc::new(Cell::new(0));
    let fake = OffscreenFake {
        next_id: Cell::new(0),
        targets: RefCell::new(Vec::new()),
        bound: Cell::new(None),
        blits: Rc::clone(&blits),
        clears: Rc::clone(&clears),
        fail_native: Rc::clone(&fail_native),
        fail_destroy: Rc::clone(&fail_destroy),
        presents: Rc::clone(&presents),
    };
    let mut backend = NativeGpuBackend::new(Box::new(fake)).expect("backend");
    assert!(backend.capabilities().offscreen);

    let handle = backend.create_offscreen(16, 16).expect("offscreen");
    assert!(backend.begin_offscreen_paint(&handle));
    assert_eq!(clears.get(), 1);
    {
        let canvas = backend.offscreen_canvas(&handle).expect("canvas");
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 16.0, 16.0),
            Color::from_rgb(10, 20, 30),
            None,
        );
    }
    backend.flush_offscreen_paint(&handle);
    backend.end_offscreen_paint();
    backend.blit_offscreen_src(
        &handle,
        Rect::new(0.0, 0.0, 16.0, 16.0),
        Rect::new(1.0, 2.0, 16.0, 16.0),
    );
    assert_eq!(blits.get(), 1);

    // The checked production boundary must surface this failure before
    // FrameRenderer reaches end_frame/final present. The old void method
    // remains only for legacy callers and is not used by LayerTree.
    backend
        .try_begin_offscreen_paint(&handle)
        .expect("offscreen begin");
    {
        let canvas = backend.offscreen_canvas(&handle).expect("canvas");
        canvas.fill_rect(
            Rect::new(2.0, 2.0, 4.0, 4.0),
            Color::from_rgb(30, 20, 10),
            None,
        );
    }
    fail_native.set(true);
    let error = backend
        .try_flush_offscreen_paint(&handle)
        .expect_err("checked offscreen failure must abort recording");
    assert_eq!(error.code(), Errc::PlatformError);
    backend
        .try_end_offscreen_paint()
        .expect("restore swapchain target");
    assert_eq!(presents.get(), 0, "no final present after checked failure");
    assert!(
        backend.offscreens[handle.0 as usize]
            .as_ref()
            .is_some_and(|off| !off.canvas.pending_native.is_empty()),
        "failed offscreen commands must remain uncommitted for recovery"
    );

    // Legacy callers still retain the failure until final present, rather
    // than silently dropping the Picture contents.
    fail_native.set(false);
    assert!(backend.begin_offscreen_paint(&handle));
    fail_native.set(true);
    backend.flush_offscreen_paint(&handle);
    backend.end_offscreen_paint();
    let error = backend
        .present(&DamageRegion::full())
        .expect_err("offscreen failure must reach final present");
    assert_eq!(error.code(), Errc::PlatformError);
    assert_eq!(presents.get(), 0, "no swapchain present after failure");

    fail_native.set(false);
    fail_destroy.set(true);
    let error = backend
        .try_destroy_offscreen(handle)
        .expect_err("checked offscreen destroy must surface the native failure");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(
        backend.offscreens[handle.0 as usize].is_some(),
        "a failed destroy must retain the target ownership for retry"
    );
    assert!(
        !backend.free_offscreen_ids.contains(&handle.0),
        "a failed destroy must not recycle the still-live handle"
    );

    fail_destroy.set(false);
    backend
        .try_destroy_offscreen(handle)
        .expect("a retained target must be destroyable on retry");
    assert!(backend.offscreens[handle.0 as usize].is_none());
    assert!(backend.free_offscreen_ids.contains(&handle.0));
}

#[test]
fn native_gpu_backend_rejects_context_without_hybrid_baseline() {
    struct GlCaps;
    impl IGraphicsContext for GlCaps {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::OpenGlEs, false, 1.0)
        }
        fn initialize(
            &mut self,
            _: *mut std::ffi::c_void,
            _: i32,
            _: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }
        fn resize(&mut self, _: i32, _: i32) -> crate::core::Result<()> {
            Ok(())
        }
        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn swap_buffers(&mut self, _: PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn read_pixels(
            &mut self,
            _: i32,
            _: i32,
            _: i32,
            _: i32,
        ) -> crate::core::Result<Vec<u32>> {
            Ok(Vec::new())
        }
        fn width(&self) -> i32 {
            1
        }
        fn height(&self) -> i32 {
            1
        }
    }
    let err = match NativeGpuBackend::new(Box::new(GlCaps)) {
        Ok(_) => panic!("expected reject"),
        Err(err) => err,
    };
    assert_eq!(err.code(), Errc::InvalidArgument);

    struct LimitedD3d12;
    impl IGraphicsContext for LimitedD3d12 {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d12, false, 1.0)
        }
        fn native_raster_caps(&self) -> NativeRasterCaps {
            NativeRasterCaps {
                clear_target: true,
                soft_blit: true,
                solid_rects: true,
                ..NativeRasterCaps::default()
            }
        }
        fn initialize(
            &mut self,
            _: *mut std::ffi::c_void,
            _: i32,
            _: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }
        fn resize(&mut self, _: i32, _: i32) -> crate::core::Result<()> {
            Ok(())
        }
        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn swap_buffers(&mut self, _: PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            Ok(())
        }
        fn read_pixels(
            &mut self,
            _: i32,
            _: i32,
            _: i32,
            _: i32,
        ) -> crate::core::Result<Vec<u32>> {
            Ok(Vec::new())
        }
        fn width(&self) -> i32 {
            1
        }
        fn height(&self) -> i32 {
            1
        }
    }
    let backend = NativeGpuBackend::new(Box::new(LimitedD3d12))
        .expect("API-neutral backend must accept a bounded D3D12 capability set");
    assert_eq!(backend.kind(), BackendKind::Gpu);
}

#[test]
fn d3d11_backend_present_uses_native_rects_not_full_upload() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    assert!(!backend.capabilities().partial_redraw);
    backend.resize(64, 48).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.fill_rect(
            Rect::new(4.0, 8.0, 20.0, 12.0),
            Color::from_rgb(255, 0, 0),
            None,
        );
        canvas.fill_rect(
            Rect::new(10.0, 10.0, 8.0, 8.0),
            Color::from_rgb(0, 255, 0),
            Some(Radius::uniform(2.0)),
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(draw_calls.get(), 1);
    assert_eq!(last_draw_count.get(), 2);
    assert_eq!(stroke_calls.get(), 0);
    assert_eq!(glyph_calls.get(), 0);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = PresentMode::Swapchain;
    let _ = (
        clear_rect_calls.get(),
        last_stroke_count.get(),
        last_glyph_count.get(),
    );
}

#[test]
fn native_gpu_soft_fallback_is_lazy_until_first_soft_op() {
    let mut canvas = NativeGpuCanvas2D::new(128, 128, NativeRasterCaps::d3d11_full());
    assert!(
        canvas.soft_fallback.is_none(),
        "pure-native canvas must not allocate CPU soft buffer at construction"
    );
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgb(1, 2, 3),
        None,
    );
    assert!(
        canvas.soft_fallback.is_none(),
        "native solid fill must not allocate soft buffer"
    );
    canvas.fill_ellipse(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white());
    assert!(
        canvas.soft_fallback.is_some(),
        "first soft-only op allocates soft buffer"
    );
    assert!(canvas.soft_has_content);
}

#[test]
fn d3d11_soft_clear_is_transparent_so_blit_does_not_wipe_native() {
    // Soft fallback PixelSurface must clear to A=0; opaque black would
    // SRC_ALPHA-overwrite GPU-native fills/glyphs on blit.
    let mut canvas = NativeGpuCanvas2D::new(8, 8, NativeRasterCaps::d3d11_full());
    // Force allocate then clear — lazy soft starts unallocated.
    let _ = canvas.ensure_soft();
    canvas.clear_soft();
    assert!(
        canvas
            .soft_fallback
            .as_ref()
            .expect("soft allocated")
            .surface()
            .pixels()
            .iter()
            .all(|&p| p == 0x0000_0000),
        "soft clear must be transparent"
    );
    assert!(!canvas.soft_has_content);
}

#[test]
fn native_gpu_canvas_stops_native_recording_after_first_soft_operation() {
    let caps = NativeRasterCaps {
        clear_target: true,
        soft_blit: true,
        solid_rects: true,
        ..NativeRasterCaps::default()
    };
    let mut canvas = NativeGpuCanvas2D::new(32, 32, caps);
    canvas.fill_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::white(), None);
    canvas.stroke_rect(Rect::new(2.0, 2.0, 10.0, 10.0), Color::white(), 1.0, None);
    canvas.fill_linear_gradient(
        Rect::new(0.0, 0.0, 12.0, 12.0),
        Color::white(),
        Color::black(),
        GradientDirection::Horizontal,
    );
    let mut path = crate::draw::primitives::path::PathBuilder::new();
    path.move_to(2.0, 2.0)
        .line_to(12.0, 2.0)
        .line_to(2.0, 12.0)
        .close();
    canvas.fill_path(&path.build(), Color::white(), FillRule::NonZero);
    canvas.fill_rect(Rect::new(16.0, 16.0, 8.0, 8.0), Color::white(), None);

    // Once a soft operation appears, later otherwise-native commands stay
    // soft so Canvas2D draw order is preserved.
    assert_eq!(canvas.pending_native.len(), 1);
    assert!(matches!(
        canvas.pending_native.first(),
        Some(PendingNativeOp::SolidRect(_))
    ));
    assert!(canvas.soft_has_content);
}

#[test]
fn native_gpu_canvas_routes_each_unadvertised_operation_to_soft_fallback() {
    assert_soft_only(|canvas| {
        canvas.fill_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::white(), None)
    });
    assert_soft_only(|canvas| {
        canvas.stroke_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::white(), 1.0, None)
    });
    assert_soft_only(|canvas| {
        canvas.fill_linear_gradient(
            Rect::new(1.0, 1.0, 8.0, 8.0),
            Color::white(),
            Color::black(),
            GradientDirection::Horizontal,
        )
    });
    assert_soft_only(|canvas| {
        canvas.fill_radial_gradient(8.0, 8.0, 0.0, 6.0, Color::white(), Color::black())
    });
    assert_soft_only(|canvas| canvas.blit_glyph(2, 2, &[255; 16], 4, 4, Color::white()));
    assert_soft_only(|canvas| {
        let mut path = crate::draw::primitives::path::PathBuilder::new();
        path.move_to(2.0, 2.0)
            .line_to(12.0, 2.0)
            .line_to(2.0, 12.0)
            .close();
        canvas.fill_path(&path.build(), Color::white(), FillRule::NonZero);
    });
    assert_soft_only(|canvas| {
        canvas.draw_box_shadow(
            Rect::new(4.0, 4.0, 8.0, 8.0),
            2.0,
            1.0,
            1.0,
            Color::white(),
            None,
        )
    });
}

#[test]
fn soft_fallback_applies_canvas_offset_exactly_once() {
    assert_soft_offset(
        |canvas| canvas.fill_rect(Rect::new(2.0, 2.0, 6.0, 6.0), Color::white(), None),
        (24, 24),
        (4, 4),
    );
    assert_soft_offset(
        |canvas| canvas.fill_circle(5.0, 5.0, 3.0, Color::white()),
        (25, 25),
        (5, 5),
    );
    assert_soft_offset(
        |canvas| canvas.stroke_rect(Rect::new(2.0, 2.0, 8.0, 8.0), Color::white(), 2.0, None),
        (22, 25),
        (2, 5),
    );
    assert_soft_offset(
        |canvas| {
            canvas.fill_linear_gradient(
                Rect::new(2.0, 2.0, 8.0, 8.0),
                Color::white(),
                Color::black(),
                GradientDirection::Horizontal,
            )
        },
        (24, 24),
        (4, 4),
    );
    assert_soft_offset(
        |canvas| {
            canvas.fill_radial_gradient(6.0, 6.0, 0.0, 4.0, Color::white(), Color::black())
        },
        (26, 26),
        (6, 6),
    );
}

#[test]
fn native_gpu_canvas_flushes_native_operations_in_recorded_order() {
    let fail_stage = Rc::new(Cell::new(FailStage::None));
    let stages = Rc::new(RefCell::new(Vec::new()));
    let shutdown_calls = Rc::new(Cell::new(0));
    let make_current_calls = Rc::new(Cell::new(0));
    let mut context =
        recording_context(&fail_stage, &stages, &shutdown_calls, &make_current_calls);
    let mut canvas = NativeGpuCanvas2D::new(32, 32, NativeRasterCaps::d3d11_full());
    canvas.stroke_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::white(), 1.0, None);
    canvas.fill_rect(Rect::new(2.0, 2.0, 8.0, 8.0), Color::white(), None);
    canvas.fill_linear_gradient(
        Rect::new(3.0, 3.0, 8.0, 8.0),
        Color::white(),
        Color::black(),
        GradientDirection::Horizontal,
    );
    canvas.fill_rect(Rect::new(4.0, 4.0, 8.0, 8.0), Color::white(), None);

    canvas.submit_native(&mut context).expect("submit native");

    assert_eq!(
        stages.borrow().as_slice(),
        ["stroke", "solid", "linear", "solid"]
    );
    assert_eq!(canvas.pending_native.len(), 4);
    canvas.commit_presented_frame();
    assert!(canvas.pending_native.is_empty());
}

#[test]
fn native_gpu_backend_propagates_each_present_stage_error() {
    let RecordingFixture {
        mut backend,
        fail_stage,
        stages,
        ..
    } = recording_backend(FailStage::ClearTarget);
    backend.resize(16, 16).expect("resize");
    assert!(backend.present(&DamageRegion::full()).is_err());
    assert_eq!(stages.borrow().as_slice(), ["clear"]);
    assert!(backend.surface.needs_gpu_clear);
    fail_stage.set(FailStage::None);
    stages.borrow_mut().clear();
    backend.present(&DamageRegion::full()).expect("retry clear");
    assert_eq!(stages.borrow().as_slice(), ["clear", "present"]);

    let RecordingFixture {
        mut backend,
        fail_stage,
        stages,
        ..
    } = recording_backend(FailStage::None);
    backend.resize(16, 16).expect("resize");
    backend.present(&DamageRegion::full()).expect("prime frame");
    stages.borrow_mut().clear();
    backend.surface.clear_rect_raw(1, 1, 4, 4);
    fail_stage.set(FailStage::ClearRects);
    assert!(backend.present(&DamageRegion::full()).is_err());
    assert_eq!(stages.borrow().as_slice(), ["clear_rects"]);
    assert!(backend.surface.needs_gpu_clear);
    assert_eq!(backend.surface.pending_clear_rects.len(), 1);
    fail_stage.set(FailStage::None);
    stages.borrow_mut().clear();
    backend
        .present(&DamageRegion::full())
        .expect("retry clear rects as full clear");
    assert_eq!(stages.borrow().as_slice(), ["clear", "present"]);
    assert!(backend.surface.pending_clear_rects.is_empty());

    let RecordingFixture {
        mut backend,
        fail_stage,
        stages,
        ..
    } = recording_backend(FailStage::Native);
    backend.resize(16, 16).expect("resize");
    backend
        .surface
        .canvas
        .fill_rect(Rect::new(1.0, 1.0, 4.0, 4.0), Color::white(), None);
    assert!(backend.present(&DamageRegion::full()).is_err());
    assert_eq!(stages.borrow().as_slice(), ["clear", "solid"]);
    assert!(backend.surface.needs_gpu_clear);
    assert_eq!(backend.surface.canvas.pending_native.len(), 1);
    fail_stage.set(FailStage::None);
    stages.borrow_mut().clear();
    backend
        .present(&DamageRegion::full())
        .expect("retry native submission");
    assert_eq!(stages.borrow().as_slice(), ["clear", "solid", "present"]);
    assert!(backend.surface.canvas.pending_native.is_empty());

    let RecordingFixture {
        mut backend,
        fail_stage,
        stages,
        ..
    } = recording_backend(FailStage::Soft);
    backend.resize(16, 16).expect("resize");
    backend
        .surface
        .canvas
        .fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
    backend
        .surface
        .canvas
        .fill_ellipse(Rect::new(1.0, 1.0, 4.0, 4.0), Color::white());
    assert!(backend.present(&DamageRegion::full()).is_err());
    assert_eq!(stages.borrow().as_slice(), ["clear", "solid", "soft"]);
    assert!(backend.surface.needs_gpu_clear);
    assert_eq!(backend.surface.canvas.pending_native.len(), 1);
    assert!(backend.surface.canvas.soft_has_content);
    fail_stage.set(FailStage::None);
    stages.borrow_mut().clear();
    backend
        .present(&DamageRegion::full())
        .expect("retry native plus soft submission");
    assert_eq!(
        stages.borrow().as_slice(),
        ["clear", "solid", "soft", "present"]
    );
    assert!(backend.surface.canvas.pending_native.is_empty());
    assert!(!backend.surface.canvas.soft_has_content);

    let RecordingFixture {
        mut backend,
        fail_stage,
        stages,
        ..
    } = recording_backend(FailStage::Present);
    backend.resize(16, 16).expect("resize");
    backend
        .surface
        .canvas
        .fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
    backend
        .surface
        .canvas
        .fill_ellipse(Rect::new(1.0, 1.0, 4.0, 4.0), Color::white());
    assert!(backend.present(&DamageRegion::full()).is_err());
    assert_eq!(
        stages.borrow().as_slice(),
        ["clear", "solid", "soft", "present"]
    );
    assert!(backend.surface.needs_gpu_clear);
    assert_eq!(backend.surface.canvas.pending_native.len(), 1);
    assert!(backend.surface.canvas.soft_has_content);
    fail_stage.set(FailStage::None);
    stages.borrow_mut().clear();
    backend
        .present(&DamageRegion::full())
        .expect("retry after present failure");
    assert_eq!(
        stages.borrow().as_slice(),
        ["clear", "solid", "soft", "present"]
    );
    assert!(backend.surface.canvas.pending_native.is_empty());
    assert!(!backend.surface.canvas.soft_has_content);
}

#[test]
fn native_gpu_rejects_cpu_additive_fallback_instead_of_alpha_over_approximation() {
    let RecordingFixture {
        mut backend,
        stages,
        ..
    } = recording_backend(FailStage::None);
    backend.resize(32, 32).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.set_blend_mode(BlendMode::Additive);
        canvas.fill_ellipse(Rect::new(4.0, 4.0, 16.0, 16.0), Color::blue());
    }

    let error = backend
        .present(&DamageRegion::full())
        .expect_err("transparent CPU overlay must not approximate Additive");
    assert_eq!(error.code(), Errc::NotImplemented);
    assert!(
        !stages.borrow().contains(&"present"),
        "final present must not run after unsupported hybrid blend"
    );
    assert!(backend.surface.canvas.soft_has_content);
}

#[test]
fn native_picture_boundary_commits_prior_native_work_without_an_extra_present() {
    let RecordingFixture {
        mut backend,
        stages,
        ..
    } = recording_backend(FailStage::None);
    backend.resize(16, 16).expect("resize");
    let picture = backend.create_offscreen(4, 4).expect("picture target");

    // Native -> Picture -> Native is the exact mixed sequence that used
    // to reorder: the old code performed Picture immediately but delayed
    // both native batches and the clear until final present.
    backend
        .surface
        .canvas
        .fill_rect(Rect::new(0.0, 0.0, 4.0, 4.0), Color::red(), None);
    backend.blit_offscreen_src(
        &picture,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        Rect::new(2.0, 2.0, 4.0, 4.0),
    );
    backend
        .surface
        .canvas
        .fill_rect(Rect::new(4.0, 4.0, 4.0, 4.0), Color::blue(), None);
    backend
        .present(&DamageRegion::full())
        .expect("one final present");

    assert_eq!(
        stages.borrow().as_slice(),
        ["clear", "solid", "picture", "solid", "present"],
        "Picture is an ordered frame command, not an independent present boundary"
    );
}

#[test]
fn main_frame_encoder_executes_each_command_at_its_recorded_boundary() {
    use crate::draw::pipeline::{FrameImage, FrameRasterOp, FrameRect};

    let RecordingFixture {
        mut backend,
        stages,
        ..
    } = recording_backend(FailStage::None);
    backend.resize(16, 16).expect("resize");

    let mut encoder = FrameEncoder::new(16, 16).expect("encoder");
    encoder.clear(Color::from_rgb(12, 20, 32));
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(1, 2, 3, 4),
        color: Color::from_rgb(220, 40, 80),
    });
    encoder.cpu_segment([FrameRasterOp::FillRect {
        rect: FrameRect::new(5, 6, 3, 4),
        color: Color::from_rgba(20, 180, 240, 160),
    }]);
    encoder.blit_picture(
        FrameImage::solid(2, 2, Color::from_rgba(180, 220, 40, 192)).expect("picture image"),
        FrameRect::new(0, 0, 2, 2),
        FrameRect::new(9, 10, 2, 2),
    );

    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute encoded main frame"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(
        stages.borrow().as_slice(),
        ["clear", "solid", "soft", "soft"],
        "each encoded command must reach its matching native boundary in painter order"
    );
}

#[test]
fn native_picture_failure_is_reported_from_the_final_present_boundary() {
    let RecordingFixture {
        mut backend,
        stages,
        ..
    } = recording_backend(FailStage::Picture);
    backend.resize(16, 16).expect("resize");
    let picture = backend.create_offscreen(4, 4).expect("picture target");
    backend.blit_offscreen_src(
        &picture,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        Rect::new(0.0, 0.0, 4.0, 4.0),
    );

    let error = backend
        .present(&DamageRegion::full())
        .expect_err("draw-time Picture failure must fail the frame");

    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(stages.borrow().as_slice(), ["clear", "picture"]);
    assert!(
        backend.surface.needs_gpu_clear,
        "retry must restart from clear"
    );
}

#[test]
fn native_gpu_backend_shutdown_is_idempotent_across_drop() {
    let RecordingFixture {
        mut backend,
        shutdown_calls,
        make_current_calls,
        ..
    } = recording_backend(FailStage::None);
    backend.try_shutdown().expect("checked shutdown");
    backend.try_shutdown().expect("checked shutdown retry");
    drop(backend);

    assert_eq!(shutdown_calls.get(), 1);
    assert_eq!(make_current_calls.get(), 0);
}

#[test]
fn native_gpu_backend_retries_a_checked_context_shutdown_failure() {
    let RecordingFixture {
        mut backend,
        fail_stage,
        shutdown_calls,
        ..
    } = recording_backend(FailStage::Shutdown);

    let error = backend
        .try_shutdown()
        .expect_err("injected shutdown failure must stay typed");
    assert_eq!(error.message(), "injected Shutdown failure");
    assert_eq!(shutdown_calls.get(), 1);
    assert!(!backend.shutdown);

    fail_stage.set(FailStage::None);
    backend.try_shutdown().expect("checked shutdown retry");
    assert_eq!(shutdown_calls.get(), 2);
    assert!(backend.shutdown);

    drop(backend);
    assert_eq!(shutdown_calls.get(), 2);
}

#[test]
fn native_gpu_backend_surfaces_swapchain_restore_failure_after_offscreen_clear_fails() {
    let RecordingFixture {
        mut backend,
        fail_stage,
        ..
    } = recording_backend(FailStage::RestoreSwapchainAfterClear);
    let handle = backend
        .create_offscreen(16, 16)
        .expect("create Picture target");

    let error = backend
        .try_begin_offscreen_paint(&handle)
        .expect_err("swapchain restoration failure must be surfaced");

    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(error.message(), "injected swapchain restore failure");
    assert_eq!(
        error.root_cause().message(),
        "injected offscreen clear failure"
    );
    assert!(backend.active_offscreen.is_none());

    fail_stage.set(FailStage::None);
    backend.try_shutdown().expect("checked shutdown");
}

#[test]
fn d3d11_backend_native_strokes_without_soft_blit() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(96, 64).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.stroke_rect(
            Rect::new(8.0, 8.0, 40.0, 24.0),
            Color::from_rgb(0, 128, 255),
            2.0,
            Some(Radius::uniform(4.0)),
        );
        canvas.stroke_circle(70.0, 32.0, 16.0, Color::from_rgb(255, 200, 0), 3.0);
        canvas.draw_line(0.0, 50.0, 90.0, 50.0, Color::black(), 2.0);
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(stroke_calls.get(), 1);
    assert_eq!(last_stroke_count.get(), 2);
    // Axis-aligned draw_line → solid fill batch.
    assert_eq!(draw_calls.get(), 1);
    assert_eq!(last_draw_count.get(), 1);
    assert_eq!(glyph_calls.get(), 0);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = (clear_rect_calls.get(), last_glyph_count.get());
}

#[test]
fn d3d11_backend_native_glyphs_without_soft_blit() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(64, 48).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
        let cov = [255u8; 4 * 6];
        canvas.blit_glyph(10, 12, &cov, 4, 6, Color::from_rgb(0, 0, 0));
        canvas.blit_glyph(20, 12, &cov, 4, 6, Color::from_rgb(32, 32, 32));
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(draw_calls.get(), 1);
    assert_eq!(glyph_calls.get(), 1);
    assert_eq!(last_glyph_count.get(), 2);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = (
        clear_rect_calls.get(),
        stroke_calls.get(),
        last_draw_count.get(),
        last_stroke_count.get(),
    );
}

#[test]
fn d3d11_backend_native_gradients_without_soft_blit() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(128, 96).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.fill_linear_gradient(
            Rect::new(8.0, 8.0, 40.0, 20.0),
            Color::from_rgb(255, 0, 0),
            Color::from_rgb(0, 0, 255),
            GradientDirection::Horizontal,
        );
        canvas.fill_linear_gradient(
            Rect::new(8.0, 40.0, 40.0, 20.0),
            Color::from_rgb(0, 255, 0),
            Color::from_rgb(255, 255, 0),
            GradientDirection::Vertical,
        );
        canvas.fill_radial_gradient(
            96.0,
            48.0,
            4.0,
            24.0,
            Color::from_rgb(255, 255, 0),
            Color::from_rgba(0, 0, 0, 0),
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(linear_calls.get(), 1);
    assert_eq!(last_linear_count.get(), 2);
    assert_eq!(radial_calls.get(), 1);
    assert_eq!(last_radial_count.get(), 1);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = (
        clear_rect_calls.get(),
        draw_calls.get(),
        stroke_calls.get(),
        glyph_calls.get(),
        last_draw_count.get(),
        last_stroke_count.get(),
        last_glyph_count.get(),
        mesh_calls.get(),
        last_mesh_count.get(),
    );
}

#[test]
fn d3d11_backend_routes_supported_and_unsupported_paths_by_topology() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(128, 96).expect("resize");
    {
        let canvas = backend.surface().canvas();
        let mut fill = crate::draw::primitives::path::PathBuilder::new();
        fill.move_to(10.0, 10.0)
            .line_to(50.0, 10.0)
            .line_to(50.0, 40.0)
            .line_to(10.0, 40.0)
            .close();
        canvas.fill_path(&fill.build(), Color::from_rgb(255, 0, 0), FillRule::NonZero);

        let mut stroke = crate::draw::primitives::path::PathBuilder::new();
        stroke
            .move_to(24.0, 24.0)
            .line_to(72.0, 24.0)
            .line_to(72.0, 72.0);
        canvas.stroke_path(
            &stroke.build(),
            Color::from_rgba(0, 128, 255, 128),
            &StrokeOptions {
                width: 16.0,
                cap: crate::draw::primitives::path::LineCap::Butt,
                join: crate::draw::primitives::path::LineJoin::Miter,
                miter_limit: 2.0,
            },
        );

        for (y, cap, join) in [
            (
                12.0,
                crate::draw::primitives::path::LineCap::Square,
                crate::draw::primitives::path::LineJoin::Bevel,
            ),
            (
                44.0,
                crate::draw::primitives::path::LineCap::Round,
                crate::draw::primitives::path::LineJoin::Round,
            ),
        ] {
            let mut variant = crate::draw::primitives::path::PathBuilder::new();
            variant
                .move_to(84.0, y)
                .line_to(108.0, y)
                .line_to(116.0, y + 8.0);
            canvas.stroke_path(
                &variant.build(),
                Color::from_rgba(64, 192, 255, 128),
                &StrokeOptions {
                    width: 6.0,
                    cap,
                    join,
                    miter_limit: 4.0,
                },
            );
        }

        let mut empty = crate::draw::primitives::path::PathBuilder::new();
        empty.move_to(20.0, 80.0).line_to(20.0, 80.0);
        canvas.stroke_path(
            &empty.build(),
            Color::from_rgba(255, 255, 255, 128),
            &StrokeOptions {
                width: 8.0,
                cap: crate::draw::primitives::path::LineCap::Round,
                join: crate::draw::primitives::path::LineJoin::Round,
                miter_limit: 4.0,
            },
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(mesh_calls.get(), 1);
    assert_eq!(last_mesh_count.get(), 4);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);

    {
        let canvas = backend.surface().canvas();
        let mut two_holes = crate::draw::primitives::path::PathBuilder::new();
        add_test_rect(&mut two_holes, 4.0, 4.0, 92.0, 92.0, true);
        add_test_rect(&mut two_holes, 12.0, 12.0, 36.0, 36.0, true);
        add_test_rect(&mut two_holes, 60.0, 12.0, 84.0, 36.0, true);
        canvas.fill_path(
            &two_holes.build(),
            Color::from_rgba(64, 192, 255, 180),
            FillRule::EvenOdd,
        );

        let mut deep = crate::draw::primitives::path::PathBuilder::new();
        for (rect, positive) in [
            ((4.0, 4.0, 92.0, 92.0), true),
            ((12.0, 12.0, 84.0, 84.0), true),
            ((28.0, 28.0, 68.0, 68.0), false),
            ((36.0, 36.0, 60.0, 60.0), false),
        ] {
            add_test_rect(&mut deep, rect.0, rect.1, rect.2, rect.3, positive);
        }
        canvas.fill_path(
            &deep.build(),
            Color::from_rgba(192, 96, 255, 180),
            FillRule::NonZero,
        );

        let mut islands = crate::draw::primitives::path::PathBuilder::new();
        add_test_rect(&mut islands, 80.0, 16.0, 112.0, 48.0, true);
        add_test_rect(&mut islands, 4.0, 4.0, 60.0, 60.0, true);
        add_test_rect(&mut islands, 68.0, 4.0, 124.0, 60.0, false);
        add_test_rect(&mut islands, 16.0, 16.0, 48.0, 48.0, false);
        canvas.fill_path(
            &islands.build(),
            Color::from_rgba(96, 255, 160, 180),
            FillRule::NonZero,
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(
        mesh_calls.get(),
        2,
        "contour forests must queue native meshes"
    );
    assert_eq!(last_mesh_count.get(), 3);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 2);

    {
        let canvas = backend.surface().canvas();
        let mut intersecting = crate::draw::primitives::path::PathBuilder::new();
        add_test_rect(&mut intersecting, 8.0, 8.0, 48.0, 40.0, true);
        add_test_rect(&mut intersecting, 32.0, 24.0, 72.0, 56.0, true);
        canvas.fill_path(
            &intersecting.build(),
            Color::from_rgba(0, 255, 0, 128),
            FillRule::NonZero,
        );

        let mut self_intersecting = crate::draw::primitives::path::PathBuilder::new();
        self_intersecting
            .move_to(8.0, 8.0)
            .line_to(56.0, 8.0)
            .line_to(56.0, 40.0)
            .line_to(24.0, 40.0)
            .line_to(24.0, 24.0)
            .line_to(72.0, 24.0)
            .line_to(72.0, 56.0)
            .line_to(8.0, 56.0)
            .close();
        canvas.fill_path(
            &self_intersecting.build(),
            Color::from_rgba(0, 255, 0, 128),
            FillRule::EvenOdd,
        );

        let mut touching = crate::draw::primitives::path::PathBuilder::new();
        add_test_rect(&mut touching, 8.0, 8.0, 32.0, 32.0, true);
        add_test_rect(&mut touching, 32.0, 8.0, 56.0, 32.0, true);
        canvas.fill_path(
            &touching.build(),
            Color::from_rgba(0, 255, 0, 128),
            FillRule::NonZero,
        );

        let mut cancelled = crate::draw::primitives::path::PathBuilder::new();
        add_test_rect(&mut cancelled, 8.0, 8.0, 32.0, 32.0, true);
        add_test_rect(&mut cancelled, 8.0, 8.0, 32.0, 32.0, false);
        canvas.fill_path(
            &cancelled.build(),
            Color::from_rgba(0, 255, 0, 128),
            FillRule::NonZero,
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(
        mesh_calls.get(),
        3,
        "complex paths must queue native meshes"
    );
    assert_eq!(last_mesh_count.get(), 3);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(present_calls.get(), 3);

    {
        let canvas = backend.surface().canvas();
        let mut oversized = crate::draw::primitives::path::PathBuilder::new();
        for index in 0..513 {
            let angle = std::f32::consts::TAU * index as f32 / 513.0;
            let x = 64.0 + angle.cos() * 48.0;
            let y = 48.0 + angle.sin() * 40.0;
            if index == 0 {
                oversized.move_to(x, y);
            } else {
                oversized.line_to(x, y);
            }
        }
        oversized.close();
        canvas.fill_path(
            &oversized.build(),
            Color::from_rgba(255, 160, 32, 180),
            FillRule::NonZero,
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(mesh_calls.get(), 3, "oversized path must stay soft");
    assert_eq!(blit_calls.get(), 1);
    assert_eq!(present_calls.get(), 4);
    let _ = (
        clear_rect_calls.get(),
        draw_calls.get(),
        stroke_calls.get(),
        glyph_calls.get(),
        linear_calls.get(),
        radial_calls.get(),
        shadow_calls.get(),
        last_shadow_count.get(),
    );

    #[cfg(feature = "d3d11")]
    assert_contour_forest_on_real_window();
}

#[cfg(feature = "d3d11")]
fn assert_contour_forest_on_real_window() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("D3D11 path test", 256, 128)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    let context = crate::native::factory::create_gpu_context_with_backend(
        surface,
        256,
        128,
        GraphicsBackend::D3d11,
    )
    .expect("D3D11 context");
    let mut backend = NativeGpuBackend::new(context).expect("D3D11 backend");
    backend.resize(256, 128).expect("resize backend");
    backend
        .gpu_ctx
        .clear_render_target(0.0, 0.0, 0.0, 1.0)
        .expect("clear render target");

    let mut path = crate::draw::primitives::path::PathBuilder::new();
    add_test_rect(&mut path, 8.0, 8.0, 112.0, 120.0, true);
    add_test_rect(&mut path, 16.0, 16.0, 40.0, 40.0, true);
    add_test_rect(&mut path, 64.0, 16.0, 96.0, 40.0, false);
    add_test_rect(&mut path, 136.0, 8.0, 248.0, 120.0, true);
    add_test_rect(&mut path, 144.0, 16.0, 240.0, 112.0, true);
    add_test_rect(&mut path, 168.0, 40.0, 216.0, 88.0, false);
    add_test_rect(&mut path, 176.0, 48.0, 208.0, 80.0, true);
    backend.surface.canvas.fill_path(
        &path.build(),
        Color::from_rgba(0, 255, 0, 128),
        FillRule::EvenOdd,
    );
    assert!(!backend.surface.canvas.soft_has_content);
    assert_eq!(backend.surface.canvas.pending_mesh_count(), 1);

    {
        let NativeGpuBackend {
            gpu_ctx, surface, ..
        } = &mut backend;
        surface
            .canvas
            .submit_native(gpu_ctx.as_mut())
            .expect("submit native contour-forest mesh");
    }
    let pixels = backend
        .gpu_ctx
        .read_pixels(0, 0, 256, 128)
        .expect("D3D11 readback");
    assert_eq!(pixels.len(), 256 * 128);
    let pixel = |x: usize, y: usize| pixels[y * 256 + x];
    let green = pixel(12, 12);
    assert_eq!(green >> 24, 0xFF);
    assert!((127..=129).contains(&((green >> 8) & 0xFF)));
    assert_eq!(pixel(4, 4), 0xFF00_0000, "outside must stay black");
    assert_eq!(pixel(24, 24), 0xFF00_0000, "first hole must stay black");
    assert_eq!(pixel(80, 24), 0xFF00_0000, "second hole must stay black");
    assert_ne!(pixel(52, 24), 0xFF00_0000, "left outer must remain filled");
    assert_ne!(pixel(140, 12), 0xFF00_0000, "deep outer must be filled");
    assert_eq!(pixel(152, 24), 0xFF00_0000, "deep hole must stay black");
    assert_ne!(pixel(172, 44), 0xFF00_0000, "nested island must be filled");
    assert_eq!(pixel(192, 64), 0xFF00_0000, "island hole must stay black");
    let green_pixels = pixels
        .iter()
        .filter(|pixel| ((**pixel >> 8) & 0xFF) != 0)
        .count();
    assert_eq!(
        green_pixels, 14_912,
        "filled pixel count must match geometry"
    );
    assert!(
        pixels.iter().all(|pixel| ((pixel >> 8) & 0xFF) <= 129),
        "triangles must not overlap and blend twice"
    );
    backend
        .gpu_ctx
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present contour-forest frame");
    backend.surface.canvas.commit_presented_frame();

    backend
        .gpu_ctx
        .clear_render_target(0.0, 0.0, 0.0, 1.0)
        .expect("clear stroke frame");
    let mut stroke = crate::draw::primitives::path::PathBuilder::new();
    stroke
        .move_to(24.0, 24.0)
        .line_to(72.0, 24.0)
        .line_to(72.0, 72.0);
    backend.surface.canvas.stroke_path(
        &stroke.build(),
        Color::from_rgba(0, 255, 0, 128),
        &StrokeOptions {
            width: 16.0,
            cap: crate::draw::primitives::path::LineCap::Butt,
            join: crate::draw::primitives::path::LineJoin::Miter,
            miter_limit: 2.0,
        },
    );
    assert!(!backend.surface.canvas.soft_has_content);
    assert_eq!(backend.surface.canvas.pending_mesh_count(), 1);
    {
        let NativeGpuBackend {
            gpu_ctx, surface, ..
        } = &mut backend;
        surface
            .canvas
            .submit_native(gpu_ctx.as_mut())
            .expect("submit native stroke mesh");
    }
    let pixels = backend
        .gpu_ctx
        .read_pixels(0, 0, 256, 128)
        .expect("D3D11 readback");
    let pixel = |x: usize, y: usize| pixels[y * 256 + x];
    for (x, y) in [(32, 24), (76, 20), (68, 28)] {
        let sample = pixel(x, y);
        assert_eq!(sample >> 24, 0xFF);
        assert!(
            (127..=129).contains(&((sample >> 8) & 0xFF)),
            "stroke sample ({x}, {y}) must blend exactly once"
        );
    }
    assert_eq!(pixel(8, 8), 0xFF00_0000, "outside must stay black");
    assert_eq!(
        pixel(60, 36),
        0xFF00_0000,
        "inside the elbow exterior must stay black"
    );
    let green_pixels = pixels
        .iter()
        .filter(|pixel| ((**pixel >> 8) & 0xFF) != 0)
        .count();
    assert_eq!(
        green_pixels, 1_536,
        "stroke pixel count must match geometry"
    );
    assert!(
        pixels.iter().all(|pixel| ((pixel >> 8) & 0xFF) <= 129),
        "stroke triangles must not overlap and blend twice"
    );
    backend
        .gpu_ctx
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present stroke frame");
    backend.surface.canvas.commit_presented_frame();

    let mut overlapping = crate::draw::primitives::path::PathBuilder::new();
    add_test_rect(&mut overlapping, 8.0, 8.0, 48.0, 40.0, true);
    add_test_rect(&mut overlapping, 32.0, 24.0, 72.0, 56.0, true);
    assert_complex_fill_frame(
        &mut backend,
        &overlapping.build(),
        FillRule::NonZero,
        2_304,
        &[(16, 16), (40, 32), (64, 48)],
        &[(4, 4)],
        "intersecting contours",
    );

    let mut self_intersecting = crate::draw::primitives::path::PathBuilder::new();
    self_intersecting
        .move_to(8.0, 8.0)
        .line_to(56.0, 8.0)
        .line_to(56.0, 40.0)
        .line_to(24.0, 40.0)
        .line_to(24.0, 24.0)
        .line_to(72.0, 24.0)
        .line_to(72.0, 56.0)
        .line_to(8.0, 56.0)
        .close();
    assert_complex_fill_frame(
        &mut backend,
        &self_intersecting.build(),
        FillRule::EvenOdd,
        2_304,
        &[(16, 16), (64, 32), (16, 48)],
        &[(32, 32)],
        "self-intersecting contour",
    );

    let mut touching = crate::draw::primitives::path::PathBuilder::new();
    add_test_rect(&mut touching, 8.0, 8.0, 32.0, 32.0, true);
    add_test_rect(&mut touching, 32.0, 8.0, 56.0, 32.0, true);
    assert_complex_fill_frame(
        &mut backend,
        &touching.build(),
        FillRule::NonZero,
        1_152,
        &[(31, 16), (32, 16)],
        &[(4, 4)],
        "edge-touching contours",
    );

    backend.try_shutdown().expect("checked shutdown");
    drop(backend);
    window.close().expect("close native window");
}

#[cfg(feature = "d3d11")]
fn assert_complex_fill_frame(
    backend: &mut NativeGpuBackend,
    path: &Path,
    fill_rule: FillRule,
    expected_green_pixels: usize,
    filled_samples: &[(usize, usize)],
    empty_samples: &[(usize, usize)],
    label: &str,
) {
    backend
        .gpu_ctx
        .clear_render_target(0.0, 0.0, 0.0, 1.0)
        .unwrap_or_else(|error| panic!("clear {label} frame: {error}"));
    backend
        .surface
        .canvas
        .fill_path(path, Color::from_rgba(0, 255, 0, 128), fill_rule);
    assert!(
        !backend.surface.canvas.soft_has_content,
        "{label} must stay native"
    );
    assert_eq!(
        backend.surface.canvas.pending_mesh_count(),
        1,
        "{label} must queue one mesh"
    );
    {
        let NativeGpuBackend {
            gpu_ctx, surface, ..
        } = backend;
        surface
            .canvas
            .submit_native(gpu_ctx.as_mut())
            .unwrap_or_else(|error| panic!("submit {label} mesh: {error}"));
    }
    let pixels = backend
        .gpu_ctx
        .read_pixels(0, 0, 256, 128)
        .expect("D3D11 readback");
    let pixel = |x: usize, y: usize| pixels[y * 256 + x];
    for &(x, y) in filled_samples {
        let green = (pixel(x, y) >> 8) & 0xFF;
        assert!(
            (127..=129).contains(&green),
            "{label} sample ({x}, {y}) must blend exactly once"
        );
    }
    for &(x, y) in empty_samples {
        assert_eq!(pixel(x, y), 0xFF00_0000, "{label} empty sample");
    }
    let green_pixels = pixels
        .iter()
        .filter(|pixel| ((**pixel >> 8) & 0xFF) != 0)
        .count();
    assert_eq!(green_pixels, expected_green_pixels, "{label} pixel count");
    assert!(
        pixels.iter().all(|pixel| ((pixel >> 8) & 0xFF) <= 129),
        "{label} triangles must not overlap and blend twice"
    );
    backend
        .gpu_ctx
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .unwrap_or_else(|error| panic!("present {label} frame: {error}"));
    backend.surface.canvas.commit_presented_frame();
}

#[test]
fn d3d11_backend_native_box_shadows_without_soft_blit() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(128, 96).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.draw_box_shadow(
            Rect::new(20.0, 20.0, 48.0, 32.0),
            8.0,
            4.0,
            6.0,
            Color::from_rgba(0, 0, 0, 120),
            Some(Radius::uniform(6.0)),
        );
        canvas.draw_box_shadow_ambient(
            Rect::new(80.0, 20.0, 32.0, 32.0),
            12.0,
            0.0,
            0.0,
            Color::from_rgba(0, 0, 0, 80),
            Some(Radius::uniform(16.0)),
        );
        // Fill on top — same-frame shadow flush precedes fills.
        canvas.fill_rect(
            Rect::new(20.0, 20.0, 48.0, 32.0),
            Color::from_rgb(240, 240, 240),
            Some(Radius::uniform(6.0)),
        );
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(clear_calls.get(), 1);
    assert_eq!(shadow_calls.get(), 1);
    assert_eq!(last_shadow_count.get(), 2);
    assert_eq!(draw_calls.get(), 1);
    assert_eq!(blit_calls.get(), 0);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = (
        clear_rect_calls.get(),
        stroke_calls.get(),
        glyph_calls.get(),
        linear_calls.get(),
        radial_calls.get(),
        mesh_calls.get(),
    );
}

#[test]
fn d3d11_backend_soft_ops_blit_without_full_upload() {
    let clear_calls = Rc::new(Cell::new(0usize));
    let clear_rect_calls = Rc::new(Cell::new(0usize));
    let draw_calls = Rc::new(Cell::new(0usize));
    let stroke_calls = Rc::new(Cell::new(0usize));
    let glyph_calls = Rc::new(Cell::new(0usize));
    let linear_calls = Rc::new(Cell::new(0usize));
    let radial_calls = Rc::new(Cell::new(0usize));
    let mesh_calls = Rc::new(Cell::new(0usize));
    let shadow_calls = Rc::new(Cell::new(0usize));
    let blit_calls = Rc::new(Cell::new(0usize));
    let upload_calls = Rc::new(Cell::new(0usize));
    let present_calls = Rc::new(Cell::new(0usize));
    let last_draw_count = Rc::new(Cell::new(0usize));
    let last_stroke_count = Rc::new(Cell::new(0usize));
    let last_glyph_count = Rc::new(Cell::new(0usize));
    let last_linear_count = Rc::new(Cell::new(0usize));
    let last_radial_count = Rc::new(Cell::new(0usize));
    let last_mesh_count = Rc::new(Cell::new(0usize));
    let last_shadow_count = Rc::new(Cell::new(0usize));
    let mut backend = NativeGpuBackend::new(Box::new(fake_ctx(
        &clear_calls,
        &clear_rect_calls,
        &draw_calls,
        &stroke_calls,
        &glyph_calls,
        &linear_calls,
        &radial_calls,
        &mesh_calls,
        &shadow_calls,
        &blit_calls,
        &upload_calls,
        &present_calls,
        &last_draw_count,
        &last_stroke_count,
        &last_glyph_count,
        &last_linear_count,
        &last_radial_count,
        &last_mesh_count,
        &last_shadow_count,
    )))
    .expect("backend");
    backend.resize(32, 32).expect("resize");
    {
        let canvas = backend.surface().canvas();
        canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
        // Diagonal line stays soft.
        canvas.draw_line(0.0, 0.0, 10.0, 10.0, Color::black(), 1.0);
    }
    backend.present(&DamageRegion::full()).expect("present");
    assert_eq!(draw_calls.get(), 1);
    assert_eq!(stroke_calls.get(), 0);
    assert_eq!(glyph_calls.get(), 0);
    assert_eq!(blit_calls.get(), 1);
    assert_eq!(upload_calls.get(), 0);
    assert_eq!(present_calls.get(), 1);
    let _ = (
        clear_calls.get(),
        clear_rect_calls.get(),
        last_draw_count.get(),
        last_stroke_count.get(),
        last_glyph_count.get(),
        mesh_calls.get(),
        last_mesh_count.get(),
        shadow_calls.get(),
        last_shadow_count.get(),
    );
}

#[cfg(feature = "d3d12")]
#[test]
fn d3d12_warp_real_context_flows_through_gpu_engine_with_mixed_native_and_soft() {
    use crate::draw::gpu_engine::GpuEngine;
    use crate::draw::traits::GraphicsEngine;

    if !crate::native::factory::d3d12_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("D3D12 mixed native/soft test", 120, 80)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null());
    let context = crate::native::factory::create_d3d12_warp_test_context(surface, 120, 80)
        .expect("mandatory D3D12 WARP context");
    let width = context.width();
    let height = context.height();
    assert_eq!(context.graphics_backend(), GraphicsBackend::D3d12);
    assert_eq!(
        context.native_raster_caps(),
        NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        }
    );

    let mut engine = GpuEngine::new(context).expect("D3D12 GpuEngine");
    GraphicsEngine::initialize(&mut engine, width, height).expect("initialize D3D12 engine");
    {
        let backend = engine
            .session_mut()
            .backend_mut()
            .as_any_mut()
            .downcast_mut::<NativeGpuBackend>()
            .expect("D3D12 must use shared NativeGpuBackend");
        assert_eq!(backend.kind(), BackendKind::Gpu);
        assert_eq!(
            backend.capabilities(),
            BackendCapabilities::gpu_full_redraw()
        );
        backend
            .gpu_ctx
            .clear_render_target(0.0, 0.0, 0.0, 1.0)
            .expect("clear real D3D12 backbuffer");
        backend.surface.needs_gpu_clear = false;

        let canvas = &mut backend.surface.canvas;
        canvas.fill_rect(
            Rect::new(0.0, 0.0, width as f32, height as f32),
            Color::from_rgba(0, 0, 0, 255),
            None,
        );
        canvas.push_clip(Rect::new(25.0, 10.0, 45.0, 50.0));
        canvas.fill_rect(
            Rect::new(20.0, 15.0, 60.0, 40.0),
            Color::from_rgba(255, 0, 0, 255),
            Some(Radius::uniform(12.0)),
        );
        canvas.pop_clip();
        canvas.fill_ellipse(
            Rect::new(42.0, 27.0, 16.0, 16.0),
            Color::from_rgba(0, 0, 255, 128),
        );
        assert_eq!(canvas.pending_native.len(), 2);
        assert!(canvas.soft_has_content);

        let NativeGpuBackend {
            gpu_ctx, surface, ..
        } = backend;
        surface
            .canvas
            .submit_native(gpu_ctx.as_mut())
            .expect("submit D3D12 native rounded solids");
        surface
            .canvas
            .submit_soft(gpu_ctx.as_mut())
            .expect("submit D3D12 unsupported ellipse through soft blit");
        let pixels = gpu_ctx
            .read_pixels(0, 0, width, height)
            .expect("D3D12 readback");
        assert_eq!(pixels.len(), (width * height) as usize);
        let pixel = |x: usize, y: usize| pixels[y * width as usize + x];
        assert_eq!(pixel(5, 5), 0xFF00_0000, "outside stays background");
        assert_eq!(
            pixel(25, 15),
            0xFF00_0000,
            "rounded corner stays background"
        );
        assert_eq!(pixel(75, 35), 0xFF00_0000, "clip excludes native rect");
        assert_eq!(
            pixel(35, 35),
            0xFFFF_0000,
            "transparent soft keeps native red"
        );
        let mixed = pixel(50, 35);
        assert_eq!(mixed >> 24, 0xFF);
        assert!((127..=128).contains(&((mixed >> 16) & 0xFF)));
        assert_eq!((mixed >> 8) & 0xFF, 0);
        assert!((127..=128).contains(&(mixed & 0xFF)));

        gpu_ctx
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect("present verified D3D12 mixed frame");
        surface.canvas.commit_presented_frame();
        assert!(surface.canvas.pending_native.is_empty());
        assert!(!surface.canvas.soft_has_content);
    }
    engine.try_shutdown().expect("checked shutdown");
    drop(engine);
    window.close().expect("close window after D3D12 engine");
}

#[cfg(feature = "d3d11")]
fn record_common_hybrid_clip_scene(canvas: &mut dyn Canvas2D) {
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 96.0, 64.0),
        Color::from_rgba(0, 0, 0, 255),
        None,
    );
    canvas.fill_rect(
        Rect::new(16.0, 12.0, 48.0, 32.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    canvas.push_clip(Rect::new(28.0, 18.0, 24.0, 20.0));
    canvas.fill_ellipse(
        Rect::new(20.0, 10.0, 40.0, 40.0),
        Color::from_rgba(0, 0, 255, 128),
    );
    canvas.pop_clip();
}

#[cfg(feature = "d3d11")]
fn common_hybrid_probes(pixels: &[u32], stride: usize) -> Vec<u32> {
    [(4usize, 4usize), (20, 16), (22, 12), (40, 28), (85, 55)]
        .into_iter()
        .map(|(x, y)| pixels[y * stride + x])
        .collect()
}

#[cfg(feature = "d3d11")]
fn assert_premultiplied_probes_match(reference: &[u32], actual: &[u32], label: &str) {
    assert_eq!(reference.len(), actual.len(), "{label}: probe count");
    for (index, (expected, observed)) in reference.iter().zip(actual).enumerate() {
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xff) as i16;
            let observed_channel = ((observed >> shift) & 0xff) as i16;
            assert!(
                (expected_channel - observed_channel).abs() <= 1,
                "{label}: probes expected={reference:?}, actual={actual:?}; probe {index}, channel {shift}: expected {expected:#010X}, got {observed:#010X}",
            );
        }
    }
}

#[cfg(feature = "d3d11")]
fn software_common_hybrid_clip_reference() -> Vec<u32> {
    let mut backend = crate::draw::backend::CpuBackend::new();
    backend.resize(96, 64).expect("resize software reference");
    record_common_hybrid_clip_scene(backend.surface().canvas());
    backend.pixels().to_vec()
}

#[cfg(feature = "d3d11")]
fn record_offscreen_crop_order_script(backend: &mut dyn RenderBackend) -> Result<(), Error> {
    let picture = backend
        .create_offscreen(32, 24)
        .expect("offscreen target must be available");
    backend.try_begin_offscreen_paint(&picture)?;
    {
        let canvas = backend
            .offscreen_canvas(&picture)
            .expect("offscreen canvas must be available");
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 32.0, 24.0),
            Color::from_rgba(0, 0, 255, 255),
            None,
        );
        canvas.fill_rect(
            Rect::new(8.0, 4.0, 12.0, 12.0),
            Color::from_rgba(255, 0, 0, 255),
            None,
        );
    }
    backend.try_flush_offscreen_paint(&picture)?;
    backend.try_end_offscreen_paint()?;

    backend.surface().canvas().fill_rect(
        Rect::new(0.0, 0.0, 96.0, 64.0),
        Color::from_rgba(0, 0, 0, 255),
        None,
    );
    backend.try_blit_offscreen_src(
        &picture,
        Rect::new(8.0, 4.0, 12.0, 12.0),
        Rect::new(36.0, 16.0, 12.0, 12.0),
    )?;
    let canvas = backend.surface().canvas();
    canvas.push_clip(Rect::new(40.0, 20.0, 8.0, 8.0));
    canvas.fill_rect(
        Rect::new(36.0, 16.0, 12.0, 12.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );
    canvas.pop_clip();
    Ok(())
}

#[cfg(feature = "d3d11")]
fn offscreen_crop_order_probes(pixels: &[u32], stride: usize) -> Vec<u32> {
    [(4usize, 4usize), (38, 18), (42, 22), (50, 30)]
        .into_iter()
        .map(|(x, y)| pixels[y * stride + x])
        .collect()
}

#[cfg(feature = "d3d11")]
#[test]
fn d3d11_warp_executes_encoded_picture_in_its_bound_offscreen_target() {
    use crate::draw::pipeline::{FrameImage, FrameRasterOp, FrameRect};

    if !crate::native::factory::d3d11_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("encoded Picture WARP test", 96, 64)
        .expect("window");
    let context = crate::native::factory::create_d3d11_warp_test_context(
        window.native_surface_ptr(),
        96,
        64,
    )
    .expect("D3D11 WARP context");
    let mut native = NativeGpuBackend::new(context).expect("native WARP backend");
    native.resize(96, 64).expect("resize native WARP backend");

    let picture = native.create_offscreen(32, 24).expect("Picture target");
    native
        .try_begin_offscreen_paint(&picture)
        .expect("bind Picture target");
    let mut encoder = FrameEncoder::new(32, 24).expect("FrameEncoder");
    encoder.clear(Color::transparent());
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(2, 2, 4, 4),
        color: Color::red(),
    });
    encoder.cpu_segment([FrameRasterOp::FillRect {
        rect: FrameRect::new(8, 4, 16, 12),
        color: Color::blue(),
    }]);
    encoder.blit_picture(
        FrameImage::solid(2, 2, Color::green()).expect("Picture image"),
        FrameRect::new(0, 0, 2, 2),
        FrameRect::new(26, 18, 2, 2),
    );
    assert_eq!(
        native
            .try_execute_encoded_picture(&picture, &encoder)
            .expect("execute encoded Picture"),
        EncodedPictureExecution::Executed
    );
    native
        .try_flush_offscreen_paint(&picture)
        .expect("flush encoded Picture");
    native
        .try_end_offscreen_paint()
        .expect("restore swapchain target");
    native
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 32.0, 24.0),
            Rect::new(32.0, 16.0, 32.0, 24.0),
        )
        .expect("blit encoded Picture");

    let reference = encoder.render_reference();
    let stride = native.gpu_ctx.width() as usize;
    let pixels = native.try_readback().expect("read encoded Picture frame");
    assert_premultiplied_probes_match(
        &[
            reference.pixel(3, 3).expect("encoded native pixel"),
            reference.pixel(12, 8).expect("encoded CPU pixel"),
            reference.pixel(26, 18).expect("encoded Picture pixel"),
        ],
        &[
            pixels[19 * stride + 35],
            pixels[24 * stride + 44],
            pixels[34 * stride + 58],
        ],
        "D3D11 WARP encoded Picture",
    );

    native
        .present(&DamageRegion::full())
        .expect("present encoded Picture frame");
    native.destroy_offscreen(picture);
    native.try_shutdown().expect("checked shutdown");
    window.close().expect("close encoded Picture WARP window");
}

#[cfg(feature = "d3d11")]
#[test]
fn d3d11_warp_executes_main_frame_encoder_before_its_only_present() {
    use crate::draw::pipeline::{FrameImage, FrameRasterOp, FrameRect};

    if !crate::native::factory::d3d11_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("main FrameEncoder WARP test", 96, 64)
        .expect("window");
    let context = crate::native::factory::create_d3d11_warp_test_context(
        window.native_surface_ptr(),
        96,
        64,
    )
    .expect("D3D11 WARP context");
    let mut native = NativeGpuBackend::new(context).expect("native WARP backend");
    native.resize(96, 64).expect("resize native WARP backend");
    let (frame_w, frame_h) = (native.width, native.height);

    let mut encoder = FrameEncoder::new(frame_w, frame_h).expect("main FrameEncoder");
    encoder.clear(Color::from_rgba(10, 20, 30, 255));
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(8, 8, 12, 12),
        color: Color::from_rgba(40, 220, 80, 255),
    });
    encoder.cpu_segment([FrameRasterOp::FillRect {
        rect: FrameRect::new(24, 16, 32, 24),
        color: Color::from_rgba(220, 40, 80, 192),
    }]);
    encoder.blit_picture(
        FrameImage::solid(2, 2, Color::from_rgba(40, 120, 240, 255))
            .expect("main Picture image"),
        FrameRect::new(0, 0, 2, 2),
        FrameRect::new(72, 48, 2, 2),
    );
    assert_eq!(
        native
            .try_execute_encoded_frame(&encoder)
            .expect("execute main FrameEncoder"),
        EncodedFrameExecution::Executed
    );

    let reference = encoder.render_reference();
    let stride = native.gpu_ctx.width() as usize;
    let pixels = native.try_readback().expect("read main FrameEncoder frame");
    assert_premultiplied_probes_match(
        &[
            reference.pixel(4, 4).expect("background probe"),
            reference.pixel(10, 10).expect("native probe"),
            reference.pixel(40, 28).expect("fill probe"),
            reference.pixel(72, 48).expect("Picture probe"),
        ],
        &[
            pixels[4 * stride + 4],
            pixels[10 * stride + 10],
            pixels[28 * stride + 40],
            pixels[48 * stride + 72],
        ],
        "D3D11 WARP main FrameEncoder before present",
    );

    native
        .present(&DamageRegion::full())
        .expect("single final present after main FrameEncoder");
    native.try_shutdown().expect("checked shutdown");
    window.close().expect("close main FrameEncoder WARP window");
}

#[cfg(feature = "d3d11")]
#[test]
fn d3d11_warp_additive_and_scroll_frame_ops_match_reference_executor() {
    use crate::draw::pipeline::{FrameRasterOp, FrameRect};

    if !crate::native::factory::d3d11_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("additive scroll FrameEncoder WARP", 48, 32)
        .expect("window");
    let context = crate::native::factory::create_d3d11_warp_test_context(
        window.native_surface_ptr(),
        48,
        32,
    )
    .expect("D3D11 WARP context");
    let mut native = NativeGpuBackend::new(context).expect("native WARP backend");
    native.resize(48, 32).expect("resize");
    let (frame_w, frame_h) = (native.width, native.height);

    let mut encoder = FrameEncoder::new(frame_w, frame_h).expect("encoder");
    encoder.clear(Color::from_rgba(20, 40, 60, 255));
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(4, 4, 8, 8),
        color: Color::from_rgba(80, 160, 40, 255),
    });
    encoder.native(FrameRasterOp::FillRectAdditive {
        rect: FrameRect::new(6, 6, 6, 6),
        color: Color::from_rgba(40, 20, 80, 128),
    });
    encoder.native(FrameRasterOp::ScrollCopy {
        viewport: FrameRect::new(0, 0, frame_w, frame_h),
        dx: 0,
        dy: 2,
    });
    assert_eq!(
        native
            .try_execute_encoded_frame(&encoder)
            .expect("execute additive/scroll FrameEncoder"),
        EncodedFrameExecution::Executed
    );

    let reference = encoder.render_reference();
    let stride = native.gpu_ctx.width() as usize;
    let pixels = native.try_readback().expect("readback");
    assert_premultiplied_probes_match(
        &[
            reference.pixel(2, 2).expect("bg"),
            reference.pixel(5, 5).expect("fill"),
            reference.pixel(8, 8).expect("additive"),
            reference.pixel(10, 10).expect("scrolled"),
        ],
        &[
            pixels[2 * stride + 2],
            pixels[5 * stride + 5],
            pixels[8 * stride + 8],
            pixels[10 * stride + 10],
        ],
        "D3D11 WARP additive+scroll FrameEncoder",
    );

    let _ = native.try_shutdown();
    window.close().expect("close");
}

#[cfg(feature = "d3d12")]
#[test]
fn d3d12_warp_additive_and_scroll_frame_ops_match_reference_executor() {
    use crate::draw::pipeline::{FrameRasterOp, FrameRect};

    if !crate::native::factory::d3d12_warp_test_context_available() {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("additive scroll FrameEncoder D3D12 WARP", 48, 32)
        .expect("window");
    let context = crate::native::factory::create_d3d12_warp_test_context(
        window.native_surface_ptr(),
        48,
        32,
    )
    .expect("D3D12 WARP context");
    let mut native = NativeGpuBackend::new(context).expect("native WARP backend");
    native.resize(48, 32).expect("resize");
    let (frame_w, frame_h) = (native.width, native.height);

    let mut encoder = FrameEncoder::new(frame_w, frame_h).expect("encoder");
    encoder.clear(Color::from_rgba(20, 40, 60, 255));
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(4, 4, 8, 8),
        color: Color::from_rgba(80, 160, 40, 255),
    });
    encoder.native(FrameRasterOp::FillRectAdditive {
        rect: FrameRect::new(6, 6, 6, 6),
        color: Color::from_rgba(40, 20, 80, 128),
    });
    encoder.native(FrameRasterOp::ScrollCopy {
        viewport: FrameRect::new(0, 0, frame_w, frame_h),
        dx: 0,
        dy: 2,
    });
    assert_eq!(
        native
            .try_execute_encoded_frame(&encoder)
            .expect("execute additive/scroll FrameEncoder"),
        EncodedFrameExecution::Executed
    );

    let reference = encoder.render_reference();
    let stride = native.gpu_ctx.width() as usize;
    let pixels = native.try_readback().expect("readback");
    assert_premultiplied_probes_match(
        &[
            reference.pixel(2, 2).expect("bg"),
            reference.pixel(5, 5).expect("fill"),
            reference.pixel(8, 8).expect("additive"),
            reference.pixel(10, 10).expect("scrolled"),
        ],
        &[
            pixels[2 * stride + 2],
            pixels[5 * stride + 5],
            pixels[8 * stride + 8],
            pixels[10 * stride + 10],
        ],
        "D3D12 WARP additive+scroll FrameEncoder",
    );

    let _ = native.try_shutdown();
    window.close().expect("close");
}

#[test]
fn destination_dependent_frame_ops_use_readback_apply_upload_on_pixel_context() {
    use crate::draw::pipeline::{FrameRasterOp, FrameRect};

    struct PixelContext {
        width: i32,
        height: i32,
        pixels: Rc<RefCell<Vec<u32>>>,
    }

    impl IGraphicsContext for PixelContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
        }

        fn native_raster_caps(&self) -> NativeRasterCaps {
            NativeRasterCaps::d3d11_full()
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            width: i32,
            height: i32,
        ) -> crate::core::Result<()> {
            self.resize(width, height)
        }

        fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
            self.width = width.max(1);
            self.height = height.max(1);
            *self.pixels.borrow_mut() =
                vec![0; (self.width as usize) * (self.height as usize)];
            Ok(())
        }

        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }

        fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }

        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            Ok(())
        }

        fn read_pixels(
            &mut self,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        ) -> crate::core::Result<Vec<u32>> {
            let pixels = self.pixels.borrow();
            let mut out = Vec::with_capacity((width * height) as usize);
            for row in y..y + height {
                let start = (row * self.width + x) as usize;
                out.extend_from_slice(&pixels[start..start + width as usize]);
            }
            Ok(out)
        }

        fn width(&self) -> i32 {
            self.width
        }

        fn height(&self) -> i32 {
            self.height
        }

        fn clear_render_target(
            &mut self,
            r: f32,
            g: f32,
            b: f32,
            a: f32,
        ) -> crate::core::Result<()> {
            let color = Color::from_rgba(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                (a * 255.0) as u8,
            )
            .premultiplied();
            self.pixels.borrow_mut().fill(color);
            Ok(())
        }

        fn draw_solid_rects(
            &mut self,
            _viewport_w: f32,
            _viewport_h: f32,
            _scissor: Option<(i32, i32, i32, i32)>,
            rects: &[GpuSolidRect],
        ) -> crate::core::Result<()> {
            let mut pixels = self.pixels.borrow_mut();
            for rect in rects {
                let color = Color::from_rgba(
                    (rect.rgba[0] * 255.0) as u8,
                    (rect.rgba[1] * 255.0) as u8,
                    (rect.rgba[2] * 255.0) as u8,
                    (rect.rgba[3] * 255.0) as u8,
                );
                crate::draw::pipeline::frame_encoder::apply_frame_raster_op(
                    self.width,
                    self.height,
                    &mut pixels,
                    &FrameRasterOp::FillRect {
                        rect: FrameRect::new(
                            rect.x as i32,
                            rect.y as i32,
                            rect.w as i32,
                            rect.h as i32,
                        ),
                        color,
                    },
                );
            }
            Ok(())
        }

        fn upload_surface_pixels(
            &mut self,
            pixels: &[u32],
            width: i32,
            height: i32,
        ) -> crate::core::Result<()> {
            assert_eq!((width, height), (self.width, self.height));
            assert_eq!(pixels.len(), (width * height) as usize);
            self.pixels.borrow_mut().copy_from_slice(pixels);
            Ok(())
        }
    }

    let pixels = Rc::new(RefCell::new(Vec::new()));
    let mut backend = NativeGpuBackend::new(Box::new(PixelContext {
        width: 1,
        height: 1,
        pixels: Rc::clone(&pixels),
    }))
    .expect("backend");
    backend.resize(4, 3).expect("resize");

    let mut encoder = FrameEncoder::new(4, 3).expect("encoder");
    encoder.clear(Color::from_rgba(10, 20, 30, 255));
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(0, 0, 2, 2),
        color: Color::from_rgba(100, 0, 0, 255),
    });
    encoder.native(FrameRasterOp::FillRectAdditive {
        rect: FrameRect::new(1, 1, 2, 2),
        color: Color::from_rgba(0, 80, 0, 128),
    });
    encoder.native(FrameRasterOp::ScrollCopy {
        viewport: FrameRect::new(0, 0, 4, 3),
        dx: 1,
        dy: 0,
    });
    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(pixels.borrow().as_slice(), encoder.render_reference().pixels());
}

#[cfg(feature = "d3d11")]
#[test]
fn software_and_d3d11_warp_match_offscreen_source_crop_and_post_blit_clip_script() {
    if !crate::native::factory::d3d11_warp_test_context_available() {
        return;
    }
    let mut software = crate::draw::backend::CpuBackend::new();
    software.resize(96, 64).expect("resize software reference");
    record_offscreen_crop_order_script(&mut software).expect("software offscreen script");
    let reference = offscreen_crop_order_probes(software.pixels(), 96);

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("offscreen crop parity WARP test", 96, 64)
        .expect("window");
    let context = crate::native::factory::create_d3d11_warp_test_context(
        window.native_surface_ptr(),
        96,
        64,
    )
    .expect("D3D11 WARP context");
    let mut native = NativeGpuBackend::new(context).expect("native WARP backend");
    native.resize(96, 64).expect("resize native WARP backend");
    assert!(native.capabilities().offscreen);
    record_offscreen_crop_order_script(&mut native).expect("D3D11 WARP offscreen script");
    let stride = native.gpu_ctx.width() as usize;
    let pixels = native.try_readback().expect("read native WARP frame");
    assert_premultiplied_probes_match(
        &reference,
        &offscreen_crop_order_probes(&pixels, stride),
        "D3D11 WARP offscreen crop",
    );

    window.close().expect("close offscreen crop parity window");
}

#[cfg(all(feature = "d3d11", feature = "d3d12"))]
fn native_common_hybrid_clip_pixels(
    context: Box<dyn IGraphicsContext>,
) -> (Vec<u32>, i32, i32, usize) {
    let mut backend = NativeGpuBackend::new(context).expect("native WARP backend");
    backend.resize(96, 64).expect("resize native WARP backend");
    record_common_hybrid_clip_scene(&mut backend.surface.canvas);
    assert!(
        backend.surface.canvas.soft_has_content,
        "ellipse must use the shared bounded soft fallback path"
    );
    let pixels = backend.try_readback().expect("read native WARP frame");
    let width = backend.gpu_ctx.width();
    let height = backend.gpu_ctx.height();
    let upload_bytes = backend.last_soft_upload_bytes();
    (pixels, width, height, upload_bytes)
}

#[cfg(all(feature = "d3d11", feature = "d3d12"))]
#[test]
fn software_and_warp_backends_match_common_hybrid_clip_and_bounded_tile_script() {
    if !crate::native::factory::d3d11_warp_test_context_available()
        || !crate::native::factory::d3d12_warp_test_context_available()
    {
        return;
    }

    let reference = common_hybrid_probes(&software_common_hybrid_clip_reference(), 96);
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("hybrid parity WARP test", 96, 64)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");

    let d3d11 = crate::native::factory::create_d3d11_warp_test_context(surface, 96, 64)
        .expect("D3D11 WARP context");
    assert_eq!(d3d11.graphics_backend(), GraphicsBackend::D3d11);
    assert!(d3d11.native_raster_caps().offscreen_targets);
    let (d3d11_pixels, d3d11_width, d3d11_height, d3d11_upload_bytes) =
        native_common_hybrid_clip_pixels(d3d11);

    let d3d12 = crate::native::factory::create_d3d12_warp_test_context(surface, 96, 64)
        .expect("D3D12 WARP context");
    assert_eq!(d3d12.graphics_backend(), GraphicsBackend::D3d12);
    assert!(
        !d3d12.native_raster_caps().offscreen_targets,
        "D3D12 must not claim an offscreen API it does not implement"
    );
    let (d3d12_pixels, d3d12_width, d3d12_height, d3d12_upload_bytes) =
        native_common_hybrid_clip_pixels(d3d12);

    assert_premultiplied_probes_match(
        &reference,
        &common_hybrid_probes(&d3d11_pixels, d3d11_width as usize),
        "D3D11 WARP",
    );
    assert_premultiplied_probes_match(
        &reference,
        &common_hybrid_probes(&d3d12_pixels, d3d12_width as usize),
        "D3D12 WARP",
    );
    let d3d11_full_frame_bytes =
        d3d11_width as usize * d3d11_height as usize * std::mem::size_of::<u32>();
    let d3d12_full_frame_bytes =
        d3d12_width as usize * d3d12_height as usize * std::mem::size_of::<u32>();
    assert!(d3d11_upload_bytes > 0 && d3d11_upload_bytes < d3d11_full_frame_bytes);
    assert!(d3d12_upload_bytes > 0 && d3d12_upload_bytes < d3d12_full_frame_bytes);

    window.close().expect("close hybrid parity window");
}
