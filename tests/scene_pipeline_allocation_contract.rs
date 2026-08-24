//! 验证稳态局部帧复用脏区与裁剪状态，保持零堆申请。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::{Point, Rect, WidgetId};
use uix::draw::painting::PaintContext;
use uix::draw::renderer::{FrameRenderInput, ScenePipeline};
use uix::draw::resources::{FontService, ImageService};
use uix::draw::scene::{NodeId, ScenePaint};
use uix::draw::{Color, DirtyRegion, FontHandle, RenderOutcome, RenderTarget, Renderer};

// 只统计显式测量区间内的堆申请。
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

const ROOT: NodeId = WidgetId::new(1);

// 单节点空绘制场景隔离场景管线自身的几何与所有权成本。
struct EmptyScene;

impl ScenePaint for EmptyScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT)
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::area(Rect::new(2.0, 3.0, 8.0, 9.0))
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, _id: NodeId) -> Rect {
        Rect::new(0.0, 0.0, 64.0, 48.0)
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        false
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
        // 使用可直接编码的原生矩形，确保稳态帧真实经过命令缓冲。
        ctx.fill_rect(Rect::new(2.0, 3.0, 8.0, 9.0), Color::blue(), None);
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
fn warmed_partial_frame_has_bounded_geometry_allocations() {
    let scene = EmptyScene;
    let font_service = FontService::default();
    let image_service = ImageService::new();
    let mut renderer = Renderer::cpu();
    renderer
        .initialize(64, 48)
        .expect("CPU renderer 应初始化成功");
    let mut pipeline = ScenePipeline::new();

    // 首帧建立图层、渲染对象与录制器容量。
    let first_dirty = DirtyRegion::full();
    let first = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(first_dirty, &font_service, &image_service, false),
    );
    assert!(
        matches!(
            first.outcome,
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
        ),
        "首帧必须完成呈现"
    );
    // 再执行一次局部帧，排除一次性容量增长。
    let warm_dirty = scene.dirty_region();
    let warm = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(warm_dirty, &font_service, &image_service, true),
    );
    assert!(
        matches!(
            warm.outcome,
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
        ),
        "预热局部帧必须完成呈现"
    );

    let measured_dirty = scene.dirty_region();
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let measured = pipeline.render_frame(
        &mut renderer,
        &scene,
        frame_input(measured_dirty, &font_service, &image_service, true),
    );
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("稳态局部帧堆申请次数: {allocations}");
    assert!(
        matches!(
            measured.outcome,
            RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
        ),
        "测量局部帧必须完成呈现"
    );
    assert_eq!(
        allocations, 0,
        "预热后的单矩形局部帧必须复用脏区、固定裁剪与光栅状态，保持零堆申请"
    );
}
