//! 临时剖析代表性组件树的命中、路由与处理器分发成本。

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use uix::platform::windowing::{KeyCode, KeyMod, MouseButton};
use uix::prelude::{Button, Container, EventResult, Point, Rect, SystemEvent};
use uix::ui::__private::WidgetTree;

const ROWS: usize = 8;
const COLUMNS: usize = 8;
const WARMUP_INTERACTIONS: usize = 1_024;
const STEADY_INTERACTIONS: usize = 2_048;
const TIMING_ROUNDS: usize = 7;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

fn update_peak(candidate: usize) {
    let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    while candidate > peak {
        match PEAK_LIVE_BYTES.compare_exchange_weak(
            peak,
            candidate,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    update_peak(live);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            let _ = LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
                Some(live.saturating_sub(layout.size()))
            });
        }
        // SAFETY: 指针与 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if MEASURING.load(Ordering::Relaxed) && !next.is_null() {
            record_allocation(new_size);
            let _ = LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
                Some(live.saturating_sub(layout.size()))
            });
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct AllocationStats {
    allocations: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
    final_live_bytes: usize,
}

fn measure_allocations(operation: impl FnOnce()) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    operation();
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

struct InteractionTree {
    tree: WidgetTree,
    points: Vec<Point>,
    targets: Vec<uix::prelude::WidgetId>,
    semantic_calls: Rc<Cell<usize>>,
}

fn interaction_tree() -> InteractionTree {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new()));
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 800.0, 480.0));

    let semantic_calls = Rc::new(Cell::new(0));
    let root_calls = Rc::clone(&semantic_calls);
    tree.handler_table().on_click(root, move |_| {
        root_calls.set(root_calls.get() + 1);
    });

    let mut points = Vec::with_capacity(ROWS * COLUMNS);
    let mut targets = Vec::with_capacity(ROWS * COLUMNS);
    for row_index in 0..ROWS {
        let row = tree.add_child(root, Box::new(Container::new()));
        tree.set_frame_dirty(row, Rect::new(0.0, row_index as f32 * 56.0, 800.0, 48.0));
        let row_calls = Rc::clone(&semantic_calls);
        tree.handler_table().on_click(row, move |_| {
            row_calls.set(row_calls.get() + 1);
        });
        for column_index in 0..COLUMNS {
            let button = tree.add_child(
                row,
                Box::new(Button::new(format!("操作 {row_index}-{column_index}"))),
            );
            tree.set_frame_dirty(
                button,
                Rect::new(
                    column_index as f32 * 96.0,
                    row_index as f32 * 56.0,
                    88.0,
                    40.0,
                ),
            );
            let button_calls = Rc::clone(&semantic_calls);
            tree.handler_table().on_click(button, move |_| {
                button_calls.set(button_calls.get() + 1);
            });
            points.push(Point::new(
                column_index as f32 * 96.0 + 44.0,
                row_index as f32 * 56.0 + 20.0,
            ));
            targets.push(button);
        }
    }

    for (point, target) in points.iter().zip(&targets) {
        assert_eq!(tree.hit_test(*point), Some(*target));
    }
    InteractionTree {
        tree,
        points,
        targets,
        semantic_calls,
    }
}

fn dispatch_pointer_move(tree: &mut WidgetTree, point: Point) {
    let result = tree.dispatch_event(&SystemEvent::PointerMove {
        pos: point,
        mods: KeyMod::NONE,
    });
    black_box(result);
}

fn dispatch_pointer_click(tree: &mut WidgetTree, point: Point) {
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: point,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: point,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

fn dispatch_keyboard_click(tree: &mut WidgetTree) {
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

fn run_hit_tests(data: &InteractionTree, iterations: usize) {
    for index in 0..iterations {
        let point = data.points[index % data.points.len()];
        black_box(data.tree.hit_test(black_box(point)));
    }
}

fn run_pointer_moves(data: &mut InteractionTree, iterations: usize) {
    for index in 0..iterations {
        let point = data.points[index % data.points.len()];
        dispatch_pointer_move(&mut data.tree, black_box(point));
    }
}

fn run_pointer_clicks(data: &mut InteractionTree, iterations: usize) {
    for index in 0..iterations {
        let point = data.points[index % data.points.len()];
        dispatch_pointer_click(&mut data.tree, black_box(point));
    }
}

fn run_keyboard_clicks(data: &mut InteractionTree, iterations: usize) {
    for _ in 0..iterations {
        dispatch_keyboard_click(&mut data.tree);
    }
}

fn run_mixed(data: &mut InteractionTree, interactions: usize) {
    for index in 0..interactions {
        let point = data.points[index % data.points.len()];
        dispatch_pointer_move(&mut data.tree, black_box(point));
        dispatch_pointer_click(&mut data.tree, black_box(point));
        dispatch_keyboard_click(&mut data.tree);
    }
}

fn median_ns(mut operation: impl FnMut()) -> u128 {
    let mut samples = [0_u128; TIMING_ROUNDS];
    for sample in &mut samples {
        let started = Instant::now();
        operation();
        *sample = started.elapsed().as_nanos();
    }
    samples.sort_unstable();
    samples[TIMING_ROUNDS / 2]
}

#[test]
fn profile_hierarchical_pointer_keyboard_interaction() {
    let mut data = interaction_tree();
    let calls_before_cold = data.semantic_calls.get();
    let cold_started = Instant::now();
    let cold_allocations = measure_allocations(|| run_mixed(&mut data, 1));
    let cold_ns = cold_started.elapsed().as_nanos();
    assert_eq!(data.semantic_calls.get() - calls_before_cold, 6);

    run_mixed(&mut data, WARMUP_INTERACTIONS);
    let hit_test_allocations = measure_allocations(|| run_hit_tests(&data, STEADY_INTERACTIONS));
    let pointer_move_allocations =
        measure_allocations(|| run_pointer_moves(&mut data, STEADY_INTERACTIONS));
    let pointer_click_allocations =
        measure_allocations(|| run_pointer_clicks(&mut data, STEADY_INTERACTIONS));
    let keyboard_click_allocations =
        measure_allocations(|| run_keyboard_clicks(&mut data, STEADY_INTERACTIONS));
    let calls_before_steady = data.semantic_calls.get();
    let steady_allocations = measure_allocations(|| run_mixed(&mut data, STEADY_INTERACTIONS));
    assert_eq!(
        data.semantic_calls.get() - calls_before_steady,
        STEADY_INTERACTIONS * 6
    );

    let hit_test_ns = median_ns(|| run_hit_tests(&data, STEADY_INTERACTIONS));
    let pointer_move_ns = median_ns(|| run_pointer_moves(&mut data, STEADY_INTERACTIONS));
    let pointer_click_ns = median_ns(|| run_pointer_clicks(&mut data, STEADY_INTERACTIONS));
    let keyboard_click_ns = median_ns(|| run_keyboard_clicks(&mut data, STEADY_INTERACTIONS));
    let mixed_ns = median_ns(|| run_mixed(&mut data, STEADY_INTERACTIONS));
    let mixed_events = STEADY_INTERACTIONS * 5;

    eprintln!(
        "PROFILE ui_interaction cold_ns={cold_ns} cold_allocations={} cold_bytes={} \
         steady_interactions={STEADY_INTERACTIONS} steady_events={mixed_events} \
         steady_allocations={} steady_bytes={} steady_peak_live={} steady_final_live={} \
         hit_test_allocations={} hit_test_bytes={} pointer_move_allocations={} \
         pointer_move_bytes={} pointer_click_allocations={} pointer_click_bytes={} \
         keyboard_click_allocations={} keyboard_click_bytes={} \
         hit_test_ns={} hit_test_ns_per_event={} pointer_move_ns_per_event={} \
         pointer_click_ns_per_event={} keyboard_click_ns_per_event={} mixed_ns={} \
         mixed_ns_per_event={} semantic_calls={}",
        cold_allocations.allocations,
        cold_allocations.allocated_bytes,
        steady_allocations.allocations,
        steady_allocations.allocated_bytes,
        steady_allocations.peak_live_bytes,
        steady_allocations.final_live_bytes,
        hit_test_allocations.allocations,
        hit_test_allocations.allocated_bytes,
        pointer_move_allocations.allocations,
        pointer_move_allocations.allocated_bytes,
        pointer_click_allocations.allocations,
        pointer_click_allocations.allocated_bytes,
        keyboard_click_allocations.allocations,
        keyboard_click_allocations.allocated_bytes,
        hit_test_ns,
        hit_test_ns / STEADY_INTERACTIONS as u128,
        pointer_move_ns / STEADY_INTERACTIONS as u128,
        pointer_click_ns / (STEADY_INTERACTIONS * 2) as u128,
        keyboard_click_ns / (STEADY_INTERACTIONS * 2) as u128,
        mixed_ns,
        mixed_ns / mixed_events as u128,
        data.semantic_calls.get(),
    );

    // 稳态仍包含语义路径等既有临时对象；这里只锁定本批消除的事件批次缓冲扩容。
    assert!(
        steady_allocations.allocations <= STEADY_INTERACTIONS * 36,
        "每个混合交互不得恢复已消除的事件批次缓冲分配"
    );
    assert!(
        steady_allocations.allocated_bytes <= STEADY_INTERACTIONS * 876,
        "每个混合交互的稳态分配字节不得回退"
    );
    assert!(
        steady_allocations.peak_live_bytes <= 96,
        "事件批次缓冲必须保留容量，而非每轮重新建立峰值"
    );
    assert_eq!(steady_allocations.final_live_bytes, 0);
    assert_eq!(hit_test_allocations.final_live_bytes, 0);
    assert_eq!(pointer_move_allocations.final_live_bytes, 0);
    assert_eq!(pointer_click_allocations.final_live_bytes, 0);
    assert_eq!(keyboard_click_allocations.final_live_bytes, 0);
    assert_eq!(data.targets.len(), ROWS * COLUMNS);
}
