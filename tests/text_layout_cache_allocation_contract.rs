//! 验证文本布局缓存命中时不重复申请键、字形与行存储。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::Error;
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::{
    FontHandle, FontService, GlyphRaster, HAlign, LineInfo, LineMetrics, PositionedGlyph,
    TextBackend, TextLayout, VAlign,
};

struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 指针与 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Debug)]
struct FixedTextBackend;

impl TextBackend for FixedTextBackend {
    fn load_font(&mut self, _data: &[u8]) -> Result<FontHandle, Error> {
        Ok(FontHandle::new(0))
    }

    fn unload_font(&mut self, _handle: &FontHandle) {}

    fn is_valid(&self, _handle: &FontHandle) -> bool {
        true
    }

    fn has_glyph(&self, _font: &FontHandle, _ch: char) -> bool {
        true
    }

    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout {
        let glyphs = text
            .chars()
            .enumerate()
            .map(|(index, _)| PositionedGlyph {
                x: index as f32 * 8.0,
                y: 10.0,
                width: 8.0,
                height: 12.0,
                glyph_id: index as u32 + 1,
                char_index: index,
                char_end: index + 1,
                bidi_level: 0,
                font: *font,
            })
            .collect::<Vec<_>>();
        let width = glyphs.len() as f32 * 8.0;
        TextLayout {
            lines: vec![LineInfo {
                y: 0.0,
                height: opts.line_height,
                width,
                start_char: 0,
                end_char: glyphs.len(),
                glyph_start: 0,
                glyph_count: glyphs.len(),
            }],
            glyphs,
            width,
            height: opts.line_height,
        }
    }

    fn rasterize_glyph(&self, _font: &FontHandle, _glyph_id: u32, _pixel_size: f32) -> GlyphRaster {
        GlyphRaster {
            width: 0,
            height: 0,
            coverage: Arc::from([]),
            bearing_x: 0.0,
            bearing_y: 0.0,
            outline_mesh: None,
        }
    }

    fn horizontal_line_metrics(&self, _font: &FontHandle, _pixel_size: f32) -> Option<LineMetrics> {
        Some(LineMetrics {
            ascent: 10.0,
            descent: 2.0,
            new_line_size: 12.0,
        })
    }
}

#[test]
fn warmed_shared_layout_cache_hit_has_zero_allocations() {
    let service = FontService::new().with_text_backend(Box::new(FixedTextBackend));
    let font = FontHandle::new(0);
    let options = TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 16.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 14.0,
    };

    let empty = service.layout_text_shared(&font, "", &options);
    let empty_again = service.layout_text_shared(&font, "", &options);
    assert!(Arc::ptr_eq(&empty, &empty_again));

    let warm = service.layout_text_shared(&font, "steady", &options);
    assert_eq!(warm.glyphs.len(), 6);
    let mut owned = service.layout_text(&font, "steady", &options);
    owned.glyphs[0].x = 999.0;
    assert_ne!(owned.glyphs[0].x, warm.glyphs[0].x);
    let mut centered_options = options.clone();
    centered_options.max_width = 100.0;
    centered_options.h_align = HAlign::Center;
    let centered = service.layout_text_shared(&font, "steady", &centered_options);
    let expected_bytes = "steady"
        .len()
        .saturating_add(
            (warm.glyphs.len() + centered.glyphs.len())
                .saturating_mul(std::mem::size_of::<PositionedGlyph>()),
        )
        .saturating_add(
            (warm.lines.len() + centered.lines.len())
                .saturating_mul(std::mem::size_of::<LineInfo>()),
        );
    assert_eq!(
        service.memory_usage(),
        expected_bytes,
        "相同文本的多种布局键必须只保留一份文本字节"
    );

    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let measured = service.layout_text_shared(&font, "steady", &options);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_eq!(measured.glyphs.len(), warm.glyphs.len());
    assert_eq!(measured.lines.len(), warm.lines.len());
    assert_eq!(measured.width, warm.width);
    assert_eq!(measured.height, warm.height);
    assert!(Arc::ptr_eq(&measured, &warm));
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("文本布局缓存命中堆申请次数: {allocations}");
    assert_eq!(allocations, 0, "共享缓存命中不得重复申请键、字形或行存储");

    // 容量淘汰后必须同时释放文本驻留身份，并允许同一文本重新进入缓存。
    let eviction_service = FontService::new().with_text_backend(Box::new(FixedTextBackend));
    let first = eviction_service.layout_text_shared(&font, "entry-0", &options);
    let mut latest_text = String::new();
    let mut latest = Arc::clone(&first);
    for index in 1..=1024 {
        latest_text = format!("entry-{index}");
        latest = eviction_service.layout_text_shared(&font, &latest_text, &options);
    }
    let latest_hit = eviction_service.layout_text_shared(&font, &latest_text, &options);
    assert!(Arc::ptr_eq(&latest, &latest_hit));
    let first_recomputed = eviction_service.layout_text_shared(&font, "entry-0", &options);
    assert!(!Arc::ptr_eq(&first, &first_recomputed));
}
