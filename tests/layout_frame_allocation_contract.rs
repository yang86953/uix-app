//! 验证真实嵌套组件树在预热后的重复布局中复用布局工作区。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::Rect;
use uix::prelude::{Container, Space};
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

fn nested_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(Container::new()));
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(20.0 + leaf_index as f32)
                        .height(12.0 + branch_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

#[test]
fn warmed_nested_layout_reuses_heap_storage() {
    let (mut tree, root) = nested_layout_tree();

    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    tree.layout();
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 321.0, 200.0));
    tree.layout();

    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    tree.layout();
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("稳态嵌套布局堆申请次数: {allocations}");
    assert_eq!(allocations, 0, "预热后的嵌套布局必须复用全部堆存储");
}
