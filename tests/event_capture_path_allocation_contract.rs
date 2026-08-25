//! 验证深层组件树的滚轮捕获路径不会在稳态重复申请临时祖先向量。

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::hint::black_box;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use uix::prelude::{EventResult, Label, PaintContext, Point, Rect, SystemEvent};
use uix::ui::__private::WidgetTree;
use uix::widget;

// 使用 512 层真实树覆盖生产中的深层声明结构。
const TREE_DEPTH: usize = 512;
// 每轮执行足够多事件，降低时钟粒度与调度抖动的相对影响。
const ITERATIONS: usize = 2_000;
// 中位数过滤偶发调度抖动，同时保持测试运行时间有界。
const TIMING_ROUNDS: usize = 9;

// 让命中测试保持常量工作，并在捕获路径根部消费滚轮事件。
widget! {
    struct WheelCaptureRoot {}

    @new -> Self {
        Self {}
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if matches!(event, SystemEvent::Wheel { .. }) {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    hit_test_children => (&self) -> bool {
        false
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
}

// 记录 Wheel 捕获与冒泡的节点顺序，并允许指定节点终止传播。
widget! {
    struct WheelOrderProbe {
        order: usize,
        handles: bool,
        blocks_child_hit_test: bool,
        calls: Rc<RefCell<Vec<usize>>>,
    }

    @new -> Self {
        Self {
            order: 0,
            handles: false,
            blocks_child_hit_test: false,
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if matches!(event, SystemEvent::Wheel { .. }) {
            self.calls.borrow_mut().push(self.order);
            if self.handles {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        } else {
            EventResult::NotHandled
        }
    }

    hit_test_children => (&self) -> bool {
        !self.blocks_child_hit_test
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
}

impl WheelOrderProbe {
    // 构造共享同一调用日志的可配置节点。
    fn recording(
        order: usize,
        handles: bool,
        blocks_child_hit_test: bool,
        calls: Rc<RefCell<Vec<usize>>>,
    ) -> Self {
        Self {
            order,
            handles,
            blocks_child_hit_test,
            calls,
        }
    }
}

// 记录窄测量区间内的系统堆活动。
struct CountingAllocator;

// 只在显式测量区间开启统计，避免树构建污染结果。
static MEASURING: AtomicBool = AtomicBool::new(false);
// 保存 alloc、alloc_zeroed 与 realloc 的调用总数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 保存每次申请或扩容请求的新尺寸之和。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
// 保存测量区间内仍存活的字节数。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
// 保存测量区间内同时存活字节数的峰值。
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 登记一次真实系统堆申请。
fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

// 保留系统分配器行为，只在测量窗口增加原子计数。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: 指针和 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                let added = new_size - layout.size();
                let live = LIVE_BYTES.fetch_add(added, Ordering::Relaxed) + added;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        // SAFETY: 指针和旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// 保存一次精确分配测量结果。
#[derive(Debug, Clone, Copy)]
struct AllocationStats {
    count: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
    final_live_bytes: usize,
}

// 构造深层单子节点链，并把 hover 目标直接设置为最深叶子。
fn deep_wheel_tree() -> WidgetTree {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(WheelCaptureRoot::new()));
    let mut parent = root;
    for depth in 1..TREE_DEPTH {
        parent = tree.add_child(parent, Box::new(Label::new(depth.to_string())));
    }
    // 根拒绝子节点命中，因此 Wheel 通过既有 hovered fallback 取得深层目标。
    tree.managers_mut()
        .interaction
        .set_hovered_widget(Some(parent));
    tree
}

// 对同一树执行固定数量的真实公开事件分发。
fn dispatch_batch(tree: &mut WidgetTree, event: &SystemEvent) -> EventResult {
    let mut result = EventResult::NotHandled;
    for _ in 0..ITERATIONS {
        result = black_box(tree.dispatch_event(black_box(event)));
    }
    result
}

// 在单独区间精确统计稳态堆申请。
fn measure_allocations(tree: &mut WidgetTree, event: &SystemEvent) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    let result = dispatch_batch(tree, event);
    MEASURING.store(false, Ordering::Release);
    assert_eq!(result, EventResult::Handled);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

// 取九轮同场景耗时中位数。
fn median_batch_time_ns(tree: &mut WidgetTree, event: &SystemEvent) -> u128 {
    let mut samples = [0_u128; TIMING_ROUNDS];
    for sample in &mut samples {
        let started = Instant::now();
        let result = dispatch_batch(tree, event);
        *sample = started.elapsed().as_nanos();
        assert_eq!(result, EventResult::Handled);
    }
    samples.sort_unstable();
    samples[TIMING_ROUNDS / 2]
}

// 构造四层事件探针树，根节点关闭子命中并由 hover fallback 指向最深目标。
fn wheel_order_tree(handles_at: Option<usize>) -> (WidgetTree, Rc<RefCell<Vec<usize>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(WheelOrderProbe::recording(
        0,
        handles_at == Some(0),
        true,
        Rc::clone(&calls),
    )));
    let mut parent = root;
    for order in 1..4 {
        parent = tree.add_child(
            parent,
            Box::new(WheelOrderProbe::recording(
                order,
                handles_at == Some(order),
                false,
                Rc::clone(&calls),
            )),
        );
    }
    tree.managers_mut()
        .interaction
        .set_hovered_widget(Some(parent));
    (tree, calls)
}

fn assert_wheel_capture_and_bubble_order() {
    let event = SystemEvent::Wheel {
        pos: Point::new(10_000.0, 10_000.0),
        delta: Point::new(0.0, 1.0),
    };
    // 无处理器时先捕获 root→target.parent，再冒泡 target→root。
    let (mut bubbling_tree, bubbling_calls) = wheel_order_tree(None);
    assert_eq!(
        bubbling_tree.dispatch_event(&event),
        EventResult::NotHandled
    );
    assert_eq!(&*bubbling_calls.borrow(), &[0, 1, 2, 3, 2, 1, 0]);

    // 捕获阶段处理后立即停止，目标与冒泡阶段均不得收到事件。
    let (mut handled_tree, handled_calls) = wheel_order_tree(Some(2));
    assert_eq!(handled_tree.dispatch_event(&event), EventResult::Handled);
    assert_eq!(&*handled_calls.borrow(), &[0, 1, 2]);
}

#[test]
fn warmed_deep_wheel_capture_path_is_allocation_free() {
    // 先锁定捕获、冒泡、目标排除与短路语义，再执行同一进程的精确分配测量。
    assert_wheel_capture_and_bubble_order();
    let mut tree = deep_wheel_tree();
    let event = SystemEvent::Wheel {
        // 根与全部子节点都保持默认零 frame，使 hit-test 常量失败并走 hover fallback。
        pos: Point::new(10_000.0, 10_000.0),
        delta: Point::new(0.0, 1.0),
    };
    // 首次事件允许运行时工作区预热到生产深度。
    assert_eq!(tree.dispatch_event(&event), EventResult::Handled);
    let allocations = measure_allocations(&mut tree, &event);
    let batch_ns = median_batch_time_ns(&mut tree, &event);
    eprintln!(
        "PROFILE deep_wheel_capture_path: depth={TREE_DEPTH} iterations={ITERATIONS} \
         batch_ns={batch_ns} per_event_ns={} allocations={} allocated_bytes={} \
         peak_live_bytes={} final_live_bytes={}",
        batch_ns / ITERATIONS as u128,
        allocations.count,
        allocations.allocated_bytes,
        allocations.peak_live_bytes,
        allocations.final_live_bytes,
    );
    assert_eq!(allocations.count, 0, "预热后的捕获路径不得再次申请堆内存");
    assert_eq!(allocations.allocated_bytes, 0);
    assert_eq!(allocations.peak_live_bytes, 0);
    assert_eq!(allocations.final_live_bytes, 0);
}
