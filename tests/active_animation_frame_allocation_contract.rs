//! 测量大量真实活跃声明式动画源的稳态帧采样、插值与失效发布成本。

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uix::prelude::{Animated, Easing, Label, State, View, ViewNode};
use uix::ui::__private::{
    WidgetTree, animated_source_ids_for_test, build_view_tree_for_test,
    update_animated_sources_at_for_test, update_animated_sources_geometric_for_test,
    update_animated_sources_into_for_test,
};

// 足够大的来源集合让逐源推进成本明显高于计时噪声，同时保持测试运行有界。
const SOURCE_COUNT: usize = 8_192;
// 每批连续推进多帧，覆盖真实长期活跃而非首次登记。
const FRAMES_PER_BATCH: usize = 96;
// 使用奇数轮中位数过滤偶发调度抖动。
const TIMING_ROUNDS: usize = 9;

// 记录显式测量窗口内的系统堆活动。
struct CountingAllocator;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: 指针与 Layout 来自同一系统分配器。
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
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct AllocationStats {
    count: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
    final_live_bytes: usize,
}

// 由真实根捕获读取全部动画值，但只建立一个运行时节点以隔离逐源成本。
struct ActiveAnimationView {
    sources: Arc<Vec<Animated<f32>>>,
}

impl View for ActiveAnimationView {
    fn build(self) -> ViewNode {
        let mut checksum = 0.0_f32;
        for source in self.sources.iter() {
            checksum += black_box(source.value());
        }
        black_box(checksum);
        ViewNode::leaf(Label::new("active-animation-profile"))
    }
}

fn active_animation_tree() -> (WidgetTree, Vec<uix::ui::WidgetId>) {
    let sources = Arc::new(
        (0..SOURCE_COUNT)
            .map(|index| {
                Animated::new(index as f32).to(index as f32 + 1.0, 10_000.0, Easing::linear)
            })
            .collect(),
    );
    let tree = build_view_tree_for_test(ActiveAnimationView { sources });
    let ids = animated_source_ids_for_test(&tree);
    assert_eq!(ids.len(), SOURCE_COUNT);
    (tree, ids)
}

fn advance_batch(tree: &mut WidgetTree, ids: &[uix::ui::WidgetId], start: Instant) -> usize {
    let mut active = 0;
    for frame in 1..=FRAMES_PER_BATCH {
        let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
        let updates = update_animated_sources_at_for_test(tree, ids, now, 1.0 / 60.0);
        active += updates.iter().filter(|(_, is_active)| *is_active).count();
        black_box(updates);
    }
    active
}

fn advance_geometric_batch(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
) -> usize {
    let mut active = 0;
    for frame in 1..=FRAMES_PER_BATCH {
        let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
        let updates = update_animated_sources_geometric_for_test(tree, ids, now, 1.0 / 60.0);
        active += updates.iter().filter(|(_, is_active)| *is_active).count();
        black_box(updates);
    }
    active
}

fn advance_reused_batch(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
    updates: &mut Vec<(uix::ui::WidgetId, bool)>,
) -> usize {
    let mut active = 0;
    for frame in 1..=FRAMES_PER_BATCH {
        let now = start + Duration::from_secs_f64(frame as f64 / 60.0);
        update_animated_sources_into_for_test(tree, ids, now, 1.0 / 60.0, updates);
        active += updates.iter().filter(|(_, is_active)| *is_active).count();
        black_box(&*updates);
    }
    active
}

fn measure_allocations(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    let active = advance_batch(tree, ids, start);
    MEASURING.store(false, Ordering::Release);
    assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn measure_geometric_allocations(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    let active = advance_geometric_batch(tree, ids, start);
    MEASURING.store(false, Ordering::Release);
    assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn measure_reused_allocations(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
    updates: &mut Vec<(uix::ui::WidgetId, bool)>,
) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    let active = advance_reused_batch(tree, ids, start, updates);
    MEASURING.store(false, Ordering::Release);
    assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn rotating_median_batch_ns(
    tree: &mut WidgetTree,
    ids: &[uix::ui::WidgetId],
    start: Instant,
    updates: &mut Vec<(uix::ui::WidgetId, bool)>,
) -> (u128, u128, u128) {
    let mut geometric_samples = [0_u128; TIMING_ROUNDS];
    let mut transient_samples = [0_u128; TIMING_ROUNDS];
    let mut reused_samples = [0_u128; TIMING_ROUNDS];
    for round in 0..TIMING_ROUNDS {
        let geometric_start = start + Duration::from_secs((round * 3 * FRAMES_PER_BATCH) as u64);
        let transient_start = geometric_start + Duration::from_secs(FRAMES_PER_BATCH as u64);
        let reused_start = transient_start + Duration::from_secs(FRAMES_PER_BATCH as u64);
        let measure_geometric = |tree: &mut WidgetTree| {
            let started = Instant::now();
            let active = advance_geometric_batch(tree, ids, geometric_start);
            let elapsed = started.elapsed().as_nanos();
            assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
            elapsed
        };
        let measure_transient = |tree: &mut WidgetTree| {
            let started = Instant::now();
            let active = advance_batch(tree, ids, transient_start);
            let elapsed = started.elapsed().as_nanos();
            assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
            elapsed
        };
        let measure_reused =
            |tree: &mut WidgetTree, updates: &mut Vec<(uix::ui::WidgetId, bool)>| {
                let started = Instant::now();
                let active = advance_reused_batch(tree, ids, reused_start, updates);
                let elapsed = started.elapsed().as_nanos();
                assert_eq!(active, SOURCE_COUNT * FRAMES_PER_BATCH);
                elapsed
            };
        match round % 3 {
            0 => {
                geometric_samples[round] = measure_geometric(tree);
                transient_samples[round] = measure_transient(tree);
                reused_samples[round] = measure_reused(tree, updates);
            }
            1 => {
                transient_samples[round] = measure_transient(tree);
                reused_samples[round] = measure_reused(tree, updates);
                geometric_samples[round] = measure_geometric(tree);
            }
            _ => {
                reused_samples[round] = measure_reused(tree, updates);
                geometric_samples[round] = measure_geometric(tree);
                transient_samples[round] = measure_transient(tree);
            }
        }
    }
    geometric_samples.sort_unstable();
    transient_samples.sort_unstable();
    reused_samples.sort_unstable();
    (
        geometric_samples[TIMING_ROUNDS / 2],
        transient_samples[TIMING_ROUNDS / 2],
        reused_samples[TIMING_ROUNDS / 2],
    )
}

#[test]
fn active_animation_frame_profile() {
    verify_single_reconcile_callback_reentrancy();
    let (mut tree, ids) = active_animation_tree();
    let start = Instant::now();
    let mut reused_updates = Vec::with_capacity(SOURCE_COUNT);
    // 首批预热所有动画与树工作区，不计入稳态样本。
    assert_eq!(
        advance_batch(&mut tree, &ids, start),
        SOURCE_COUNT * FRAMES_PER_BATCH
    );
    assert_eq!(
        advance_geometric_batch(
            &mut tree,
            &ids,
            start + Duration::from_secs(FRAMES_PER_BATCH as u64),
        ),
        SOURCE_COUNT * FRAMES_PER_BATCH
    );
    assert_eq!(
        advance_reused_batch(
            &mut tree,
            &ids,
            start + Duration::from_secs((FRAMES_PER_BATCH * 2) as u64),
            &mut reused_updates,
        ),
        SOURCE_COUNT * FRAMES_PER_BATCH
    );
    let geometric_allocations = measure_geometric_allocations(
        &mut tree,
        &ids,
        start + Duration::from_secs((FRAMES_PER_BATCH * 3) as u64),
    );
    let transient_allocations = measure_allocations(
        &mut tree,
        &ids,
        start + Duration::from_secs((FRAMES_PER_BATCH * 4) as u64),
    );
    let reused_allocations = measure_reused_allocations(
        &mut tree,
        &ids,
        start + Duration::from_secs((FRAMES_PER_BATCH * 5) as u64),
        &mut reused_updates,
    );
    let (geometric_batch_ns, transient_batch_ns, reused_batch_ns) = rotating_median_batch_ns(
        &mut tree,
        &ids,
        start + Duration::from_secs((FRAMES_PER_BATCH * 6) as u64),
        &mut reused_updates,
    );
    let retained_bytes = reused_updates.capacity() * size_of::<(uix::ui::WidgetId, bool)>();
    eprintln!(
        "PROFILE active_animation_frame_geometric: sources={SOURCE_COUNT} frames={FRAMES_PER_BATCH} \
         batch_ns={geometric_batch_ns} per_frame_ns={} allocations={} allocated_bytes={} \
         peak_live_bytes={} final_live_bytes={}",
        geometric_batch_ns / FRAMES_PER_BATCH as u128,
        geometric_allocations.count,
        geometric_allocations.allocated_bytes,
        geometric_allocations.peak_live_bytes,
        geometric_allocations.final_live_bytes,
    );
    eprintln!(
        "PROFILE active_animation_frame_transient: sources={SOURCE_COUNT} frames={FRAMES_PER_BATCH} \
         batch_ns={transient_batch_ns} per_frame_ns={} allocations={} allocated_bytes={} \
         peak_live_bytes={} final_live_bytes={}",
        transient_batch_ns / FRAMES_PER_BATCH as u128,
        transient_allocations.count,
        transient_allocations.allocated_bytes,
        transient_allocations.peak_live_bytes,
        transient_allocations.final_live_bytes,
    );
    eprintln!(
        "PROFILE active_animation_frame_reused: sources={SOURCE_COUNT} frames={FRAMES_PER_BATCH} \
         batch_ns={reused_batch_ns} per_frame_ns={} allocations={} allocated_bytes={} \
         peak_live_bytes={} final_live_bytes={} retained_bytes={retained_bytes}",
        reused_batch_ns / FRAMES_PER_BATCH as u128,
        reused_allocations.count,
        reused_allocations.allocated_bytes,
        reused_allocations.peak_live_bytes,
        reused_allocations.final_live_bytes,
    );
    assert!(transient_allocations.count < geometric_allocations.count);
    assert!(transient_allocations.allocated_bytes < geometric_allocations.allocated_bytes);
    // 单树 reconcile 站点不得再为每个动画源发布临时回调 Vec。
    assert_eq!(reused_allocations.count, 0);
    assert_eq!(reused_allocations.allocated_bytes, 0);
    assert_eq!(reused_allocations.peak_live_bytes, 0);
    assert_eq!(transient_allocations.final_live_bytes, 0);
}

fn verify_single_reconcile_callback_reentrancy() {
    let state = State::new(0_u32);
    let notifications = Arc::new(AtomicUsize::new(0));
    let state_for_callback = state.clone();
    let notifications_for_callback = Arc::clone(&notifications);
    state.bind_reconcile_invalidation(
        7,
        Arc::new(move || {
            notifications_for_callback.fetch_add(1, Ordering::Relaxed);
            // 回调仍必须在站点锁外执行，才能同步释放自己的唯一租约。
            state_for_callback.unbind_reconcile_invalidation(7);
        }),
    );

    state.set(1);
    state.set(2);

    assert_eq!(notifications.load(Ordering::Relaxed), 1);
}
