//! D3D11 RHI 对 API 无关全图元规范场景的真实离屏执行与 staging 回读 harness。

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
};
use windows::core::w;

use super::*;
use crate::core::{Errc, PresentDamage};
use crate::draw::backend::production_chain_parity::{
    execute_ui_production_chain, execute_ui_production_surface_chain,
};
use crate::draw::backend::rhi_renderer::consistency::{
    CONSISTENCY_BACKGROUND, CONSISTENCY_EXTENT, ConsistencyBlurScenario, ConsistencySample,
    ConsistencyScene, blur_subregion_scenario, canonical_scenes, production_chain_scene,
    validate_canonical_scenes, validate_production_chain_readback,
};
use crate::platform::presentation::rhi::{
    DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState, DrawSamplingBinding,
    GraphicsDevice, GraphicsSurface, IndexBufferBinding, IndexFormat, PipelineKind,
    RhiBufferUpload, RhiExtent, RhiPresentTransaction, RhiTextureUpload, RhiViewport,
    SampledTextureBinding,
};

// 隐藏窗口只为生产 D3D11 context 的真实 swapchain 构造提供 HWND，不进入上层绘制。
struct HiddenWindow {
    hwnd: Option<HWND>,
}

impl HiddenWindow {
    fn new() -> Self {
        let hwnd = unsafe {
            // SAFETY: STATIC 是系统预注册窗口类；全部参数仅在同步创建期间读取，窗口保持隐藏。
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("UIX D3D11 parity"),
                WS_POPUP,
                0,
                0,
                CONSISTENCY_EXTENT.width as i32,
                CONSISTENCY_EXTENT.height as i32,
                None,
                None,
                None,
                None,
            )
        }
        .expect("D3D11 consistency hidden window must be created");
        Self { hwnd: Some(hwnd) }
    }

    fn raw(&self) -> *mut c_void {
        self.hwnd
            .as_ref()
            .expect("D3D11 consistency hidden window must remain alive")
            .0
    }

    fn shutdown(&mut self) {
        let Some(hwnd) = self.hwnd.take() else {
            return;
        };
        unsafe {
            // SAFETY: hwnd 由本 owner 在当前线程创建且只销毁一次，D3D11 context 已先释放。
            DestroyWindow(hwnd)
        }
        .expect("D3D11 consistency hidden window must be destroyed");
    }
}

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            unsafe {
                // SAFETY: 失败展开时仍只尝试销毁本 owner 唯一持有的隐藏窗口一次。
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

// 在真实 HWND swapchain 上执行 acquire、submit 与 Present/Present1。
fn present_swapchain_surface(rhi: &mut D3d11Context) {
    let frame = GraphicsSurface::acquire(rhi).expect("D3D11 surface acquire must succeed");
    GraphicsDevice::begin_render_pass(
        rhi,
        frame.target(),
        LoadAction::Clear(RhiColor::from_straight_rgba([0.125, 0.25, 0.5, 1.0])),
    )
    .expect("D3D11 surface render pass must begin");
    GraphicsDevice::end_render_pass(rhi).expect("D3D11 surface render pass must end");
    let submission =
        GraphicsDevice::submit(rhi).expect("D3D11 surface submit must reach the device context");
    let present = GraphicsSurface::present(
        rhi,
        RhiPresentTransaction::new(frame, submission, PresentDamage::Full),
    );
    // 隐藏 HWND 可由 DWM 明确报告 Occluded；该 typed 结果仍证明已到达原生 Present。
    if let Err(error) = present {
        assert_eq!(
            error.code(),
            Errc::GraphicsOccluded,
            "D3D11 production swapchain present failed before an allowed hidden-window occlusion"
        );
    }
}

// 在同一隐藏窗口与生产 context 上验证共享 D3D11 Surface 生命周期。
fn run_surface_lifecycle_test(rhi: &mut D3d11Context) {
    let initial_token = GraphicsSurface::token(rhi);
    let resized_extent = RhiExtent::new(
        initial_token.extent.width + 16,
        initial_token.extent.height + 8,
    );

    // 初始化后的正常路径必须真实经过 acquire、submit 与 Present。
    present_swapchain_surface(rhi);

    // ResizeBuffers 必须由共享事务推进一次 generation，并使旧 frame 失效。
    let stale_frame = GraphicsSurface::acquire(rhi).expect("D3D11 stale frame must acquire");
    let resized = GraphicsSurface::resize(rhi, resized_extent)
        .expect("D3D11 ResizeBuffers transaction must succeed");
    assert_eq!(resized.generation, initial_token.generation + 1);
    assert_eq!(resized.extent, resized_extent);
    let submission = GraphicsDevice::submit(rhi)
        .expect("D3D11 stale-token validation needs a latest submission");
    let stale_error = GraphicsSurface::present(
        rhi,
        RhiPresentTransaction::new(stale_frame, submission, PresentDamage::Full),
    )
    .expect_err("D3D11 pre-resize frame token must be rejected");
    assert_eq!(stale_error.code(), Errc::GraphicsSurfaceLost);

    // 零尺寸与超出 Win32 i32 的尺寸必须在共享门禁拒绝且不推进 generation。
    let before_invalid = GraphicsSurface::token(rhi);
    for invalid in [
        RhiExtent::new(0, resized_extent.height),
        RhiExtent::new(i32::MAX as u32 + 1, resized_extent.height),
    ] {
        let invalid_error = GraphicsSurface::resize(rhi, invalid)
            .expect_err("D3D11 invalid surface extent must be rejected");
        assert_eq!(invalid_error.code(), Errc::InvalidArgument);
        assert_eq!(GraphicsSurface::token(rhi), before_invalid);
    }

    // acquire SurfaceLost 只能触发一次同尺寸共享重建，随后真实帧恢复成功。
    GraphicsSurface::inject_surface_lost_for_test(rhi)
        .expect("D3D11 acquire surface loss must be scheduled");
    let acquire_error =
        GraphicsSurface::acquire(rhi).expect_err("D3D11 acquire surface loss must request a retry");
    assert_eq!(acquire_error.code(), Errc::GraphicsSurfaceChanged);
    assert_eq!(
        GraphicsSurface::token(rhi).generation,
        before_invalid.generation + 1
    );
    present_swapchain_surface(rhi);

    // present SurfaceLost 同样只推进一次 generation，DeviceLost 分类不参与此路径。
    let frame = GraphicsSurface::acquire(rhi).expect("D3D11 recovered frame must acquire");
    GraphicsDevice::begin_render_pass(
        rhi,
        frame.target(),
        LoadAction::Clear(RhiColor::transparent()),
    )
    .expect("D3D11 recovered surface pass must begin");
    GraphicsDevice::end_render_pass(rhi).expect("D3D11 recovered surface pass must end");
    let submission = GraphicsDevice::submit(rhi).expect("D3D11 recovered submit must succeed");
    let before_present_recovery = GraphicsSurface::token(rhi);
    GraphicsSurface::inject_surface_lost_for_test(rhi)
        .expect("D3D11 present surface loss must be scheduled");
    let present_error = GraphicsSurface::present(
        rhi,
        RhiPresentTransaction::new(frame, submission, PresentDamage::Full),
    )
    .expect_err("D3D11 present surface loss must request a retry");
    assert_eq!(present_error.code(), Errc::GraphicsSurfaceChanged);
    assert_eq!(
        GraphicsSurface::token(rhi).generation,
        before_present_recovery.generation + 1
    );
    present_swapchain_surface(rhi);

    eprintln!(
        "D3D11 surface lifecycle verified: initialization, real acquire/submit/present, ResizeBuffers, stale token, zero/invalid extent, acquire+present one-shot recovery"
    );
}

// 在真实 D3D11 Device 上闭合 UI → Drawing → 共享 FramePlan 与 staging 回读。
fn run_ui_production_chain_test(rhi: &mut D3d11Context) {
    let scene = production_chain_scene();
    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let target = execute_ui_production_chain(rhi, scene.extent, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
    .expect("D3D11 UI production scene must execute through the shared Drawing FramePlan");
    let pixels = read_target(rhi, target);
    let invariant_count = validate_production_chain_readback(&scene, &pixels)
        .expect("D3D11 production-chain readback must satisfy shared Drawing invariants");
    GraphicsDevice::destroy_texture(rhi, target)
        .expect("D3D11 production-chain target must be released");
    eprintln!(
        "D3D11 production chain verified: path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan -> D3D11 GraphicsDevice; draw-readback={invariant_count}/{}",
        scene.samples.len(),
    );
}

// 在同一真实 DXGI Surface 上复用 API 中立生产桥；隐藏窗口允许明确的 occluded 结果。
fn run_ui_production_surface_chain_test(rhi: &mut D3d11Context) {
    let scene = production_chain_scene();
    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let before = GraphicsSurface::token(rhi);
    let status = match execute_ui_production_surface_chain(rhi, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    }) {
        Ok(presented) => {
            assert_eq!(presented, before);
            "presented"
        }
        Err(error) if error.code() == Errc::GraphicsOccluded => {
            assert_eq!(GraphicsSurface::token(rhi), before);
            "occluded-token-unchanged"
        }
        Err(error) => panic!("D3D11 UI production Surface chain failed: {error}"),
    };
    eprintln!(
        "D3D11 Surface production chain verified: shared UI/Drawing FramePlan/RHI bridge; generation={}; status={status}",
        before.generation,
    );
}

// 保存共享场景机械映射后的 D3D11 RHI 身份，不拥有期望像素或容差。
struct NativeSceneResources {
    pipeline: PipelineBinding,
    vertex: BufferHandle,
    uniform: BufferHandle,
    index: Option<BufferHandle>,
    sampled: Option<(TextureHandle, SamplerHandle)>,
}

// 把一个 API 无关场景机械创建为生产 D3D11 RHI 资源。
fn create_scene_resources(
    rhi: &mut D3d11Context,
    scene: &ConsistencyScene,
    indexed: bool,
) -> NativeSceneResources {
    let vertex_bytes = scene.vertex.encode_ne_bytes();
    let uniform_bytes = scene.uniform.encode_ne_bytes();
    let contract = scene.kind.contract();
    let vertex_count = scene
        .vertex
        .vertex_count()
        .expect("canonical D3D11 scene vertex count must be complete");
    let vertex = GraphicsDevice::create_buffer(
        rhi,
        BufferDesc::vertex(vertex_bytes.len(), contract.vertex.stride_bytes()),
    )
    .expect("D3D11 consistency vertex buffer must be created");
    let uniform = GraphicsDevice::create_buffer(rhi, BufferDesc::uniform(uniform_bytes.len()))
        .expect("D3D11 consistency uniform buffer must be created");
    GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(vertex, &vertex_bytes))
        .expect("D3D11 consistency vertices must upload");
    GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(uniform, &uniform_bytes))
        .expect("D3D11 consistency uniforms must upload");
    let index = indexed.then(|| {
        let index_bytes = (0..vertex_count)
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>();
        let buffer = GraphicsDevice::create_buffer(
            rhi,
            BufferDesc::index(index_bytes.len(), IndexFormat::Uint32.stride_bytes()),
        )
        .expect("D3D11 consistency index buffer must be created");
        GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(buffer, &index_bytes))
            .expect("D3D11 consistency indices must upload");
        buffer
    });
    let pipeline = GraphicsDevice::create_pipeline(rhi, PipelineDesc { kind: scene.kind })
        .expect("D3D11 consistency pipeline must be created");
    let sampled = scene.texture.as_ref().map(|source| {
        let texture =
            GraphicsDevice::create_texture(rhi, TextureDesc::new(source.extent, source.format))
                .expect("D3D11 consistency sampled texture must be created");
        GraphicsDevice::update_texture(
            rhi,
            RhiTextureUpload::full(texture, source.extent, &source.bytes),
        )
        .expect("D3D11 consistency sampled texture must upload");
        let sampler = GraphicsDevice::create_sampler(rhi, source.sampler)
            .expect("D3D11 consistency sampler must be created");
        (texture, sampler)
    });
    NativeSceneResources {
        pipeline,
        vertex,
        uniform,
        index,
        sampled,
    }
}

// 编码一个真实 Draw 或 DrawIndexed；场景、ABI、采样与栅格事实只来自共享规范。
fn draw_scene(rhi: &mut D3d11Context, scene: &ConsistencyScene, native: &NativeSceneResources) {
    let sampling = native
        .sampled
        .map_or_else(DrawSamplingBinding::none, |(texture, sampler)| {
            DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                texture,
                sampler,
                native.pipeline,
            ))
        });
    let vertex_count = scene
        .vertex
        .vertex_count()
        .expect("canonical D3D11 scene vertex count must be complete");
    let range = native.index.map_or_else(
        || DrawRange::vertices(vertex_count),
        |index| {
            DrawRange::indices(
                IndexBufferBinding::new(index, IndexFormat::Uint32),
                vertex_count,
                0,
            )
        },
    );
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
        range,
    );
    GraphicsDevice::draw(rhi, packet)
        .unwrap_or_else(|error| panic!("{} D3D11 draw failed: {error}", scene.name));
}

// 从生产 RHI 颜色 texture 复制到真实 staging texture，并按顶部到底部紧密读取 RGBA8。
fn read_target(rhi: &D3d11Context, texture: TextureHandle) -> Vec<u8> {
    let target = rhi
        .rhi_device
        .texture(texture)
        .expect("D3D11 consistency target must remain alive");
    let extent = target.desc.extent();
    let format = D3d11RhiDevice::texture_format(target.desc.format());
    let native = target.native.clone();
    let desc = D3D11_TEXTURE2D_DESC {
        Width: extent.width,
        Height: extent.height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT(format),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging: Option<ID3D11Texture2D> = None;
    unsafe {
        // SAFETY: desc 精确复用目标尺寸与格式，输出槽在同步创建期间有效。
        rhi.device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .expect("D3D11 consistency staging texture must be created");
    }
    let staging = staging.expect("D3D11 consistency staging texture must not be null");
    unsafe {
        // SAFETY: staging 与 native 同属当前 device，尺寸、格式和子资源布局完全一致。
        rhi.context.CopyResource(&staging, &native);
        // Flush 让真实 immediate context 的 draw 与 copy 进入驱动；Map 随后形成同步完成边界。
        rhi.context.Flush();
    }
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        // SAFETY: staging 使用 CPU_READ/STAGING 创建，映射输出槽在同步调用期间有效。
        rhi.context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .expect("D3D11 consistency staging Map must succeed");
    }
    let row_bytes = extent.width as usize * target.desc.format().bytes_per_pixel();
    let mut pixels = vec![0u8; row_bytes * extent.height as usize];
    for row in 0..extent.height as usize {
        let source = unsafe {
            // SAFETY: Map 成功后 pData 覆盖全部行，RowPitch 至少包含目标紧密行宽。
            mapped
                .pData
                .cast::<u8>()
                .add(row * mapped.RowPitch as usize)
        };
        let destination = pixels[row * row_bytes..].as_mut_ptr();
        unsafe {
            // SAFETY: 两个区域各自至少覆盖 row_bytes 字节且互不重叠。
            std::ptr::copy_nonoverlapping(source, destination, row_bytes);
        }
    }
    unsafe {
        // SAFETY: staging 在本函数中成功 Map 一次，此处在同一 immediate context 配对 Unmap。
        rhi.context.Unmap(&staging, 0);
    }
    pixels
}

// 用共享采样点与共享 accepts 判定真实 D3D11 RGBA 回读。
fn validate_samples(name: &str, samples: &[ConsistencySample], pixels: &[u8]) {
    let row_bytes = CONSISTENCY_EXTENT.width as usize * TextureFormat::Rgba8Unorm.bytes_per_pixel();
    for sample in samples {
        let offset = sample.y as usize * row_bytes + sample.x as usize * 4;
        let actual: [u8; 4] = pixels[offset..offset + 4]
            .try_into()
            .expect("canonical D3D11 sample must address one RGBA pixel");
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

// 把共享 Blur pass 输入机械编码成生产 D3D11 draw packet。
fn draw_blur_pass(
    rhi: &mut D3d11Context,
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
                .expect("canonical D3D11 blur vertex count must be complete"),
        ),
    );
    GraphicsDevice::draw(rhi, packet).expect("D3D11 blur subregion draw must execute");
}

// 在同一真实 D3D11 context 中执行 source→scratch→target 的水平与垂直 Blur。
fn run_blur_subregion_two_pass(rhi: &mut D3d11Context) -> usize {
    let scenario = blur_subregion_scenario();
    let contract = PipelineKind::BlurPass.contract();
    let vertex_bytes = scenario.vertex.encode_ne_bytes();
    let horizontal_bytes = scenario.horizontal.encode_ne_bytes();
    let vertical_bytes = scenario.vertical.encode_ne_bytes();
    let vertex = GraphicsDevice::create_buffer(
        rhi,
        BufferDesc::vertex(vertex_bytes.len(), contract.vertex.stride_bytes()),
    )
    .expect("D3D11 blur vertex buffer must be created");
    let horizontal =
        GraphicsDevice::create_buffer(rhi, BufferDesc::uniform(horizontal_bytes.len()))
            .expect("D3D11 blur horizontal uniform must be created");
    let vertical = GraphicsDevice::create_buffer(rhi, BufferDesc::uniform(vertical_bytes.len()))
        .expect("D3D11 blur vertical uniform must be created");
    GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(vertex, &vertex_bytes))
        .expect("D3D11 blur vertices must upload");
    GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(horizontal, &horizontal_bytes))
        .expect("D3D11 blur horizontal uniform must upload");
    GraphicsDevice::update_buffer(rhi, RhiBufferUpload::new(vertical, &vertical_bytes))
        .expect("D3D11 blur vertical uniform must upload");

    let source = GraphicsDevice::create_texture(
        rhi,
        TextureDesc::new(scenario.texture.extent, scenario.texture.format),
    )
    .expect("D3D11 blur source texture must be created");
    GraphicsDevice::update_texture(
        rhi,
        RhiTextureUpload::full(source, scenario.texture.extent, &scenario.texture.bytes),
    )
    .expect("D3D11 blur source texture must upload");
    let scratch = GraphicsDevice::create_texture(
        rhi,
        TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
    )
    .expect("D3D11 blur scratch texture must be created");
    let target = GraphicsDevice::create_texture(
        rhi,
        TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
    )
    .expect("D3D11 blur target texture must be created");
    let sampler = GraphicsDevice::create_sampler(rhi, scenario.texture.sampler)
        .expect("D3D11 blur sampler must be created");
    let pipeline = GraphicsDevice::create_pipeline(
        rhi,
        PipelineDesc {
            kind: PipelineKind::BlurPass,
        },
    )
    .expect("D3D11 blur pipeline must be created");

    let scratch_target = GraphicsDevice::resolve_render_target(rhi, scratch)
        .expect("D3D11 blur scratch target identity must resolve");
    GraphicsDevice::begin_render_pass(
        rhi,
        scratch_target,
        LoadAction::Clear(RhiColor::transparent()),
    )
    .expect("D3D11 blur horizontal pass must begin");
    draw_blur_pass(
        rhi, pipeline, vertex, horizontal, source, sampler, &scenario,
    );
    GraphicsDevice::end_render_pass(rhi).expect("D3D11 blur horizontal pass must end");

    let target_identity = GraphicsDevice::resolve_render_target(rhi, target)
        .expect("D3D11 blur target identity must resolve");
    let clear =
        RhiColor::from_straight_rgba(CONSISTENCY_BACKGROUND.map(|value| value as f32 / 255.0));
    GraphicsDevice::begin_render_pass(rhi, target_identity, LoadAction::Clear(clear))
        .expect("D3D11 blur vertical pass must begin");
    draw_blur_pass(rhi, pipeline, vertex, vertical, scratch, sampler, &scenario);
    GraphicsDevice::end_render_pass(rhi).expect("D3D11 blur vertical pass must end");
    GraphicsDevice::submit(rhi).expect("D3D11 blur two-pass commands must submit");
    let pixels = read_target(rhi, target);
    validate_samples("BlurPassTwoPass", &scenario.final_samples, &pixels);
    eprintln!(
        "D3D11 blur subregion verified: origin=({}, {}), extent={}x{}, {} invariants",
        scenario.scissor.x,
        scenario.scissor.y,
        scenario.scissor.width,
        scenario.scissor.height,
        scenario.final_samples.len(),
    );
    scenario.final_samples.len()
}

pub(super) fn run_gpu_parity_test() {
    let mut window = HiddenWindow::new();
    // 生产构造器真实执行 hardware-first、WARP fallback，并复用同一 D3D11 shader/pipeline owner。
    let mut rhi = D3d11Context::new(
        window.raw(),
        CONSISTENCY_EXTENT.width as i32,
        CONSISTENCY_EXTENT.height as i32,
    )
    .expect("D3D11 consistency production context must initialize");
    eprintln!(
        "D3D11 consistency device: {}",
        rhi.adapter_info.diagnostic_summary()
    );
    run_surface_lifecycle_test(&mut rhi);
    run_ui_production_chain_test(&mut rhi);
    run_ui_production_surface_chain_test(&mut rhi);

    let scenes = canonical_scenes();
    validate_canonical_scenes(&scenes).expect("shared consistency architecture gate must pass");
    let target = GraphicsDevice::create_texture(
        &mut rhi,
        TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
    )
    .expect("D3D11 consistency target must be created");
    let resources = scenes
        .iter()
        .enumerate()
        // 交替使用真实 DrawIndexed 与 Draw；选择不建立任何视觉期望或 pipeline 清单。
        .map(|(index, scene)| create_scene_resources(&mut rhi, scene, index.is_multiple_of(2)))
        .collect::<Vec<_>>();

    let target_identity = GraphicsDevice::resolve_render_target(&rhi, target)
        .expect("D3D11 consistency target identity must resolve");
    let clear =
        RhiColor::from_straight_rgba(CONSISTENCY_BACKGROUND.map(|value| value as f32 / 255.0));
    GraphicsDevice::begin_render_pass(&mut rhi, target_identity, LoadAction::Clear(clear))
        .expect("D3D11 consistency render pass must begin");
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
        draw_scene(&mut rhi, scene, &resources[index]);
    }
    GraphicsDevice::end_render_pass(&mut rhi).expect("D3D11 consistency render pass must end");
    GraphicsDevice::submit(&mut rhi).expect("D3D11 consistency draws must submit");
    let pixels = read_target(&rhi, target);
    for scene in &scenes {
        validate_samples(scene.name, &scene.samples, &pixels);
        eprintln!(
            "D3D11 pipeline verified: {}; {} invariants",
            scene.name,
            scene.samples.len()
        );
    }
    let scene_invariants = scenes
        .iter()
        .map(|scene| scene.samples.len())
        .sum::<usize>();
    eprintln!(
        "D3D11 consistency verified: {} PipelineKind, {scene_invariants} direct invariants",
        scenes.len(),
    );

    let blur_invariants = run_blur_subregion_two_pass(&mut rhi);
    eprintln!(
        "D3D11 consistency complete: {scene_invariants} shared scene invariants; Blur two-pass {blur_invariants} final invariants",
    );
    rhi.shutdown_result()
        .expect("D3D11 consistency context must shut down");
    drop(rhi);
    window.shutdown();
}
