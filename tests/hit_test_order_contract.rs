//! 验证公开运行时命中行为在内部排序工作区复用后保持不变。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use uix::prelude::{Label, Point, Rect};
use uix::ui::__private::WidgetTree;

// 只在测试显式开启的窄区间统计当前进程堆申请。
struct CountingAllocator;

// 标记是否正在测量预热后的命中调用。
static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
// 保存测量区间内 alloc/alloc_zeroed/realloc 的总次数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

// 使用系统分配器执行真实申请，同时提供无侵入计数。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 指针和 Layout 来自同一系统分配器。
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

// 构造两个完全重叠、声明顺序稳定的普通子节点。
fn overlapping_children() -> (WidgetTree, uix::prelude::WidgetId, uix::prelude::WidgetId) {
    // 创建独立窗口树。
    let mut tree = WidgetTree::new();
    // 根节点拥有测试兄弟。
    let root = tree.set_root(Box::new(Label::new("root")));
    // 根几何覆盖目标坐标。
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 100.0, 100.0));
    // 按稳定声明顺序添加两个兄弟。
    let first = tree.add_child(root, Box::new(Label::new("first")));
    let second = tree.add_child(root, Box::new(Label::new("second")));
    // 两个兄弟使用相同命中矩形。
    for child in [first, second] {
        tree.set_frame_dirty(child, Rect::new(0.0, 0.0, 80.0, 80.0));
    }
    // 返回树与兄弟身份。
    (tree, first, second)
}

#[test]
fn hit_test_preserves_order_and_avoids_warmed_heap_allocations() {
    // 建立完全重叠的兄弟节点。
    let (mut tree, first, second) = overlapping_children();
    // 同 z-index 时后声明兄弟位于视觉上层。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(second));
    // 显式提高先声明兄弟的 z-index。
    tree.set_z_index(first, 10);
    // 更高 z-index 必须覆盖声明顺序。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(first));
    // 清零计数并只测量已经预热容量的同一命中调用。
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let repeated_hit = tree.hit_test(Point::new(10.0, 10.0));
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    // 先确认行为，再确认排序与视觉路径都没有再次申请堆内存。
    assert_eq!(repeated_hit, Some(first));
    assert_eq!(allocations, 0, "预热后的命中调用不应再申请堆内存");
}
