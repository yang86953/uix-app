//! 测量无 key 同级声明协调的位置复用、结构变更、临时堆流量与耗时。

use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::hint::black_box;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use uix::prelude::{ViewNode, WidgetId};
use uix::ui::__private::traits::{Widget, WidgetCapabilities};
use uix::ui::__private::{
    WidgetTree, build_view_tree_for_test, reconcile_view_tree_for_test,
    view_tree_children_for_test, view_tree_effective_user_select_for_test,
};
use uix::ui::UserSelect;

// 真实大列表常见的同级声明规模，足以稳定放大协调临时结构成本。
const SIBLING_COUNT: usize = 512;
// 结构轮次裁掉四分之一尾部，再恢复完整列表。
const REDUCED_SIBLING_COUNT: usize = SIBLING_COUNT * 3 / 4;
// 每轮批量协调多次，降低时钟粒度和单次调度噪声。
const RECONCILES_PER_ROUND: usize = 24;
// 使用奇数轮中位数过滤偶发系统抖动。
const TIMING_ROUNDS: usize = 9;
// 只需跟踪协调期间同时存活的少量新申请，不给被测路径自身分配内存。
const TRACKED_ALLOCATION_CAPACITY: usize = 4096;
// 固定容量保存热段内的申请尺寸分布，避免剖析器自身进入被测堆流量。
const ALLOCATION_SIZE_CAPACITY: usize = 64;

// 只在协调热段启用精确资源统计，声明树准备与首次挂载不计入结果。
static MEASURING: AtomicBool = AtomicBool::new(false);
// 记录 alloc、alloc_zeroed 与 realloc 的真实调用次数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 记录所有申请或扩容请求的新尺寸之和。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
// 只累计测量窗口内新申请且尚未释放的字节数。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
// 保存测量窗口内新申请字节的精确峰值。
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 固定表保存测量窗口内申请的真实指针与尺寸，避免把窗口前对象的释放误扣为临时内存。
#[derive(Clone, Copy)]
struct TrackedAllocation {
    ptr: usize,
    size: usize,
}

const EMPTY_TRACKED_ALLOCATION: TrackedAllocation = TrackedAllocation { ptr: 0, size: 0 };
static TRACKED_ALLOCATIONS: Mutex<[TrackedAllocation; TRACKED_ALLOCATION_CAPACITY]> =
    Mutex::new([EMPTY_TRACKED_ALLOCATION; TRACKED_ALLOCATION_CAPACITY]);

#[derive(Clone, Copy)]
struct AllocationSizeCount {
    size: usize,
    count: usize,
}

const EMPTY_ALLOCATION_SIZE_COUNT: AllocationSizeCount = AllocationSizeCount { size: 0, count: 0 };
static ALLOCATION_SIZE_COUNTS: Mutex<[AllocationSizeCount; ALLOCATION_SIZE_CAPACITY]> =
    Mutex::new([EMPTY_ALLOCATION_SIZE_COUNT; ALLOCATION_SIZE_CAPACITY]);

// 使用系统分配器执行生产申请，并在窄测量窗口旁路登记资源事实。
struct CountingAllocator;

// 更新精确同时存活峰值，不引入测量器自身分配。
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

// 记录精确申请尺寸及出现次数；超过固定种类上限说明剖析精度不足并立即失败。
fn record_allocation_size(size: usize) {
    let mut counts = ALLOCATION_SIZE_COUNTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = counts.iter_mut().find(|entry| entry.size == size) {
        entry.count += 1;
        return;
    }
    let Some(entry) = counts.iter_mut().find(|entry| entry.count == 0) else {
        panic!("协调申请尺寸种类超过固定剖析容量");
    };
    *entry = AllocationSizeCount { size, count: 1 };
}

// 登记窗口内真实申请；固定表耗尽表示场景失去精确性，直接终止测试。
fn track_allocation(ptr: *mut u8, size: usize) {
    let mut tracked = TRACKED_ALLOCATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(entry) = tracked.iter_mut().find(|entry| entry.ptr == 0) else {
        panic!("协调分配跟踪表容量不足");
    };
    *entry = TrackedAllocation {
        ptr: ptr as usize,
        size,
    };
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    update_peak(live);
}

// 只释放同一窗口内登记的申请，窗口前已有对象的销毁不会污染 live 指标。
fn untrack_allocation(ptr: *mut u8) {
    let mut tracked = TRACKED_ALLOCATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = tracked.iter_mut().find(|entry| entry.ptr == ptr as usize) {
        LIVE_BYTES.fetch_sub(entry.size, Ordering::Relaxed);
        *entry = EMPTY_TRACKED_ALLOCATION;
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            record_allocation_size(layout.size());
            track_allocation(ptr, layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            record_allocation_size(layout.size());
            track_allocation(ptr, layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            untrack_allocation(ptr);
        }
        // SAFETY: 指针和 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let measured = MEASURING.load(Ordering::Relaxed);
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if measured && !next.is_null() {
            // 只有 realloc 成功后旧申请才失效；失败时继续保留原指针的精确 live 记录。
            if !ptr.is_null() {
                untrack_allocation(ptr);
            }
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            record_allocation_size(new_size);
            track_allocation(next, new_size);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// 无内部集合与字符串的叶节点，让结果聚焦 ViewAdapter 的结构协调。
#[derive(Default)]
struct ReconcileProbe;

impl Widget for ReconcileProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::new()
    }
}

// 保存热段的完整资源指标。
#[derive(Clone, Copy, Debug)]
struct AllocationStats {
    count: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
    final_live_bytes: usize,
}

// 构造依赖位置语义复用的无 key 同级声明。
fn unkeyed_children(count: usize) -> Vec<ViewNode> {
    (0..count).map(|_| ViewNode::leaf(ReconcileProbe)).collect()
}

// 建立真实运行时父节点及初始无 key 子树。
fn unkeyed_tree() -> (WidgetTree, WidgetId) {
    let tree = build_view_tree_for_test(ViewNode::new(
        ReconcileProbe,
        unkeyed_children(SIBLING_COUNT),
    ));
    let root = tree.root_id().expect("无 key 协调场景必须建立根节点");
    (tree, root)
}

// 在热段外准备稳定规模声明，排除 ViewNode 与 Widget 自身构造成本。
fn stable_rounds() -> Vec<Vec<ViewNode>> {
    (0..RECONCILES_PER_ROUND)
        .map(|_| unkeyed_children(SIBLING_COUNT))
        .collect()
}

// 构造父子选择声明，用于验证最终值不变时仍保存新的声明事实。
fn user_select_tree(parent: UserSelect, child: UserSelect) -> ViewNode {
    ViewNode::new(
        ReconcileProbe,
        vec![ViewNode::leaf(ReconcileProbe).user_select(child)],
    )
    .user_select(parent)
}

#[test]
fn unchanged_effective_policy_keeps_the_new_declaration_for_later_inheritance() {
    let mut tree = build_view_tree_for_test(user_select_tree(UserSelect::None, UserSelect::Auto));
    let root = tree.root_id().expect("选择策略场景必须建立根节点");
    let child = view_tree_children_for_test(&tree, root)[0];
    assert_eq!(
        view_tree_effective_user_select_for_test(&tree, child),
        Some(UserSelect::None)
    );

    // 子声明从 Auto 改成 None，但在当前父策略下最终值保持 None，命中快返路径。
    reconcile_view_tree_for_test(
        &mut tree,
        user_select_tree(UserSelect::None, UserSelect::None),
    );
    // 随后改变父策略；子节点必须使用上轮已保存的 None，而不是旧 Auto。
    reconcile_view_tree_for_test(
        &mut tree,
        user_select_tree(UserSelect::Text, UserSelect::None),
    );
    assert_eq!(
        view_tree_effective_user_select_for_test(&tree, root),
        Some(UserSelect::Text)
    );
    assert_eq!(
        view_tree_effective_user_select_for_test(&tree, child),
        Some(UserSelect::None)
    );
}

// 在热段外准备真实尾部批量卸载与重新挂载声明。
fn structural_rounds() -> Vec<Vec<ViewNode>> {
    (0..RECONCILES_PER_ROUND)
        .map(|index| {
            let count = if index % 2 == 0 {
                REDUCED_SIBLING_COUNT
            } else {
                SIBLING_COUNT
            };
            unkeyed_children(count)
        })
        .collect()
}

// 清空固定跟踪表与计数器，开始互不重叠的精确测量。
fn begin_measurement() {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    TRACKED_ALLOCATIONS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .fill(EMPTY_TRACKED_ALLOCATION);
    ALLOCATION_SIZE_COUNTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .fill(EMPTY_ALLOCATION_SIZE_COUNT);
    MEASURING.store(true, Ordering::Release);
}

// 停止测量并取得该热段的精确资源事实。
fn end_measurement() -> AllocationStats {
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
    }
}

// 执行已经准备好的完整生产协调入口，并维持同一根所有者。
fn reconcile_rounds(tree: &mut WidgetTree, root: WidgetId, rounds: Vec<Vec<ViewNode>>) {
    for children in rounds {
        reconcile_view_tree_for_test(
            black_box(tree),
            ViewNode::new(ReconcileProbe, black_box(children)),
        );
        assert_eq!(tree.root_id(), Some(root));
    }
}

// 单独测量分配，避免分配跟踪器锁放大耗时结果。
fn measure_allocations(
    tree: &mut WidgetTree,
    root: WidgetId,
    rounds: Vec<Vec<ViewNode>>,
) -> AllocationStats {
    begin_measurement();
    reconcile_rounds(tree, root, rounds);
    end_measurement()
}

// 对同场景运行九轮，返回每轮协调耗时中位数。
fn median_ns_per_reconcile(
    tree: &mut WidgetTree,
    root: WidgetId,
    prepare: fn() -> Vec<Vec<ViewNode>>,
) -> f64 {
    let mut samples = Vec::with_capacity(TIMING_ROUNDS);
    for _ in 0..TIMING_ROUNDS {
        let rounds = prepare();
        let started = Instant::now();
        reconcile_rounds(tree, root, rounds);
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    samples[TIMING_ROUNDS / 2] as f64 / RECONCILES_PER_ROUND as f64
}

#[test]
fn unkeyed_reconcile_profile() {
    let profile_label =
        std::env::var("UIX_UNKEYED_PROFILE_LABEL").unwrap_or_else(|_| "current".to_owned());
    let (mut tree, root) = unkeyed_tree();
    let initial_order = view_tree_children_for_test(&tree, root);

    // 先执行稳定协调，隔离首次类型分派和运行时准备成本。
    reconcile_rounds(&mut tree, root, vec![unkeyed_children(SIBLING_COUNT)]);
    let stable_allocations = measure_allocations(&mut tree, root, stable_rounds());
    let mut stable_sizes = ALLOCATION_SIZE_COUNTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|entry| entry.count > 0)
        .map(|entry| (entry.size, entry.count))
        .collect::<Vec<_>>();
    stable_sizes.sort_unstable_by_key(|(_, count)| std::cmp::Reverse(*count));
    let stable_ns = median_ns_per_reconcile(&mut tree, root, stable_rounds);
    assert_eq!(view_tree_children_for_test(&tree, root), initial_order);

    // 交替缩短和恢复尾部，验证位置前缀身份保留及真实卸载、挂载路径。
    let structural_allocations = measure_allocations(&mut tree, root, structural_rounds());
    let mut structural_sizes = ALLOCATION_SIZE_COUNTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|entry| entry.count > 0)
        .map(|entry| (entry.size, entry.count))
        .collect::<Vec<_>>();
    structural_sizes.sort_unstable_by_key(|(_, count)| std::cmp::Reverse(*count));
    let structural_ns = median_ns_per_reconcile(&mut tree, root, structural_rounds);
    let final_order = view_tree_children_for_test(&tree, root);
    assert_eq!(final_order.len(), SIBLING_COUNT);
    assert_eq!(
        &final_order[..REDUCED_SIBLING_COUNT],
        &initial_order[..REDUCED_SIBLING_COUNT]
    );

    eprintln!(
        "PROFILE unkeyed_reconcile label={profile_label} siblings={SIBLING_COUNT} reconciles={RECONCILES_PER_ROUND} stable_ns_per_reconcile={stable_ns:.2} stable_allocations={} stable_allocated_bytes={} stable_peak_live_bytes={} stable_final_live_bytes={} structural_ns_per_reconcile={structural_ns:.2} structural_allocations={} structural_allocated_bytes={} structural_peak_live_bytes={} structural_final_live_bytes={}",
        stable_allocations.count,
        stable_allocations.allocated_bytes,
        stable_allocations.peak_live_bytes,
        stable_allocations.final_live_bytes,
        structural_allocations.count,
        structural_allocations.allocated_bytes,
        structural_allocations.peak_live_bytes,
        structural_allocations.final_live_bytes,
    );
    eprintln!("PROFILE unkeyed_reconcile stable_allocation_sizes={stable_sizes:?}");
    eprintln!("PROFILE unkeyed_reconcile structural_allocation_sizes={structural_sizes:?}");

    // 不变选择策略不得恢复逐节点祖先栈；门槛保留协调根级工作区容量余量。
    assert!(stable_allocations.count <= 512);
    assert!(stable_allocations.allocated_bytes <= 2_000_000);
    assert!(stable_allocations.peak_live_bytes <= 70_000);
    assert_eq!(stable_allocations.final_live_bytes, 0);
    // 批量卸载、挂载允许节点生命周期流量，但不得恢复每个复用节点的祖先栈。
    assert!(structural_allocations.count <= 2_000);
    assert!(structural_allocations.allocated_bytes <= 1_500_000);
    assert!(structural_allocations.peak_live_bytes <= 50_000);
    assert!(structural_allocations.final_live_bytes <= 1_024);
}
