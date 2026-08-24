//! 验证真实 Textarea 稳态重绘复用逐行布局、命中字形与显示列表存储。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::{Error, Rect};
use uix::draw::renderer::{FrameRenderInput, ScenePipeline};
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::resources::{FontService, ImageService};
use uix::draw::scene::ScenePaint;
use uix::draw::{
    DirtyRegion, FontHandle, GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, RenderOutcome,
    RenderTarget, Renderer, TextBackend, TextLayout,
};
use uix::prelude::Input;
use uix::ui::__private::WidgetTree;

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
            width: 1,
            height: 1,
            coverage: Arc::from([u8::MAX]),
            bearing_x: 0.0,
            bearing_y: -1.0,
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

fn frame_input<'a>(
    tree: &WidgetTree,
    dirty_region: DirtyRegion,
    font_service: &'a FontService,
    image_service: &'a ImageService,
    rendered_first: bool,
) -> FrameRenderInput<'a> {
    FrameRenderInput {
        rendered_first,
        dirty_region,
        tree_version: ScenePaint::tree_version(tree),
        scroll_move: None,
        font: FontHandle::new(0),
        font_service,
        image_service,
        debug_mode: false,
        hover_pos: None,
        metrics: None,
        debug_frame: None,
        invalidation_source: uix::draw::InvalidationSource::DirtyRegion,
    }
}

fn assert_presented(outcome: RenderOutcome) {
    assert!(matches!(
        outcome,
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
}

#[test]
fn warmed_textarea_frame_has_zero_allocations() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Input::textarea()
            .rows(4)
            .with_value("alpha\nbeta\ngamma\ndelta"),
    ));
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 240.0, 120.0));
    tree.layout();

    let font_service = FontService::new().with_text_backend(Box::new(FixedTextBackend));
    let image_service = ImageService::new();
    let mut renderer = Renderer::cpu();
    renderer
        .initialize(240, 120)
        .expect("CPU renderer 应初始化成功");
    let mut pipeline = ScenePipeline::new();

    let first_dirty = DirtyRegion::full();
    let first = pipeline.render_frame(
        &mut renderer,
        &tree,
        frame_input(&tree, first_dirty, &font_service, &image_service, false),
    );
    assert_presented(first.outcome);

    tree.invalidate_paint(root);
    let warm_dirty = ScenePaint::dirty_region(&tree);
    let warm = pipeline.render_frame(
        &mut renderer,
        &tree,
        frame_input(&tree, warm_dirty, &font_service, &image_service, true),
    );
    assert_presented(warm.outcome);

    tree.invalidate_paint(root);
    let measured_dirty = ScenePaint::dirty_region(&tree);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let measured = pipeline.render_frame(
        &mut renderer,
        &tree,
        frame_input(&tree, measured_dirty, &font_service, &image_service, true),
    );
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_presented(measured.outcome);
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("稳态 Textarea 帧堆申请次数: {allocations}");
    assert_eq!(allocations, 0, "预热后的 Textarea 重绘必须保持零堆申请");
}
