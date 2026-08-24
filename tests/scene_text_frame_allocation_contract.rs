//! 验证稳态脏文字选区帧复用布局、几何、字形、显示列表与呈现存储。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::{Error, Point, Rect, WidgetId};
use uix::draw::painting::PaintContext;
use uix::draw::renderer::{FrameRenderInput, ScenePipeline};
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::resources::{FontService, ImageService};
use uix::draw::scene::{NodeId, ScenePaint};
use uix::draw::{
    Color, DirtyRegion, FontHandle, GlyphRaster, LineInfo, LineMetrics, PositionedGlyph,
    RenderOutcome, RenderTarget, Renderer, TextBackend, TextLayout,
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

const ROOT: NodeId = WidgetId::new(1);

struct TextScene;

impl ScenePaint for TextScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT)
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::area(Rect::new(2.0, 3.0, 56.0, 20.0))
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, _id: NodeId) -> Rect {
        Rect::new(0.0, 0.0, 64.0, 48.0)
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        true
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

    fn paint(&self, _id: NodeId, _frame: Rect, ctx: &mut PaintContext<'_>) {
        ctx.fill_text_selection(
            "steady",
            14.0,
            Point::new(2.0, 3.0),
            1,
            5,
            Color::from_rgba(64, 96, 160, 128),
        );
        ctx.draw_text("steady", Point::new(2.0, 3.0), Color::blue(), 14.0);
    }
}

fn frame_input<'a>(
    dirty_region: DirtyRegion,
    font_service: &'a FontService,
    image_service: &'a ImageService,
    rendered_first: bool,
) -> FrameRenderInput<'a> {
    FrameRenderInput {
        rendered_first,
        dirty_region,
        tree_version: 1,
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

#[test]
fn warmed_dirty_text_selection_frame_has_zero_allocations() {
    let scene = TextScene;
    let font_service = FontService::new().with_text_backend(Box::new(FixedTextBackend));
    let image_service = ImageService::new();
    let mut renderer = Renderer::cpu();
    renderer
        .initialize(64, 48)
        .expect("CPU renderer 应初始化成功");
    let mut pipeline = ScenePipeline::new();

    let first = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(DirtyRegion::full(), &font_service, &image_service, false),
    );
    assert!(matches!(
        first.outcome,
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
    let warm = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(scene.dirty_region(), &font_service, &image_service, true),
    );
    assert!(matches!(
        warm.outcome,
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));

    let measured_dirty_region = scene.dirty_region();
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let measured = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(measured_dirty_region, &font_service, &image_service, true),
    );
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert!(matches!(
        measured.outcome,
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("稳态脏文字选区帧堆申请次数: {allocations}");
    assert_eq!(allocations, 0, "预热后的脏文字选区帧必须保持零堆申请");
}
