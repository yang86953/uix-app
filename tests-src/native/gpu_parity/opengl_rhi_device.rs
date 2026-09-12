//! OpenGL ES RHI 对 API 无关全图元规范场景的 EGL 离屏执行与回读 harness。

use glow::HasContext as _;
use khronos_egl as egl;

use super::*;
use crate::core::{Errc, Error, PresentDamage};
use crate::draw::backend::production_chain_parity::execute_ui_production_chain;
use crate::draw::backend::rhi_renderer::consistency::{
    CONSISTENCY_BACKGROUND, CONSISTENCY_EXTENT, ConsistencyBlurScenario, ConsistencySample,
    ConsistencyScene, blur_subregion_scenario, canonical_scenes, production_chain_scene,
    validate_canonical_scenes, validate_production_chain_readback,
};
use crate::native::presentation::graphics::opengl::NativeOpenGlRuntime;
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
use crate::platform::presentation::rhi::{
    DrawBufferBindings, DrawRange, DrawRasterState, DrawSamplingBinding, GraphicsDevice,
    GraphicsSurface, PipelineKind, RhiBufferUpload, RhiPresentTransaction, RhiSurfaceLifecycle,
    RhiSurfaceRecreateReason, RhiSurfaceRecreateTransaction, RhiSurfaceResizeTransaction,
    RhiTextureUpload, RhiViewport, SampledTextureBinding, TextureFormat,
};

// EGL_MESA_platform_surfaceless 的公开平台枚举；只用于无窗口测试 display。
const EGL_PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;

// 在构造、执行和失败展开期间唯一持有测试 EGL display/context/pbuffer。
struct HeadlessEgl {
    api: egl::Instance<egl::Static>,
    display: egl::Display,
    context: Option<egl::Context>,
    surface: Option<egl::Surface>,
    config: Option<egl::Config>,
    active: bool,
    platform: &'static str,
    version: (egl::Int, egl::Int),
}

impl HeadlessEgl {
    // 优先初始化 surfaceless display，并以默认 display 作为同驱动 pbuffer 回退。
    fn new() -> Self {
        let api = egl::Instance::new(egl::Static);
        let mut failures = Vec::new();
        let surfaceless = unsafe {
            // SAFETY: surfaceless 平台要求 EGL_DEFAULT_DISPLAY 和空属性表，不接收 OS 句柄。
            api.get_platform_display(
                EGL_PLATFORM_SURFACELESS_MESA,
                egl::DEFAULT_DISPLAY,
                &[egl::ATTRIB_NONE],
            )
        };
        let initialized = match surfaceless {
            Ok(display) => match api.initialize(display) {
                Ok(version) => Some((display, "surfaceless+pbuffer", version)),
                Err(error) => {
                    failures.push(format!("surfaceless initialize: {error:?}"));
                    None
                }
            },
            Err(error) => {
                failures.push(format!("surfaceless display: {error:?}"));
                None
            }
        };
        let initialized = initialized.or_else(|| {
            let display = unsafe {
                // SAFETY: EGL_DEFAULT_DISPLAY 是 EGL 规范允许的空原生 display 值。
                api.get_display(egl::DEFAULT_DISPLAY)
            };
            match display {
                Some(display) => match api.initialize(display) {
                    Ok(version) => Some((display, "default+pbuffer", version)),
                    Err(error) => {
                        failures.push(format!("default initialize: {error:?}"));
                        None
                    }
                },
                None => {
                    failures.push("default display: NO_DISPLAY".to_owned());
                    None
                }
            }
        });
        let (display, platform, version) = initialized.unwrap_or_else(|| {
            panic!(
                "OpenGL consistency requires a real EGL display: {}",
                failures.join("; ")
            )
        });
        let mut owner = Self {
            api,
            display,
            context: None,
            surface: None,
            config: None,
            active: true,
            platform,
            version,
        };
        owner
            .api
            .bind_api(egl::OPENGL_ES_API)
            .expect("OpenGL consistency must bind the GLES API");
        let config = owner
            .api
            .choose_first_config(
                display,
                &[
                    egl::SURFACE_TYPE,
                    egl::PBUFFER_BIT,
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_ES3_BIT,
                    egl::RED_SIZE,
                    8,
                    egl::GREEN_SIZE,
                    8,
                    egl::BLUE_SIZE,
                    8,
                    egl::ALPHA_SIZE,
                    8,
                    egl::SAMPLE_BUFFERS,
                    0,
                    egl::SAMPLES,
                    0,
                    egl::NONE,
                ],
            )
            .expect("OpenGL consistency EGL config query must succeed")
            .expect("OpenGL consistency requires an RGBA8 GLES3 pbuffer config");
        owner.config = Some(config);
        let surface = owner
            .api
            .create_pbuffer_surface(
                display,
                config,
                &[
                    egl::WIDTH,
                    CONSISTENCY_EXTENT.width as egl::Int,
                    egl::HEIGHT,
                    CONSISTENCY_EXTENT.height as egl::Int,
                    egl::NONE,
                ],
            )
            .expect("OpenGL consistency pbuffer must be created");
        owner.surface = Some(surface);
        let context = owner
            .api
            .create_context(
                display,
                config,
                None,
                &[
                    egl::CONTEXT_MAJOR_VERSION,
                    3,
                    egl::CONTEXT_MINOR_VERSION,
                    0,
                    egl::NONE,
                ],
            )
            .expect("OpenGL consistency requires a GLES 3.0 context");
        owner.context = Some(context);
        owner
            .api
            .make_current(display, Some(surface), Some(surface), Some(context))
            .expect("OpenGL consistency context must become current");
        owner
    }

    // 从当前 EGL context 建立 glow 分派表，不取得第二份原生 owner。
    fn load_gl(&self) -> glow::Context {
        unsafe {
            // SAFETY: 当前线程已绑定本 owner 的 GLES3 context，分派表只在 owner 存活期间使用。
            glow::Context::from_loader_function(|name| {
                self.api
                    .get_proc_address(name)
                    .map(|function| function as *const std::ffi::c_void)
                    .unwrap_or(std::ptr::null())
            })
        }
    }

    // 重新绑定当前 owner 的 context 与 pbuffer。
    fn make_current(&self) -> Result<(), egl::Error> {
        self.api
            .make_current(self.display, self.surface, self.surface, self.context)
    }

    // 按共享事务已验证的尺寸替换真实 EGL pbuffer。
    fn recreate_pbuffer(&mut self, width: i32, height: i32) -> Result<(), egl::Error> {
        self.api.make_current(self.display, None, None, None)?;
        if let Some(surface) = self.surface.take() {
            self.api.destroy_surface(self.display, surface)?;
        }
        let surface = self.api.create_pbuffer_surface(
            self.display,
            self.config.expect("headless EGL config must remain alive"),
            &[egl::WIDTH, width, egl::HEIGHT, height, egl::NONE],
        )?;
        self.surface = Some(surface);
        self.make_current()
    }

    // 在 RHI 资源释放后按 current→context→surface→display 逆序关闭测试 owner。
    fn shutdown(&mut self) {
        if !self.active {
            return;
        }
        self.api
            .make_current(self.display, None, None, None)
            .expect("OpenGL consistency context must detach");
        if let Some(context) = self.context.take() {
            self.api
                .destroy_context(self.display, context)
                .expect("OpenGL consistency context must be destroyed");
        }
        if let Some(surface) = self.surface.take() {
            self.api
                .destroy_surface(self.display, surface)
                .expect("OpenGL consistency pbuffer must be destroyed");
        }
        self.api
            .terminate(self.display)
            .expect("OpenGL consistency display must terminate");
        self.active = false;
    }
}

impl Drop for HeadlessEgl {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = self.api.make_current(self.display, None, None, None);
        if let Some(context) = self.context.take() {
            let _ = self.api.destroy_context(self.display, context);
        }
        if let Some(surface) = self.surface.take() {
            let _ = self.api.destroy_surface(self.display, surface);
        }
        let _ = self.api.terminate(self.display);
        self.active = false;
    }
}

// 真实 EGL pbuffer 对生产 OpenGL Surface blanket 实现的最小原生 host。
struct HeadlessOpenGlSurfaceHost<'a> {
    egl: &'a mut HeadlessEgl,
    pipeline: OpenGlRasterPipeline,
    surface_lifecycle: RhiSurfaceLifecycle,
    fail_current_once: bool,
    fail_swap_once: bool,
    recreate_count: u64,
}

impl<'a> HeadlessOpenGlSurfaceHost<'a> {
    // 在已经 current 的真实 EGL context 上建立生产 raster pipeline。
    fn new(egl: &'a mut HeadlessEgl, extent: RhiExtent) -> Self {
        let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(extent);
        let initialize = surface_lifecycle
            .begin_recreate(extent, RhiSurfaceRecreateReason::Initialize)
            .expect("headless surface initialize transaction must begin");
        let runtime = NativeOpenGlRuntime::from_loader(|name| {
            egl.api
                .get_proc_address(name)
                .map(|function| function as *const std::ffi::c_void)
                .unwrap_or(std::ptr::null())
        });
        let pipeline = OpenGlRasterPipeline::new(
            runtime,
            extent.width as i32,
            extent.height as i32,
            extent.width as i32,
            extent.height as i32,
        )
        .expect("headless production OpenGL pipeline must initialize");
        surface_lifecycle
            .commit_recreate(initialize, extent)
            .expect("headless surface initialize transaction must commit");
        Self {
            egl,
            pipeline,
            surface_lifecycle,
            fail_current_once: false,
            fail_swap_once: false,
            recreate_count: 0,
        }
    }

    // 在 EGL teardown 前检查式释放生产 pipeline 资源。
    fn release(&mut self) {
        self.pipeline.release();
    }
}

impl OpenGlRhiHost for HeadlessOpenGlSurfaceHost<'_> {
    fn rhi_ensure_active(&self) -> crate::core::Result<()> {
        if self.egl.active {
            Ok(())
        } else {
            Err(Error::new(
                Errc::InvalidState,
                "headless EGL owner is inactive",
            ))
        }
    }

    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline {
        &mut self.pipeline
    }

    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline {
        &self.pipeline
    }

    fn rhi_make_current(&mut self) -> crate::core::Result<()> {
        if std::mem::take(&mut self.fail_current_once) {
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "headless EGL acquire boundary injected surface loss",
            ));
        }
        self.egl.make_current().map_err(|error| {
            Error::new(
                Errc::GraphicsSurfaceLost,
                format!("headless EGL make current failed: {error:?}"),
            )
        })
    }

    fn rhi_surface_lifecycle(&self) -> &RhiSurfaceLifecycle {
        &self.surface_lifecycle
    }

    fn rhi_surface_lifecycle_mut(&mut self) -> &mut RhiSurfaceLifecycle {
        &mut self.surface_lifecycle
    }

    fn rhi_surface_matches(
        &self,
        resize: RhiSurfaceResizeTransaction,
    ) -> crate::core::Result<bool> {
        Ok(self.pipeline.rhi_surface_extent() == resize.extent())
    }

    fn rhi_recreate_surface(
        &mut self,
        recreate: RhiSurfaceRecreateTransaction,
    ) -> crate::core::Result<RhiExtent> {
        let (width, height) = recreate.native_size_i32();
        self.egl.recreate_pbuffer(width, height).map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("headless EGL pbuffer recreate failed: {error:?}"),
            )
        })?;
        self.pipeline.resize_swapchain(width, height, width, height);
        self.recreate_count = self.recreate_count.saturating_add(1);
        Ok(recreate.requested())
    }

    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        if std::mem::take(&mut self.fail_swap_once) {
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "headless EGL swap boundary injected surface loss",
            ));
        }
        self.egl
            .api
            .swap_buffers(
                self.egl.display,
                self.egl
                    .surface
                    .expect("headless EGL surface must remain alive"),
            )
            .map_err(|error| {
                Error::new(
                    Errc::GraphicsSurfaceLost,
                    format!("headless EGL swap failed: {error:?}"),
                )
            })
    }
}

// 执行一次真实 surface acquire、clear、submit 与 EGL swap 边界。
fn present_headless_surface(host: &mut HeadlessOpenGlSurfaceHost<'_>) {
    let frame = GraphicsSurface::acquire(host).expect("headless surface acquire must succeed");
    GraphicsDevice::begin_render_pass(
        host,
        frame.target(),
        LoadAction::Clear(RhiColor::from_straight_rgba([0.125, 0.25, 0.5, 1.0])),
    )
    .expect("headless surface pass must begin");
    GraphicsDevice::end_render_pass(host).expect("headless surface pass must end");
    let submission =
        GraphicsDevice::submit(host).expect("headless surface submit must reach native GL");
    GraphicsSurface::present(
        host,
        RhiPresentTransaction::new(frame, submission, PresentDamage::Full),
    )
    .expect("headless surface present must reach eglSwapBuffers");
}

// 在真实 Mesa EGL pbuffer 上验证生产 OpenGL Surface 生命周期。
fn run_surface_lifecycle_test(egl: &mut HeadlessEgl) {
    let initial_extent = CONSISTENCY_EXTENT;
    let resized_extent = RhiExtent::new(initial_extent.width + 16, initial_extent.height + 8);
    let mut host = HeadlessOpenGlSurfaceHost::new(egl, initial_extent);
    let initial_token = GraphicsSurface::token(&host);

    // 正常路径必须经过真实 acquire/submit/eglSwapBuffers。
    present_headless_surface(&mut host);

    // resize 由共享事务推进一次 generation，并使旧 acquired frame 失效。
    let stale_frame = GraphicsSurface::acquire(&mut host).expect("stale frame must acquire");
    let resized = GraphicsSurface::resize(&mut host, resized_extent)
        .expect("headless EGL resize transaction must succeed");
    assert_eq!(resized.generation, initial_token.generation + 1);
    assert_eq!(resized.extent, resized_extent);
    assert_eq!(host.recreate_count, 1);
    let submission = GraphicsDevice::submit(&mut host)
        .expect("stale-token validation needs a latest submission");
    let stale_error = GraphicsSurface::present(
        &mut host,
        RhiPresentTransaction::new(stale_frame, submission, PresentDamage::Full),
    )
    .expect_err("pre-resize frame token must be rejected");
    assert_eq!(stale_error.code(), Errc::GraphicsSurfaceLost);

    // 零尺寸在共享生命周期前置门禁拒绝，且不得触碰原生 pbuffer。
    let recreate_before_invalid = host.recreate_count;
    let zero_error = GraphicsSurface::resize(&mut host, RhiExtent::new(0, resized_extent.height))
        .expect_err("zero surface extent must be rejected");
    assert_eq!(zero_error.code(), Errc::InvalidArgument);
    assert_eq!(host.recreate_count, recreate_before_invalid);

    // acquire SurfaceLost 只触发一次共享 generation 重建。
    let before_acquire_recovery = GraphicsSurface::token(&host);
    host.fail_current_once = true;
    let acquire_error = GraphicsSurface::acquire(&mut host)
        .expect_err("injected acquire surface loss must request a retry");
    assert_eq!(acquire_error.code(), Errc::GraphicsSurfaceChanged);
    assert_eq!(
        GraphicsSurface::token(&host).generation,
        before_acquire_recovery.generation + 1
    );
    assert_eq!(host.recreate_count, recreate_before_invalid + 1);

    // present SurfaceLost 同样只建立一个新代际，随后真实交换恢复成功。
    let frame = GraphicsSurface::acquire(&mut host).expect("recovered surface must acquire");
    GraphicsDevice::begin_render_pass(
        &mut host,
        frame.target(),
        LoadAction::Clear(RhiColor::transparent()),
    )
    .expect("recovered surface pass must begin");
    GraphicsDevice::end_render_pass(&mut host).expect("recovered surface pass must end");
    let submission = GraphicsDevice::submit(&mut host).expect("recovered submit must succeed");
    let before_present_recovery = GraphicsSurface::token(&host);
    host.fail_swap_once = true;
    let present_error = GraphicsSurface::present(
        &mut host,
        RhiPresentTransaction::new(frame, submission, PresentDamage::Full),
    )
    .expect_err("injected present surface loss must request a retry");
    assert_eq!(present_error.code(), Errc::GraphicsSurfaceChanged);
    assert_eq!(
        GraphicsSurface::token(&host).generation,
        before_present_recovery.generation + 1
    );
    assert_eq!(host.recreate_count, recreate_before_invalid + 2);
    present_headless_surface(&mut host);

    eprintln!(
        "OpenGL surface lifecycle verified: real acquire/submit/EGL swap, resize, stale token, zero extent, acquire+present recovery"
    );
    host.release();
}

// 保存一个共享场景机械映射后的 OpenGL RHI 身份，不拥有像素语义。
struct NativeSceneResources {
    pipeline: PipelineBinding,
    vertex: BufferHandle,
    uniform: BufferHandle,
    sampled: Option<(TextureHandle, SamplerHandle)>,
}

// 把一个 API 无关场景机械创建为 OpenGL RHI 资源。
fn create_scene_resources(
    rhi: &mut OpenGlRhiDevice,
    gl: &glow::Context,
    scene: &ConsistencyScene,
) -> NativeSceneResources {
    let vertex_bytes = scene.vertex.encode_ne_bytes();
    let uniform_bytes = scene.uniform.encode_ne_bytes();
    let contract = scene.kind.contract();
    let vertex = rhi
        .create_buffer(
            gl,
            BufferDesc::vertex(vertex_bytes.len(), contract.vertex.stride_bytes()),
        )
        .expect("OpenGL consistency vertex buffer must be created");
    let uniform = rhi
        .create_buffer(gl, BufferDesc::uniform(uniform_bytes.len()))
        .expect("OpenGL consistency uniform buffer must be created");
    rhi.update_buffer(gl, RhiBufferUpload::new(vertex, &vertex_bytes))
        .expect("OpenGL consistency vertices must upload");
    rhi.update_buffer(gl, RhiBufferUpload::new(uniform, &uniform_bytes))
        .expect("OpenGL consistency uniforms must upload");
    let pipeline = rhi
        .create_pipeline(gl, PipelineDesc { kind: scene.kind })
        .expect("OpenGL consistency pipeline must be created");
    let sampled = scene.texture.as_ref().map(|source| {
        let texture = rhi
            .create_texture(gl, TextureDesc::new(source.extent, source.format))
            .expect("OpenGL consistency sampled texture must be created");
        rhi.update_texture(
            gl,
            RhiTextureUpload::full(texture, source.extent, &source.bytes),
        )
        .expect("OpenGL consistency sampled texture must upload");
        let sampler = rhi
            .create_sampler(gl, source.sampler)
            .expect("OpenGL consistency sampler must be created");
        (texture, sampler)
    });
    NativeSceneResources {
        pipeline,
        vertex,
        uniform,
        sampled,
    }
}

// 编码一个真实 draw；PipelineKind、ABI、采样与栅格事实只来自共享场景。
fn draw_scene(
    rhi: &mut OpenGlRhiDevice,
    gl: &glow::Context,
    scene: &ConsistencyScene,
    native: &NativeSceneResources,
) {
    let sampling = native
        .sampled
        .map_or_else(DrawSamplingBinding::none, |(texture, sampler)| {
            DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                texture,
                sampler,
                native.pipeline,
            ))
        });
    let packet = DrawPacket::new(
        native.pipeline,
        DrawBufferBindings::new(native.vertex, native.uniform),
        sampling,
        DrawRasterState::new(
            RhiViewport {
                width: CONSISTENCY_EXTENT.width as f32,
                height: CONSISTENCY_EXTENT.height as f32,
            },
            scene.scissor,
        ),
        DrawRange::vertices(
            scene
                .vertex
                .vertex_count()
                .expect("canonical scene vertex count must be complete"),
        ),
    );
    rhi.draw(gl, packet)
        .unwrap_or_else(|error| panic!("{} OpenGL draw failed: {error}", scene.name));
}

// 从颜色 texture 的真实 FBO 回读共享画布；texture target 的第零行就是逻辑顶部。
fn read_target(
    rhi: &OpenGlRhiDevice,
    gl: &glow::Context,
    texture: TextureHandle,
    extent: RhiExtent,
) -> Vec<u8> {
    let framebuffer = rhi
        .texture(texture)
        .expect("OpenGL consistency target must remain alive")
        .framebuffer
        .expect("OpenGL consistency color target must own a framebuffer");
    let byte_count = extent.width as usize
        * extent.height as usize
        * TextureFormat::Rgba8Unorm.bytes_per_pixel();
    let mut pixels = vec![0u8; byte_count];
    unsafe {
        // SAFETY: framebuffer 属于当前 context 且保持存活；载荷与共享 RGBA8 extent 精确匹配。
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(framebuffer));
        gl.read_buffer(glow::COLOR_ATTACHMENT0);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        gl.read_pixels(
            0,
            0,
            extent.width as i32,
            extent.height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        assert_eq!(
            gl.get_error(),
            glow::NO_ERROR,
            "OpenGL consistency RGBA readback must succeed"
        );
    }
    pixels
}

// 用共享采样点与共享 accepts 判定一份真实 RGBA 回读。
fn validate_samples(name: &str, samples: &[ConsistencySample], pixels: &[u8]) {
    let row_bytes = CONSISTENCY_EXTENT.width as usize * TextureFormat::Rgba8Unorm.bytes_per_pixel();
    for sample in samples {
        let offset = sample.y as usize * row_bytes + sample.x as usize * 4;
        let actual: [u8; 4] = pixels[offset..offset + 4]
            .try_into()
            .expect("canonical sample must address one RGBA pixel");
        assert!(
            sample.accepts(actual),
            "{name} {} at ({}, {}): actual {actual:?}, expected {:?}..={:?}, tolerance {:?}({})",
            sample.semantic,
            sample.x,
            sample.y,
            sample.minimum,
            sample.maximum,
            sample.tolerance,
            sample.tolerance.amount(),
        );
    }
}

// 把共享 Blur pass 输入机械编码成 OpenGL draw packet。
fn draw_blur_pass(
    rhi: &mut OpenGlRhiDevice,
    gl: &glow::Context,
    pipeline: PipelineBinding,
    vertex: BufferHandle,
    uniform: BufferHandle,
    texture: TextureHandle,
    sampler: SamplerHandle,
    scenario: &ConsistencyBlurScenario,
) {
    let packet = DrawPacket::new(
        pipeline,
        DrawBufferBindings::new(vertex, uniform),
        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
            texture, sampler, pipeline,
        )),
        DrawRasterState::new(
            RhiViewport {
                width: CONSISTENCY_EXTENT.width as f32,
                height: CONSISTENCY_EXTENT.height as f32,
            },
            Some(scenario.scissor),
        ),
        DrawRange::vertices(
            scenario
                .vertex
                .vertex_count()
                .expect("canonical blur vertex count must be complete"),
        ),
    );
    rhi.draw(gl, packet)
        .expect("OpenGL blur subregion draw must execute");
}

// 在同一真实 context 中执行 source→scratch→target 的水平与垂直 Blur。
fn run_blur_subregion_two_pass(rhi: &mut OpenGlRhiDevice, gl: &glow::Context) -> usize {
    let scenario = blur_subregion_scenario();
    let contract = PipelineKind::BlurPass.contract();
    let vertex_bytes = scenario.vertex.encode_ne_bytes();
    let horizontal_bytes = scenario.horizontal.encode_ne_bytes();
    let vertical_bytes = scenario.vertical.encode_ne_bytes();
    let vertex = rhi
        .create_buffer(
            gl,
            BufferDesc::vertex(vertex_bytes.len(), contract.vertex.stride_bytes()),
        )
        .expect("OpenGL blur vertex buffer must be created");
    let horizontal = rhi
        .create_buffer(gl, BufferDesc::uniform(horizontal_bytes.len()))
        .expect("OpenGL blur horizontal uniform must be created");
    let vertical = rhi
        .create_buffer(gl, BufferDesc::uniform(vertical_bytes.len()))
        .expect("OpenGL blur vertical uniform must be created");
    rhi.update_buffer(gl, RhiBufferUpload::new(vertex, &vertex_bytes))
        .expect("OpenGL blur vertices must upload");
    rhi.update_buffer(gl, RhiBufferUpload::new(horizontal, &horizontal_bytes))
        .expect("OpenGL blur horizontal uniform must upload");
    rhi.update_buffer(gl, RhiBufferUpload::new(vertical, &vertical_bytes))
        .expect("OpenGL blur vertical uniform must upload");

    let source = rhi
        .create_texture(
            gl,
            TextureDesc::new(scenario.texture.extent, scenario.texture.format),
        )
        .expect("OpenGL blur source texture must be created");
    rhi.update_texture(
        gl,
        RhiTextureUpload::full(source, scenario.texture.extent, &scenario.texture.bytes),
    )
    .expect("OpenGL blur source texture must upload");
    let scratch = rhi
        .create_texture(
            gl,
            TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
        )
        .expect("OpenGL blur scratch texture must be created");
    let target = rhi
        .create_texture(
            gl,
            TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
        )
        .expect("OpenGL blur target texture must be created");
    let sampler = rhi
        .create_sampler(gl, scenario.texture.sampler)
        .expect("OpenGL blur sampler must be created");
    let pipeline = rhi
        .create_pipeline(
            gl,
            PipelineDesc {
                kind: PipelineKind::BlurPass,
            },
        )
        .expect("OpenGL blur pipeline must be created");

    let scratch_target = rhi
        .resolve_render_target(scratch)
        .expect("OpenGL blur scratch target identity must resolve");
    rhi.begin_render_pass(
        gl,
        scratch_target,
        LoadAction::Clear(RhiColor::transparent()),
        CONSISTENCY_EXTENT,
    )
    .expect("OpenGL blur horizontal pass must begin");
    draw_blur_pass(
        rhi, gl, pipeline, vertex, horizontal, source, sampler, &scenario,
    );
    rhi.end_render_pass(gl)
        .expect("OpenGL blur horizontal pass must end");

    let target_identity = rhi
        .resolve_render_target(target)
        .expect("OpenGL blur target identity must resolve");
    let clear =
        RhiColor::from_straight_rgba(CONSISTENCY_BACKGROUND.map(|value| value as f32 / 255.0));
    rhi.begin_render_pass(
        gl,
        target_identity,
        LoadAction::Clear(clear),
        CONSISTENCY_EXTENT,
    )
    .expect("OpenGL blur vertical pass must begin");
    draw_blur_pass(
        rhi, gl, pipeline, vertex, vertical, scratch, sampler, &scenario,
    );
    rhi.end_render_pass(gl)
        .expect("OpenGL blur vertical pass must end");
    rhi.submit(gl)
        .expect("OpenGL blur two-pass commands must submit");
    unsafe {
        // SAFETY: current context 存活；finish 只等待本 context 已提交命令完成。
        gl.finish();
    }
    let pixels = read_target(rhi, gl, target, CONSISTENCY_EXTENT);
    validate_samples("BlurPassTwoPass", &scenario.final_samples, &pixels);
    eprintln!(
        "OpenGL blur subregion verified: origin=({}, {}), extent={}x{}, {} invariants",
        scenario.scissor.x,
        scenario.scissor.y,
        scenario.scissor.width,
        scenario.scissor.height,
        scenario.final_samples.len(),
    );
    scenario.final_samples.len()
}

pub(super) fn run_gpu_parity_test() {
    let mut egl = HeadlessEgl::new();
    let gl = egl.load_gl();
    let vendor = unsafe { gl.get_parameter_string(glow::VENDOR) };
    let renderer = unsafe { gl.get_parameter_string(glow::RENDERER) };
    let version = unsafe { gl.get_parameter_string(glow::VERSION) };
    eprintln!(
        "OpenGL consistency device: vendor={vendor}; renderer={renderer}; version={version}; EGL={}.{}; platform={}",
        egl.version.0, egl.version.1, egl.platform,
    );

    let scenes = canonical_scenes();
    validate_canonical_scenes(&scenes).expect("shared consistency architecture gate must pass");
    let mut rhi = OpenGlRhiDevice::new(&gl).expect("OpenGL consistency RHI must initialize");
    let target = rhi
        .create_texture(
            &gl,
            TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
        )
        .expect("OpenGL consistency target must be created");
    let resources = scenes
        .iter()
        .map(|scene| create_scene_resources(&mut rhi, &gl, scene))
        .collect::<Vec<_>>();

    let target_identity = rhi
        .resolve_render_target(target)
        .expect("OpenGL consistency target identity must resolve");
    let clear =
        RhiColor::from_straight_rgba(CONSISTENCY_BACKGROUND.map(|value| value as f32 / 255.0));
    rhi.begin_render_pass(
        &gl,
        target_identity,
        LoadAction::Clear(clear),
        CONSISTENCY_EXTENT,
    )
    .expect("OpenGL consistency render pass must begin");
    // Replace blur 先建立自己的隔离区域，其余十类再覆盖同一共享画布。
    let draw_order = scenes
        .iter()
        .enumerate()
        .filter(|(_, scene)| scene.kind == PipelineKind::BlurPass)
        .chain(
            scenes
                .iter()
                .enumerate()
                .filter(|(_, scene)| scene.kind != PipelineKind::BlurPass),
        );
    for (index, scene) in draw_order {
        draw_scene(&mut rhi, &gl, scene, &resources[index]);
    }
    rhi.end_render_pass(&gl)
        .expect("OpenGL consistency render pass must end");
    rhi.submit(&gl)
        .expect("OpenGL consistency draws must submit");
    unsafe {
        // SAFETY: current context 存活；finish 形成 submit 后真实回读的完成边界。
        gl.finish();
    }
    let pixels = read_target(&rhi, &gl, target, CONSISTENCY_EXTENT);
    for scene in &scenes {
        validate_samples(scene.name, &scene.samples, &pixels);
    }
    let scene_invariants = scenes
        .iter()
        .map(|scene| scene.samples.len())
        .sum::<usize>();
    eprintln!(
        "OpenGL consistency verified: {} scenes, {scene_invariants} direct invariants",
        scenes.len(),
    );

    let blur_invariants = run_blur_subregion_two_pass(&mut rhi, &gl);
    eprintln!(
        "OpenGL consistency complete: {scene_invariants} shared scene invariants; Blur two-pass {blur_invariants} final invariants",
    );
    rhi.release(&gl);
    drop(gl);
    // 复用真实 PaintContext/Canvas2D 与共享 FramePlan 验收 GLES Device/readback。
    run_ui_production_chain_test(&mut egl);
    // 复用同一个真实 Mesa EGL owner 验证生产 Surface blanket 生命周期。
    run_surface_lifecycle_test(&mut egl);
    egl.shutdown();
}

// 在真实 EGL/GLES owner 上闭合 UI → Drawing → FramePlan → OpenGL Device 回读。
fn run_ui_production_chain_test(egl: &mut HeadlessEgl) {
    let scene = production_chain_scene();
    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let mut host = HeadlessOpenGlSurfaceHost::new(egl, scene.extent);
    let target = execute_ui_production_chain(&mut host, scene.extent, |draw_context| {
        crate::ui::view::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
    .expect("OpenGL UI production scene must execute through the shared Drawing FramePlan");
    let gl = host.pipeline.runtime.context();
    unsafe {
        // SAFETY: host 唯一拥有的 EGL/GLES context 当前且存活；finish 只等待已提交命令。
        gl.finish();
    }
    let pixels = read_target(&host.pipeline.rhi, gl, target, scene.extent);
    let invariant_count = validate_production_chain_readback(&scene, &pixels)
        .expect("OpenGL production-chain readback must satisfy shared Drawing invariants");
    GraphicsDevice::destroy_texture(&mut host, target)
        .expect("OpenGL production-chain target must be released");
    host.release();
    eprintln!(
        "OpenGL production chain verified: path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan -> OpenGL ES GraphicsDevice; draw-readback={invariant_count}/{}",
        scene.samples.len(),
    );
}

#[cfg(test)]
#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_opengl_es_device() {
    run_gpu_parity_test();
}
