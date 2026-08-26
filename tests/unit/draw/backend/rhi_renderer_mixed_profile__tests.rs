//! 混合绘制产物到 RHI 帧计划的纯 CPU 性能取样。

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::*;
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, GraphicsDeviceCapabilities, PipelineBinding, PipelineDesc,
    PipelineHandle, RenderTargetHandle, RhiBufferUpload, RhiBufferUploadPreflight, RhiColor,
    RhiScissor, RhiTextureUpload, SamplerDesc, SamplerHandle, SubmissionHandle, TextureCopy,
    TextureDesc, TextureHandle, TextureMove,
};

const GROUP_COUNT: usize = 32;
const GLYPHS_PER_GROUP: usize = 12;
const STEADY_FRAMES: usize = 240;
const TIMING_ROUNDS: usize = 9;

struct CountingAllocator;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                let added = new_size - layout.size();
                let live = LIVE_BYTES.fetch_add(added, Ordering::Relaxed) + added;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct AllocationStats {
    count: usize,
    bytes: usize,
    peak_live: usize,
    final_live: usize,
}

struct CpuDevice {
    next_resource: u64,
    draw_count: usize,
}

impl CpuDevice {
    fn new() -> Self {
        Self {
            next_resource: 10,
            draw_count: 0,
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

    fn create_texture(&mut self, _desc: TextureDesc) -> Result<TextureHandle> {
        Ok(TextureHandle::from_raw(self.allocate()))
    }

    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        Ok(RenderTargetHandle::for_test(texture))
    }

    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        Ok(())
    }

    fn update_texture(&mut self, _upload: RhiTextureUpload<'_>) -> Result<()> {
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

fn allocation_stats<F: FnOnce()>(action: F) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    action();
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live: LIVE_BYTES.load(Ordering::Relaxed),
    }
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
