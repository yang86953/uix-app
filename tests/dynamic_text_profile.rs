//! 动态多语言文本编辑管线的纯 CPU 性能取样。

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use uix::core::Point;
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::{FontHandle, FontService, HAlign, VAlign};

const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");
const CACHE_WARM_UPDATES: usize = 1024;
const STEADY_UPDATES: usize = 240;
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

#[derive(Clone, Copy, Default)]
struct PhaseTimes {
    layout: Duration,
    cursor_hit: Duration,
    glyph_prepare: Duration,
}

impl PhaseTimes {
    fn add(&mut self, other: Self) {
        self.layout += other.layout;
        self.cursor_hit += other.cursor_hit;
        self.glyph_prepare += other.glyph_prepare;
    }
}

#[derive(Clone, Copy)]
struct AllocationStats {
    count: usize,
    bytes: usize,
    peak_live_delta: isize,
    final_live_delta: isize,
}

fn options() -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: 420.0,
        max_height: 0.0,
        line_height: 24.0,
        word_wrap: true,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 17.0,
    }
}

fn dynamic_text(index: usize) -> String {
    format!(
        "第 {index} 次编辑：快速输入会持续改变内容与光标位置。UIX 需要在有限宽度内稳定换行，并保持中文、English words、Résumé naïve café 的字形顺序。\n第二段包含标点（，。！？）、数字 2026-{index:04} 和组合字符 e\u{301}，用于覆盖真实编辑后的重新测量与塑形。"
    )
}

fn execute_update(
    service: &FontService,
    font: FontHandle,
    text: &str,
    cursor: usize,
    opts: &TextLayoutOptions,
) -> (PhaseTimes, usize) {
    let layout_start = Instant::now();
    let layout = service.layout_text_shared(&font, text, opts);
    let layout_elapsed = layout_start.elapsed();

    let cursor_start = Instant::now();
    let cursor_x = service.text_cursor_x(&font, text, opts, cursor);
    let hit = service
        .hit_test_text(
            &font,
            text,
            opts,
            Point::new(cursor_x, opts.line_height * 0.5),
        )
        .unwrap_or(0);
    let cursor_elapsed = cursor_start.elapsed();

    let glyph_start = Instant::now();
    let mut prepared_bytes = 0usize;
    for glyph in &layout.glyphs {
        let raster = service.rasterize_glyph(&glyph.font, glyph.glyph_id, opts.font_size);
        prepared_bytes = prepared_bytes.saturating_add(raster.coverage.len());
    }
    let glyph_elapsed = glyph_start.elapsed();

    (
        PhaseTimes {
            layout: layout_elapsed,
            cursor_hit: cursor_elapsed,
            glyph_prepare: glyph_elapsed,
        },
        layout.glyphs.len() ^ layout.lines.len() ^ hit ^ prepared_bytes,
    )
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

fn median(mut values: [u128; TIMING_ROUNDS]) -> u128 {
    values.sort_unstable();
    values[TIMING_ROUNDS / 2]
}

#[test]
#[ignore = "性能取样需独占进程并加载仓库固定字体"]
fn profile_dynamic_text_edit_pipeline() {
    let opts = options();
    let total_updates = CACHE_WARM_UPDATES + TIMING_ROUNDS * STEADY_UPDATES + 1;
    let texts = (0..=total_updates).map(dynamic_text).collect::<Vec<_>>();

    let mut cold_service = FontService::new();
    let cold_font = cold_service
        .load_font(FONT_BYTES)
        .expect("仓库固定 Noto CJK 字体应可加载");
    let cold_start = Instant::now();
    let (_, cold_checksum) = execute_update(&cold_service, cold_font, &texts[0], 7, &opts);
    let cold_ns = cold_start.elapsed().as_nanos();
    black_box(cold_checksum);

    let mut service = FontService::new();
    let font = service
        .load_font(FONT_BYTES)
        .expect("仓库固定 Noto CJK 字体应可加载");
    let mut checksum = 0usize;
    for (index, text) in texts.iter().take(CACHE_WARM_UPDATES).enumerate() {
        let cursor = (index * 7) % text.chars().count().max(1);
        let (_, value) = execute_update(&service, font, text, cursor, &opts);
        checksum ^= value;
    }

    let mut total_samples = [0_u128; TIMING_ROUNDS];
    let mut layout_samples = [0_u128; TIMING_ROUNDS];
    let mut cursor_samples = [0_u128; TIMING_ROUNDS];
    let mut glyph_samples = [0_u128; TIMING_ROUNDS];
    for round in 0..TIMING_ROUNDS {
        let start_index = CACHE_WARM_UPDATES + round * STEADY_UPDATES;
        let total_start = Instant::now();
        let mut phases = PhaseTimes::default();
        for (offset, text) in texts[start_index..start_index + STEADY_UPDATES]
            .iter()
            .enumerate()
        {
            let cursor = ((start_index + offset) * 7) % text.chars().count().max(1);
            let (sample, value) = execute_update(&service, font, black_box(text), cursor, &opts);
            phases.add(sample);
            checksum ^= value;
        }
        total_samples[round] = total_start.elapsed().as_nanos();
        layout_samples[round] = phases.layout.as_nanos();
        cursor_samples[round] = phases.cursor_hit.as_nanos();
        glyph_samples[round] = phases.glyph_prepare.as_nanos();
    }

    let measured_index = CACHE_WARM_UPDATES + TIMING_ROUNDS * STEADY_UPDATES;
    let measured_text = &texts[measured_index];
    let memory_before = service.memory_usage();
    let allocations = allocation_stats(|| {
        let cursor = measured_index * 7 % measured_text.chars().count().max(1);
        let (_, value) = execute_update(&service, font, black_box(measured_text), cursor, &opts);
        checksum ^= value;
    });
    let memory_after = service.memory_usage();

    eprintln!(
        "PROFILE dynamic_text cold_ns={cold_ns} total_ns_per_update={} layout_ns_per_update={} cursor_hit_ns_per_update={} glyph_prepare_ns_per_update={}",
        median(total_samples) / STEADY_UPDATES as u128,
        median(layout_samples) / STEADY_UPDATES as u128,
        median(cursor_samples) / STEADY_UPDATES as u128,
        median(glyph_samples) / STEADY_UPDATES as u128,
    );
    eprintln!(
        "PROFILE dynamic_text allocations={} allocated_bytes={} peak_live_delta={} final_live_delta={} service_memory_before={} service_memory_after={} checksum={}",
        allocations.count,
        allocations.bytes,
        allocations.peak_live_delta,
        allocations.final_live_delta,
        memory_before,
        memory_after,
        black_box(checksum),
    );
}
