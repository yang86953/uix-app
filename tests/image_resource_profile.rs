//! 固定 PNG 图片资源从冷解码到稳态复用、绘制准备与释放的性能取样。

#![cfg(feature = "image-codecs")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::time::Instant;

use uix::core::{Rect, Size};
use uix::draw::resources::image::{blit_handle, BitmapHandle, ImageService};
use uix::draw::{
    BlendMode, Canvas2D, Color, FillRule, GradientDirection, Path as DrawPath, Radius,
    StrokeOptions, Transform,
};

const SMALL_BYTES: &[u8] = include_bytes!("../assets/images/demo.png");
const LARGE_BYTES: &[u8] = include_bytes!("../assets/images/acceptance-1461-before.png");
const WARM_DECODES: usize = 16;
const COLD_LOADS_PER_ROUND: usize = 4;
const CACHE_WARM_HITS: usize = 1024;
const STEADY_HITS: usize = 240;
const DRAW_PREPARES: usize = 16;
const TIMING_ROUNDS: usize = 9;

struct CountingAllocator;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);
static PEAK_LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

fn add_live(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes as isize, Ordering::Relaxed) + bytes as isize;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add_live(layout.size());
            if MEASURING.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            }
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            add_live(layout.size());
            if MEASURING.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            }
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            let delta = new_size as isize - layout.size() as isize;
            let live = LIVE_BYTES.fetch_add(delta, Ordering::Relaxed) + delta;
            PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            if MEASURING.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            }
        }
        resized
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct AllocationStats {
    count: usize,
    bytes: usize,
    peak_live_delta: isize,
    final_live_delta: isize,
}

fn allocation_stats<F: FnOnce()>(action: F) -> AllocationStats {
    let live_before = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(live_before, Ordering::Relaxed);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    action();
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_delta: PEAK_LIVE_BYTES.load(Ordering::Relaxed) - live_before,
        final_live_delta: LIVE_BYTES.load(Ordering::Relaxed) - live_before,
    }
}

fn asset_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/images")
        .join(name)
}

fn median(mut values: [u128; TIMING_ROUNDS]) -> u128 {
    values.sort_unstable();
    values[TIMING_ROUNDS / 2]
}

fn verify_slot(service: &ImageService, handle: BitmapHandle, width: i32, height: i32) -> u64 {
    service
        .with_slot(handle, |slot| {
            assert_eq!((slot.width(), slot.height()), (width, height));
            assert_eq!(slot.pixels().len(), width as usize * height as usize);
            slot.pixels().iter().fold(0_u64, |checksum, pixel| {
                checksum.rotate_left(5) ^ u64::from(*pixel)
            })
        })
        .expect("有效图片句柄必须保留尺寸与预乘像素")
}

#[derive(Default)]
struct RetainingCanvas {
    retained: Option<std::sync::Arc<Vec<u32>>>,
    transform: Transform,
}

impl Canvas2D for RetainingCanvas {
    fn current_transform(&self) -> Transform {
        self.transform
    }
    fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }
    fn fill_rect(&mut self, _: Rect, _: Color, _: Option<Radius>) {}
    fn fill_circle(&mut self, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_ellipse(&mut self, _: Rect, _: Color) {}
    fn fill_sector(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_path(&mut self, _: &DrawPath, _: Color, _: FillRule) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: Option<Radius>) {}
    fn stroke_circle(&mut self, _: f32, _: f32, _: f32, _: Color, _: f32) {}
    fn stroke_path(&mut self, _: &DrawPath, _: Color, _: &StrokeOptions) {}
    fn draw_line(&mut self, _: f32, _: f32, _: f32, _: f32, _: Color, _: f32) {}
    fn fill_linear_gradient(&mut self, _: Rect, _: Color, _: Color, _: GradientDirection) {}
    fn fill_radial_gradient(&mut self, _: f32, _: f32, _: f32, _: f32, _: Color, _: Color) {}
    fn draw_box_shadow(&mut self, _: Rect, _: f32, _: f32, _: f32, _: Color, _: Option<Radius>) {}
    fn draw_box_shadow_ambient(
        &mut self,
        _: Rect,
        _: f32,
        _: f32,
        _: f32,
        _: Color,
        _: Option<Radius>,
    ) {
    }
    fn blit_image(&mut self, source: &[u32], _: i32, _: Rect, _: Rect) {
        self.retained = Some(std::sync::Arc::new(source.to_vec()));
    }
    fn blit_image_shared(&mut self, source: std::sync::Arc<Vec<u32>>, _: i32, _: Rect, _: Rect) {
        self.retained = Some(source);
    }
    fn blit_glyph(&mut self, _: i32, _: i32, _: &[u8], _: usize, _: usize, _: Color) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn set_opacity(&mut self, _: f32) {}
    fn opacity(&self) -> f32 {
        1.0
    }
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn push_clip_path(&mut self, _: &DrawPath) {}
    fn pixels(&self) -> &[u32] {
        self.retained
            .as_ref()
            .map_or(&[], |pixels| pixels.as_slice())
    }
    fn surface_size(&self) -> Size {
        Size::new(1200.0, 800.0)
    }
    fn current_clip(&self) -> Rect {
        Rect::new(0.0, 0.0, 1200.0, 800.0)
    }
    fn scroll_region(&mut self, _: Rect, _: f32, _: f32) {}
}

fn prepare_retained_pixels(
    service: &ImageService,
    handle: BitmapHandle,
    canvas: &mut RetainingCanvas,
) -> usize {
    blit_handle(
        service,
        canvas,
        handle,
        Rect::new(0.0, 0.0, 1200.0, 800.0),
        false,
    );
    black_box(canvas.pixels()).len()
}

fn premultiply_profile_pixel(pixel: &[u8]) -> u32 {
    let red = u32::from(pixel[0]);
    let green = u32::from(pixel[1]);
    let blue = u32::from(pixel[2]);
    let alpha = u32::from(pixel[3]);
    match alpha {
        0 => 0,
        255 => 0xFF00_0000 | (red << 16) | (green << 8) | blue,
        _ => {
            let red = red * alpha / 255;
            let green = green * alpha / 255;
            let blue = blue * alpha / 255;
            (alpha << 24) | (red << 16) | (green << 8) | blue
        }
    }
}

#[test]
#[ignore = "性能取样需独占进程并读取仓库固定 PNG"]
fn profile_image_resource_pipeline() {
    let small_path = asset_path("demo.png");
    let large_path = asset_path("acceptance-1461-before.png");
    // 预先触达固定文件，把文件系统页缓存与图片服务冷加载分开。
    black_box(std::fs::read(&small_path).expect("小 PNG 资源应可读取"));
    black_box(std::fs::read(&large_path).expect("大 PNG 资源应可读取"));

    for _ in 0..WARM_DECODES {
        black_box(uix::draw::resources::image::decode_to_pixels(SMALL_BYTES).unwrap());
        black_box(uix::draw::resources::image::decode_to_pixels(LARGE_BYTES).unwrap());
    }

    // 按生产实现的三个互斥步骤复现大 PNG 解码，分离第三方解码、RGBA 准备与预乘输出。
    let mut decoder_samples = [0_u128; TIMING_ROUNDS];
    let mut rgba_samples = [0_u128; TIMING_ROUNDS];
    let mut premultiply_samples = [0_u128; TIMING_ROUNDS];
    for round in 0..TIMING_ROUNDS {
        let decoder_start = Instant::now();
        let decoded = image::load_from_memory(LARGE_BYTES).unwrap();
        decoder_samples[round] = decoder_start.elapsed().as_nanos();

        let rgba_start = Instant::now();
        let rgba = decoded.into_rgba8();
        rgba_samples[round] = rgba_start.elapsed().as_nanos();

        let premultiply_start = Instant::now();
        let pixels = rgba
            .chunks_exact(4)
            .map(premultiply_profile_pixel)
            .collect::<Vec<_>>();
        premultiply_samples[round] = premultiply_start.elapsed().as_nanos();
        black_box(pixels);
    }

    let mut decoded = None;
    let decoder_allocations = allocation_stats(|| {
        decoded = Some(image::load_from_memory(LARGE_BYTES).unwrap());
    });
    let decoded = decoded.expect("固定大 PNG 必须可解码");
    let mut rgba = None;
    let rgba_allocations = allocation_stats(|| {
        rgba = Some(decoded.into_rgba8());
    });
    let rgba = rgba.expect("解码图片必须可准备为 RGBA8");
    let mut premultiplied = None;
    let premultiply_allocations = allocation_stats(|| {
        premultiplied = Some(
            rgba.chunks_exact(4)
                .map(premultiply_profile_pixel)
                .collect::<Vec<_>>(),
        );
    });
    black_box(premultiplied);

    let mut cold_samples = [0_u128; TIMING_ROUNDS];
    for sample in &mut cold_samples {
        let start = Instant::now();
        for _ in 0..COLD_LOADS_PER_ROUND {
            let service = ImageService::new();
            let small = service.load_from_bytes(SMALL_BYTES).unwrap();
            let large = service.load_from_bytes(LARGE_BYTES).unwrap();
            black_box((small, large));
        }
        *sample = start.elapsed().as_nanos() / (COLD_LOADS_PER_ROUND * 2) as u128;
    }

    let service = ImageService::new();
    let memory_before = service.memory_usage();
    let mut handles = None;
    let cold_allocations = allocation_stats(|| {
        handles = Some((
            service.load_from_path(&small_path).unwrap(),
            service.load_from_path(&large_path).unwrap(),
        ));
    });
    let (small, large) = handles.expect("两张固定图片必须完成冷加载");
    let memory_loaded = service.memory_usage();
    let small_checksum = verify_slot(&service, small, 1, 1);
    let large_checksum = verify_slot(&service, large, 1200, 800);
    let checksum = small_checksum ^ large_checksum.rotate_left(13);
    assert_eq!(checksum, 7_293_468_762_605_642_447);
    assert_eq!(service.load_from_path(&small_path).unwrap(), small);
    assert_eq!(service.load_from_path(&large_path).unwrap(), large);

    for index in 0..CACHE_WARM_HITS {
        let path = if index & 1 == 0 {
            &small_path
        } else {
            &large_path
        };
        black_box(service.load_from_path(path).unwrap());
    }
    let mut hit_samples = [0_u128; TIMING_ROUNDS];
    for sample in &mut hit_samples {
        let start = Instant::now();
        for index in 0..STEADY_HITS {
            let path = if index & 1 == 0 {
                &small_path
            } else {
                &large_path
            };
            black_box(service.load_from_path(path).unwrap());
        }
        *sample = start.elapsed().as_nanos() / STEADY_HITS as u128;
    }
    let steady_allocations = allocation_stats(|| {
        for index in 0..STEADY_HITS {
            let path = if index & 1 == 0 {
                &small_path
            } else {
                &large_path
            };
            black_box(service.load_from_path(path).unwrap());
        }
    });

    let mut canvas = RetainingCanvas::default();
    for _ in 0..32 {
        black_box(prepare_retained_pixels(&service, large, &mut canvas));
    }
    let mut draw_samples = [0_u128; TIMING_ROUNDS];
    for sample in &mut draw_samples {
        let start = Instant::now();
        for _ in 0..DRAW_PREPARES {
            black_box(prepare_retained_pixels(&service, large, &mut canvas));
        }
        *sample = start.elapsed().as_nanos() / DRAW_PREPARES as u128;
    }
    let draw_allocations = allocation_stats(|| {
        black_box(prepare_retained_pixels(&service, large, &mut canvas));
    });

    let release_stats = allocation_stats(|| {
        service.unload(small);
        service.unload(large);
    });
    let memory_after = service.memory_usage();
    assert!(!service.is_valid(small));
    assert!(!service.is_valid(large));
    assert_eq!(memory_after, 2 * std::mem::size_of::<u32>());
    assert_eq!(canvas.pixels().len(), 1200 * 800);
    let retained_checksum = canvas.pixels().iter().fold(0_u64, |checksum, pixel| {
        checksum.rotate_left(5) ^ u64::from(*pixel)
    });
    assert_eq!(retained_checksum, large_checksum);
    let frame_drop_stats = allocation_stats(|| {
        canvas.retained = None;
    });

    eprintln!(
        "PROFILE image_resource cold_ns_per_image={} cache_hit_ns={} draw_prepare_ns={} cold_allocs={} cold_bytes={} cold_peak_live={} cold_final_live={} steady_allocs={} steady_bytes={} steady_peak_live={} steady_final_live={} draw_allocs={} draw_bytes={} draw_peak_live={} draw_final_live={} release_peak_live={} release_final_live={} frame_drop_final_live={} service_memory_before={} service_memory_loaded={} service_memory_after={} checksum={}",
        median(cold_samples),
        median(hit_samples),
        median(draw_samples),
        cold_allocations.count,
        cold_allocations.bytes,
        cold_allocations.peak_live_delta,
        cold_allocations.final_live_delta,
        steady_allocations.count,
        steady_allocations.bytes,
        steady_allocations.peak_live_delta,
        steady_allocations.final_live_delta,
        draw_allocations.count,
        draw_allocations.bytes,
        draw_allocations.peak_live_delta,
        draw_allocations.final_live_delta,
        release_stats.peak_live_delta,
        release_stats.final_live_delta,
        frame_drop_stats.final_live_delta,
        memory_before,
        memory_loaded,
        memory_after,
        black_box(checksum),
    );
    eprintln!(
        "PROFILE image_resource_stage decoder_ns={} rgba_prepare_ns={} premultiply_ns={} decoder_allocs={} decoder_bytes={} decoder_peak_live={} decoder_final_live={} rgba_allocs={} rgba_bytes={} rgba_peak_live={} rgba_final_live={} premultiply_allocs={} premultiply_bytes={} premultiply_peak_live={} premultiply_final_live={}",
        median(decoder_samples),
        median(rgba_samples),
        median(premultiply_samples),
        decoder_allocations.count,
        decoder_allocations.bytes,
        decoder_allocations.peak_live_delta,
        decoder_allocations.final_live_delta,
        rgba_allocations.count,
        rgba_allocations.bytes,
        rgba_allocations.peak_live_delta,
        rgba_allocations.final_live_delta,
        premultiply_allocations.count,
        premultiply_allocations.bytes,
        premultiply_allocations.peak_live_delta,
        premultiply_allocations.final_live_delta,
    );
}
