//! 分层渲染的浮层变换与裁剪坐标契约。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use uix::core::{DirtyRegion, Point, Rect, WidgetId};
use uix::draw::Transform;
use uix::draw::painting::PaintContext;
use uix::draw::scene::{NodeId, ScenePaint, node_visual_rect, visible_viewport_rect};

/// 只在测试显式开启的窄区间统计场景坐标查询的堆申请。
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
        // SAFETY: 指针和 Layout 均来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 指针和旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const ROOT_NODE: NodeId = WidgetId::new(1);
const OVERLAY_NODE: NodeId = WidgetId::new(2);
const CHILD_NODE: NodeId = WidgetId::new(3);

/// 模拟滚动容器内被提升到根画布、且自身裁剪后代的浮层。
struct ScrolledClippedOverlayScene;

impl ScenePaint for ScrolledClippedOverlayScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT_NODE)
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        match id {
            OVERLAY_NODE => Rect::new(20.0, 100.0, 100.0, 60.0),
            CHILD_NODE => Rect::new(30.0, 110.0, 40.0, 20.0),
            _ => Rect::new(0.0, 0.0, 200.0, 120.0),
        }
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        false
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        i32::from(id == OVERLAY_NODE) * 900
    }

    fn node_transform(&self, id: NodeId) -> Transform {
        match id {
            // 视觉路径必须在浮层根截断，不能继承普通根节点缩放。
            ROOT_NODE => Transform::scale(3.0, 3.0),
            // 子节点缩放与浮层平移不可交换，用于锁定矩阵累计顺序。
            CHILD_NODE => Transform::scale(2.0, 1.0),
            _ => Transform::identity(),
        }
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static ROOT_CHILDREN: [NodeId; 1] = [OVERLAY_NODE];
        static OVERLAY_CHILDREN: [NodeId; 1] = [CHILD_NODE];
        match id {
            ROOT_NODE => &ROOT_CHILDREN,
            OVERLAY_NODE => &OVERLAY_CHILDREN,
            _ => &[],
        }
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        id == OVERLAY_NODE
    }

    fn node_overlay_transform(&self, id: NodeId) -> Transform {
        if id == OVERLAY_NODE {
            Transform::translate(0.0, -80.0)
        } else {
            Transform::identity()
        }
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        (id == OVERLAY_NODE).then_some(frame)
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        match id {
            ROOT_NODE => Some((0.0, 80.0)),
            // 浮层自身仍可作为滚动祖先影响其普通后代。
            OVERLAY_NODE => Some((5.0, 0.0)),
            _ => None,
        }
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

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        match id {
            OVERLAY_NODE => Some(ROOT_NODE),
            CHILD_NODE => Some(OVERLAY_NODE),
            _ => None,
        }
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

#[test]
fn overlay_descendant_projection_preserves_clipping_without_heap_allocations() {
    let scene = ScrolledClippedOverlayScene;
    let expected_rect = Rect::new(55.0, 30.0, 80.0, 20.0);
    let expected_visible = Rect::new(55.0, 30.0, 65.0, 20.0);

    assert_eq!(
        node_visual_rect(&scene, CHILD_NODE, scene.node_frame(CHILD_NODE)),
        expected_rect
    );
    assert_eq!(
        visible_viewport_rect(&scene, CHILD_NODE),
        Some(expected_visible)
    );

    // 预热测试进程后，只测量相同坐标与裁剪查询本身。
    let _ = node_visual_rect(&scene, CHILD_NODE, scene.node_frame(CHILD_NODE));
    let _ = visible_viewport_rect(&scene, CHILD_NODE);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let repeated_rect = node_visual_rect(&scene, CHILD_NODE, scene.node_frame(CHILD_NODE));
    let repeated_visible = visible_viewport_rect(&scene, CHILD_NODE);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_eq!(repeated_rect, expected_rect);
    assert_eq!(repeated_visible, Some(expected_visible));
    assert_eq!(
        ALLOCATION_COUNT.load(Ordering::Relaxed),
        0,
        "预热后的场景坐标与裁剪查询不应申请堆内存"
    );
}
