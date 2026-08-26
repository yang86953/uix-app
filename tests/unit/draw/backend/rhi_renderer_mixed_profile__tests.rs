//! 混合绘制产物到 RHI 帧计划的纯 CPU 性能取样。

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use super::*;
use crate::core::{Errc, Error};
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, GraphicsDeviceCapabilities, PipelineBinding, PipelineDesc,
    PipelineHandle, RenderTargetHandle, RhiBufferUpload, RhiBufferUploadPreflight, RhiColor,
    RhiExtent, RhiScissor, RhiTextureUpload, SamplerDesc, SamplerHandle, SubmissionHandle,
    TextureCopy, TextureDesc, TextureFormat, TextureHandle, TextureMove,
};
use crate::test_allocation_probe::allocation_stats;

const GROUP_COUNT: usize = 32;
const GLYPHS_PER_GROUP: usize = 12;
const STEADY_FRAMES: usize = 240;
const TIMING_ROUNDS: usize = 9;

struct CpuDevice {
    next_resource: u64,
    draw_count: usize,
    last_texture: Option<(TextureHandle, TextureDesc)>,
    upload_observation: u64,
}

impl CpuDevice {
    fn new() -> Self {
        Self {
            next_resource: 10,
            draw_count: 0,
            last_texture: None,
            upload_observation: 0,
        }
    }

    fn allocate(&mut self) -> u64 {
        let raw = self.next_resource;
        self.next_resource += 1;
        raw
    }
}

impl GraphicsDevice for CpuDevice {
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        GraphicsDeviceCapabilities::full_gpu_baseline()
    }

    fn create_buffer(&mut self, _desc: BufferDesc) -> Result<BufferHandle> {
        Ok(BufferHandle::from_raw(self.allocate()))
    }

    fn update_buffer(&mut self, _upload: RhiBufferUpload<'_>) -> Result<()> {
        Ok(())
    }

    fn preflight_buffer_upload(&self, _upload: RhiBufferUploadPreflight) -> Result<()> {
        Ok(())
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        let texture = TextureHandle::from_raw(self.allocate());
        self.last_texture = Some((texture, desc));
        Ok(texture)
    }

    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        Ok(RenderTargetHandle::for_test(texture))
    }

    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        Ok(())
    }

    fn update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        let Some((texture, desc)) = self.last_texture else {
            return Err(Error::new(
                Errc::InvalidState,
                "CPU profile device texture upload has no created resource",
            ));
        };
        if upload.texture() != texture {
            return Err(Error::new(
                Errc::InvalidArgument,
                "CPU profile device texture upload targets an unknown resource",
            ));
        }
        let validated = upload.validate(desc)?;
        let data = validated.data();
        self.upload_observation = u64::from(validated.row_pitch())
            ^ u64::from(data.first().copied().unwrap_or_default())
            ^ u64::from(data.last().copied().unwrap_or_default()).rotate_left(13);
        Ok(())
    }

    fn create_sampler(&mut self, _desc: SamplerDesc) -> Result<SamplerHandle> {
        Ok(SamplerHandle::from_raw(self.allocate()))
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        Ok(PipelineBinding::for_test(
            PipelineHandle::from_raw(self.allocate()),
            desc.kind,
        ))
    }

    fn destroy_buffer(&mut self, _buffer: BufferHandle) -> Result<()> {
        Ok(())
    }

    fn destroy_texture(&mut self, _texture: TextureHandle) -> Result<()> {
        Ok(())
    }

    fn begin_render_pass(&mut self, _target: RenderTargetHandle, _load: LoadAction) -> Result<()> {
        Ok(())
    }

    fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
        Ok(())
    }

    fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
        self.draw_count += 1;
        Ok(())
    }

    fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
        Ok(())
    }

    fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
        Ok(())
    }

    fn end_render_pass(&mut self) -> Result<()> {
        Ok(())
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        Ok(SubmissionHandle::from_raw(1))
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> [[f32; 2]; 4] {
    [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ]
}

fn mixed_ops(coverage: &Arc<[u8]>) -> Vec<RhiOp> {
    let mut operations = Vec::with_capacity(GROUP_COUNT * (GLYPHS_PER_GROUP + 2));
    for group in 0..GROUP_COUNT {
        let x = (group % 8) as f32 * 92.0 + 8.0;
        let y = (group / 8) as f32 * 120.0 + 8.0;
        let scissor = Some(RhiScissor {
            x: x as i32,
            y: y as i32,
            width: 84,
            height: 108,
        });
        operations.push(RhiOp::Gradient(RhiGradientRect {
            x,
            y,
            w: 84.0,
            h: 108.0,
            corners: rect(x, y, 84.0, 108.0),
            color_a: [0.08, 0.16, 0.35, 1.0],
            color_b: [0.30, 0.55, 0.92, 1.0],
            params: [0.0, 1.0, 0.0, 92.0],
            scissor,
        }));
        operations.push(RhiOp::Shape(RhiShapeRect {
            x: x + 1.0,
            y: y + 1.0,
            w: 82.0,
            h: 106.0,
            rgba: [0.95, 0.95, 1.0, 1.0],
            radius: [8.0; 4],
            half_stroke: 1.0,
            scissor,
        }));
        for glyph in 0..GLYPHS_PER_GROUP {
            let glyph_x = x + 6.0 + (glyph % 4) as f32 * 18.0;
            let glyph_y = y + 10.0 + (glyph / 4) as f32 * 24.0;
            operations.push(RhiOp::Coverage(RhiCoverageQuad {
                x: glyph_x,
                y: glyph_y,
                w: 12.0,
                h: 18.0,
                corners: rect(glyph_x, glyph_y, 12.0, 18.0),
                rgba: [0.92, 0.94, 1.0, 1.0],
                coverage: Arc::clone(coverage),
                pixel_w: 12,
                pixel_h: 18,
                scissor,
            }));
        }
    }
    operations
}

fn execute_frame(renderer: &mut RhiRenderer, device: &mut CpuDevice, operations: &[RhiOp]) {
    renderer
        .execute_ops(
            super::super::RhiRendererFrame::offscreen(device, TextureHandle::from_raw(1)),
            RhiViewport {
                width: 768.0,
                height: 512.0,
            },
            LoadAction::Load,
            operations,
        )
        .expect("混合帧计划应通过 CPU 编码与共享契约验证");
}

fn median(mut values: [u128; TIMING_ROUNDS]) -> u128 {
    values.sort_unstable();
    values[TIMING_ROUNDS / 2]
}

#[test]
#[ignore = "性能取样需独占测试进程，避免全局分配统计受到并行测试干扰"]
fn profile_mixed_ui_draw_preparation() {
    let coverage: Arc<[u8]> = Arc::from(vec![220_u8; 12 * 18]);
    let operations = mixed_ops(&coverage);
    assert_eq!(operations.len(), GROUP_COUNT * (GLYPHS_PER_GROUP + 2));

    let cold_start = Instant::now();
    let mut cold_renderer = RhiRenderer::default();
    let mut cold_device = CpuDevice::new();
    execute_frame(&mut cold_renderer, &mut cold_device, &operations);
    let cold_ns = cold_start.elapsed().as_nanos();

    let mut renderer = RhiRenderer::default();
    let mut device = CpuDevice::new();
    for _ in 0..32 {
        execute_frame(&mut renderer, &mut device, &operations);
    }

    let mut generation_samples = [0_u128; TIMING_ROUNDS];
    let mut backend_samples = [0_u128; TIMING_ROUNDS];
    for round in 0..TIMING_ROUNDS {
        let generation_start = Instant::now();
        for _ in 0..STEADY_FRAMES {
            black_box(mixed_ops(&coverage));
        }
        generation_samples[round] = generation_start.elapsed().as_nanos();

        let backend_start = Instant::now();
        for _ in 0..STEADY_FRAMES {
            execute_frame(&mut renderer, &mut device, black_box(&operations));
        }
        backend_samples[round] = backend_start.elapsed().as_nanos();
    }

    let generation_allocations = allocation_stats(|| {
        black_box(mixed_ops(&coverage));
    });
    let backend_allocations = allocation_stats(|| {
        execute_frame(&mut renderer, &mut device, black_box(&operations));
    });
    let generation_ns = median(generation_samples) / STEADY_FRAMES as u128;
    let backend_ns = median(backend_samples) / STEADY_FRAMES as u128;
    eprintln!(
        "PROFILE mixed_draw cold_ns={cold_ns} generation_ns_per_frame={generation_ns} backend_ns_per_frame={backend_ns}"
    );
    eprintln!(
        "PROFILE mixed_draw generation_allocations={} generation_bytes={} generation_peak_live={} generation_final_live={} backend_allocations={} backend_bytes={} backend_peak_live={} backend_final_live={} draws_per_frame={}",
        generation_allocations.count,
        generation_allocations.bytes,
        generation_allocations.peak_live,
        generation_allocations.final_live,
        backend_allocations.count,
        backend_allocations.bytes,
        backend_allocations.peak_live,
        backend_allocations.final_live,
        device.draw_count / (32 + TIMING_ROUNDS * STEADY_FRAMES + 1),
    );
    assert!(device.draw_count > 0);

    // 同一 Arc 的稳态命中只保留一个受 atlas 生命周期约束的身份条目。
    assert_eq!(renderer.coverage_atlas_cache.len(), 1);
    assert_eq!(renderer.coverage_atlas_identity_cache.len(), 1);
    // 新 Arc 即使内容相同也不得扩张身份索引，内容缓存仍可复用原 placement。
    let equivalent_coverage: Arc<[u8]> = Arc::from(vec![220_u8; 12 * 18]);
    let equivalent_operations = mixed_ops(&equivalent_coverage);
    execute_frame(&mut renderer, &mut device, &equivalent_operations);
    assert_eq!(renderer.coverage_atlas_cache.len(), 1);
    assert_eq!(renderer.coverage_atlas_identity_cache.len(), 1);
    // 不同内容必须继续建立独立 atlas 条目，不能因尺寸相同误命中身份快路径。
    let distinct_coverage: Arc<[u8]> = Arc::from(vec![219_u8; 12 * 18]);
    let distinct_operations = mixed_ops(&distinct_coverage);
    execute_frame(&mut renderer, &mut device, &distinct_operations);
    assert_eq!(renderer.coverage_atlas_cache.len(), 2);
    assert_eq!(renderer.coverage_atlas_identity_cache.len(), 2);
    // Renderer 的显式释放边界必须逆序清空身份、内容和页面所有权。
    renderer
        .release_coverage_atlas(&mut device)
        .expect("coverage atlas 与身份索引应在同一生命周期边界释放");
    assert!(renderer.coverage_atlas_identity_cache.is_empty());
    assert!(renderer.coverage_atlas_cache.is_empty());
    assert!(renderer.coverage_atlas_pages.is_empty());
}

const IMAGE_WIDTH: u32 = 1200;
const IMAGE_HEIGHT: u32 = 800;
const IMAGE_UPDATES: usize = 240;

fn image_operation(pixels: &Arc<Vec<u32>>) -> RhiOp {
    RhiOp::Textured(RhiTexturedQuad {
        x: 0.0,
        y: 0.0,
        w: IMAGE_WIDTH as f32,
        h: IMAGE_HEIGHT as f32,
        corners: rect(0.0, 0.0, IMAGE_WIDTH as f32, IMAGE_HEIGHT as f32),
        rgba: [1.0; 4],
        additive: false,
        pixels: Arc::clone(pixels),
        pixel_w: IMAGE_WIDTH,
        pixel_h: IMAGE_HEIGHT,
        scissor: None,
    })
}

#[test]
fn image_upload_encoding_preserves_bgra_bytes_on_native_and_big_endian_fallback() {
    let pixels = [0x8040_2010_u32, 0xFFCC_AA55_u32, 0x0000_0000_u32];
    let expected = [
        0x10, 0x20, 0x40, 0x80, 0x55, 0xAA, 0xCC, 0xFF, 0x00, 0x00, 0x00, 0x00,
    ];
    let native = RhiRenderer::encode_u32s(&pixels);
    #[cfg(target_endian = "little")]
    assert!(matches!(native, std::borrow::Cow::Borrowed(_)));
    assert_eq!(native.as_ref(), expected);
    assert_eq!(RhiRenderer::encode_u32s_big_endian(&pixels), expected);
}

#[test]
#[ignore = "性能取样需独占测试进程，避免全局分配统计受到并行测试干扰"]
fn profile_full_image_texture_upload() {
    let pixels = Arc::new(vec![0x8040_2010_u32; (IMAGE_WIDTH * IMAGE_HEIGHT) as usize]);
    let extent = RhiExtent::new(IMAGE_WIDTH, IMAGE_HEIGHT);
    let desc = TextureDesc::new(extent, TextureFormat::Bgra8Unorm);
    let texture = TextureHandle::from_raw(77);

    for _ in 0..32 {
        black_box(image_operation(&pixels));
        black_box(RhiRenderer::encode_u32s(pixels.as_slice()));
    }
    let encoded = RhiRenderer::encode_u32s(pixels.as_slice());
    assert_eq!(encoded.len(), pixels.len() * std::mem::size_of::<u32>());
    assert_eq!(&encoded[..4], &[0x10, 0x20, 0x40, 0x80]);
    let validated = RhiTextureUpload::full(texture, extent, &encoded)
        .validate(desc)
        .expect("完整 BGRA 图片上传必须通过尺寸、容量与 row pitch 验证");
    assert_eq!(validated.row_pitch(), IMAGE_WIDTH * 4);
    assert_eq!(validated.data().len(), encoded.len());

    let mut device = CpuDevice::new();
    let device_texture = device
        .create_texture(desc)
        .expect("CPU profile device 必须创建图片纹理");
    let device_upload = RhiTextureUpload::full(device_texture, extent, &encoded);
    device
        .update_texture(device_upload)
        .expect("CPU profile device 必须同步验证并消费上传借用");

    let operations = [image_operation(&pixels)];
    let mut renderer = RhiRenderer::default();
    for _ in 0..32 {
        renderer
            .execute_ops(
                super::super::RhiRendererFrame::offscreen(&mut device, TextureHandle::from_raw(1)),
                RhiViewport {
                    width: IMAGE_WIDTH as f32,
                    height: IMAGE_HEIGHT as f32,
                },
                LoadAction::Load,
                &operations,
            )
            .expect("完整图片必须通过 RHI textured upload 同步边界");
    }

    let mut lowering_samples = [0_u128; TIMING_ROUNDS];
    let mut encoding_samples = [0_u128; TIMING_ROUNDS];
    let mut validation_samples = [0_u128; TIMING_ROUNDS];
    let mut device_samples = [0_u128; TIMING_ROUNDS];
    let mut frame_samples = [0_u128; TIMING_ROUNDS];
    for round in 0..TIMING_ROUNDS {
        let start = Instant::now();
        for _ in 0..IMAGE_UPDATES {
            black_box(image_operation(&pixels));
        }
        lowering_samples[round] = start.elapsed().as_nanos() / IMAGE_UPDATES as u128;

        let start = Instant::now();
        for _ in 0..IMAGE_UPDATES {
            black_box(RhiRenderer::encode_u32s(pixels.as_slice()));
        }
        encoding_samples[round] = start.elapsed().as_nanos() / IMAGE_UPDATES as u128;

        let start = Instant::now();
        for _ in 0..IMAGE_UPDATES {
            black_box(
                RhiTextureUpload::full(texture, extent, encoded.as_ref())
                    .validate(desc)
                    .expect("重复验证必须保持相同资源契约"),
            );
        }
        validation_samples[round] = start.elapsed().as_nanos() / IMAGE_UPDATES as u128;

        device.last_texture = Some((device_texture, desc));
        let start = Instant::now();
        for _ in 0..IMAGE_UPDATES {
            device
                .update_texture(device_upload)
                .expect("同步 Device 重复消费不得改变借用契约");
        }
        device_samples[round] = start.elapsed().as_nanos() / IMAGE_UPDATES as u128;

        let start = Instant::now();
        for _ in 0..IMAGE_UPDATES {
            renderer
                .execute_ops(
                    super::super::RhiRendererFrame::offscreen(
                        &mut device,
                        TextureHandle::from_raw(1),
                    ),
                    RhiViewport {
                        width: IMAGE_WIDTH as f32,
                        height: IMAGE_HEIGHT as f32,
                    },
                    LoadAction::Load,
                    &operations,
                )
                .expect("稳态完整图片帧必须成功");
        }
        frame_samples[round] = start.elapsed().as_nanos() / IMAGE_UPDATES as u128;
    }

    let lowering_allocations = allocation_stats(|| {
        black_box(image_operation(&pixels));
    });
    let encoding_allocations = allocation_stats(|| {
        black_box(RhiRenderer::encode_u32s(pixels.as_slice()));
    });
    let validation_allocations = allocation_stats(|| {
        black_box(
            RhiTextureUpload::full(texture, extent, encoded.as_ref())
                .validate(desc)
                .expect("分配测量期间验证必须成功"),
        );
    });
    device.last_texture = Some((device_texture, desc));
    let device_allocations = allocation_stats(|| {
        device
            .update_texture(device_upload)
            .expect("分配测量期间 Device 必须成功");
    });
    let frame_allocations = allocation_stats(|| {
        renderer
            .execute_ops(
                super::super::RhiRendererFrame::offscreen(&mut device, TextureHandle::from_raw(1)),
                RhiViewport {
                    width: IMAGE_WIDTH as f32,
                    height: IMAGE_HEIGHT as f32,
                },
                LoadAction::Load,
                &operations,
            )
            .expect("分配测量期间完整图片帧必须成功");
    });

    eprintln!(
        "PROFILE image_upload lowering_ns={} encoding_ns={} validation_ns={} device_ns={} frame_ns={} lowering_allocs={} lowering_bytes={} lowering_peak={} lowering_final={} encoding_allocs={} encoding_bytes={} encoding_peak={} encoding_final={} validation_allocs={} validation_bytes={} validation_peak={} validation_final={} device_allocs={} device_bytes={} device_peak={} device_final={} frame_allocs={} frame_bytes={} frame_peak={} frame_final={} observation={}",
        median(lowering_samples),
        median(encoding_samples),
        median(validation_samples),
        median(device_samples),
        median(frame_samples),
        lowering_allocations.count,
        lowering_allocations.bytes,
        lowering_allocations.peak_live,
        lowering_allocations.final_live,
        encoding_allocations.count,
        encoding_allocations.bytes,
        encoding_allocations.peak_live,
        encoding_allocations.final_live,
        validation_allocations.count,
        validation_allocations.bytes,
        validation_allocations.peak_live,
        validation_allocations.final_live,
        device_allocations.count,
        device_allocations.bytes,
        device_allocations.peak_live,
        device_allocations.final_live,
        frame_allocations.count,
        frame_allocations.bytes,
        frame_allocations.peak_live,
        frame_allocations.final_live,
        device.upload_observation,
    );
}
