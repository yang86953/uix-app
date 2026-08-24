//! 验证真实嵌套组件树在预热后的重复布局中复用布局工作区。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::Rect;
use uix::prelude::{Card, Container, Grid, GridTrack, Space};
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

fn wrapped_container_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let mut branch = Container::new().size(100.0, 30.0);
        branch.style.flex_wrap = true;
        let branch = tree.add_child(root, Box::new(branch));
        for leaf_index in 0..5 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn wrapped_space_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(
            root,
            Box::new(Space::new().width(100.0).height(30.0).wrap(true)),
        );
        for leaf_index in 0..5 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn card_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(Card::new().size(100.0, 60.0)));
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(
            root,
            Box::new(
                Grid::new()
                    .size(100.0, 60.0)
                    .columns(vec![GridTrack::Auto, GridTrack::Fr(1.0)])
                    .rows(vec![GridTrack::Auto, GridTrack::Fr(1.0)]),
            ),
        );
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn responsive_grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..1 {
        let branch = tree.add_child(root, Box::new(Grid::responsive().size(300.0, 180.0)));
        for leaf_index in 0..40 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn spanning_grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let branch = tree.add_child(
        root,
        Box::new(
            Grid::new()
                .size(300.0, 180.0)
                .columns(vec![GridTrack::Auto; 8]),
        ),
    );
    for index in 0..40 {
        let mut leaf = Container::new().size(18.0 + index as f32, 10.0);
        leaf.style.grid_column_span = 2 + (index % 3) as u32;
        tree.add_child(branch, Box::new(leaf));
    }
    (tree, root)
}

fn warmed_layout_allocations(mut tree: WidgetTree, root: uix::ui::WidgetId) -> usize {
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    tree.layout();
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 321.0, 200.0));
    tree.layout();

    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    tree.layout();
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    ALLOCATION_COUNT.load(Ordering::Relaxed)
}

#[test]
fn warmed_nested_layout_reuses_heap_storage() {
    let scenarios = [
        ("Container", nested_layout_tree()),
        ("Container wrap", wrapped_container_tree()),
        ("Space wrap", wrapped_space_tree()),
        ("Card", card_layout_tree()),
        ("Grid", grid_layout_tree()),
        ("Responsive Grid", responsive_grid_layout_tree()),
        ("Spanning Grid", spanning_grid_layout_tree()),
    ];
    for (name, (tree, root)) in scenarios {
        let allocations = warmed_layout_allocations(tree, root);
        eprintln!("稳态 {name} 布局堆申请次数: {allocations}");
        assert_eq!(allocations, 0, "预热后的 {name} 布局必须复用全部堆存储");
    }
}
