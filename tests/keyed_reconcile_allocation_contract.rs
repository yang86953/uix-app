//! 测量稳定 keyed 同级声明协调的临时堆流量与耗时。

use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use uix::prelude::{Button, Input, ViewNode, WidgetId};
use uix::ui::__private::traits::{Widget, WidgetCapabilities};
use uix::ui::__private::{
    WidgetTree, build_view_tree_for_test, reconcile_view_tree_for_test,
    set_reconcile_key_fingerprint_mask_for_test, view_tree_children_for_test,
};

// 真实列表常见的同级 keyed 节点数量，同时足以放大索引临时成本。
const SIBLING_COUNT: usize = 512;
// 每轮协调多次同一规模声明，降低时钟粒度与一次性噪声。
const RECONCILES_PER_ROUND: usize = 24;
// 使用奇数轮中位数过滤调度抖动。
const TIMING_ROUNDS: usize = 9;

// 只在协调热段开启统计，声明节点构造与首次运行时建树不计入结果。
static MEASURING: AtomicBool = AtomicBool::new(false);
// 记录 alloc、alloc_zeroed 与 realloc 次数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 记录全部申请或扩容的新尺寸。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
// 业务 key 的固定字节宽度用于把旧索引字符串克隆从其他申请中精确分离。
const PROFILE_KEY_BYTES: usize = "stable-business-key-0000-with-production-width".len();
// 记录与业务 key 等宽的申请次数，验证索引不再复制字符串所有权。
static KEY_WIDTH_ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 记录测量区间内新申请字节的近似存活量。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
// 记录热段新申请字节的峰值存活量。
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 使用系统分配器执行真实申请，并在窄测量窗口记录资源指标。
struct CountingAllocator;

// 更新峰值而不引入测量器自身分配。
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

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            if layout.size() == PROFILE_KEY_BYTES {
                KEY_WIDTH_ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            let live = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            update_peak(live);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            if layout.size() == PROFILE_KEY_BYTES {
                KEY_WIDTH_ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            let live = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            update_peak(live);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            let _ = LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
                Some(live.saturating_sub(layout.size()))
            });
        }
        // SAFETY: 指针和 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if MEASURING.load(Ordering::Relaxed) && !next.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size == PROFILE_KEY_BYTES {
                KEY_WIDTH_ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            let live = LIVE_BYTES
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
                    Some(live.saturating_sub(layout.size()).saturating_add(new_size))
                })
                .unwrap_or_default()
                .saturating_sub(layout.size())
                .saturating_add(new_size);
            update_peak(live);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// 不含组件自身集合与字符串申请，让测量聚焦协调索引。
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

// 保存一次热段的完整资源指标。
#[derive(Clone, Copy, Debug)]
struct ProfileStats {
    elapsed_ns: u128,
    allocations: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
    key_width_allocations: usize,
}

// 构造稳定、唯一且长度一致的业务 key 同级声明。
fn keyed_children() -> Vec<ViewNode> {
    (0..SIBLING_COUNT)
        .map(|index| {
            ViewNode::leaf(ReconcileProbe).key(format!(
                "stable-business-key-{index:04}-with-production-width"
            ))
        })
        .collect()
}

// 构造固定非空生产宽度文字的真实 Button keyed 同级声明。
fn keyed_button_children() -> Vec<ViewNode> {
    (0..SIBLING_COUNT)
        .map(|index| {
            ViewNode::leaf(Button::new(format!("生产按钮-{index:04}-固定宽度"))).key(format!(
                "stable-button-key-{index:04}-with-production-width"
            ))
        })
        .collect()
}

// 构造实际运行时父节点与首版 keyed 子树。
fn keyed_tree() -> (WidgetTree, WidgetId) {
    let tree = build_view_tree_for_test(ViewNode::new(ReconcileProbe, keyed_children()));
    let root = tree.root_id().expect("keyed 协调场景必须建立根节点");
    (tree, root)
}

// 构造真实 Button keyed 父节点与首版子树。
fn keyed_button_tree() -> (WidgetTree, WidgetId) {
    let tree = build_view_tree_for_test(ViewNode::new(ReconcileProbe, keyed_button_children()));
    let root = tree.root_id().expect("Button keyed 协调场景必须建立根节点");
    (tree, root)
}

// 在测量前完整构造下一轮声明，排除声明字符串和 ViewNode 自身申请。
fn prepared_rounds() -> Vec<Vec<ViewNode>> {
    (0..RECONCILES_PER_ROUND)
        .map(|_| keyed_children())
        .collect()
}

// 在 Button 热段外完整构造下一轮声明，排除声明自身申请。
fn prepared_button_rounds() -> Vec<Vec<ViewNode>> {
    (0..RECONCILES_PER_ROUND)
        .map(|_| keyed_button_children())
        .collect()
}

// 在作用域结束时恢复摘要掩码，避免碰撞测试影响后续性能轮次。
struct FingerprintMaskGuard(u64);

impl Drop for FingerprintMaskGuard {
    fn drop(&mut self) {
        let _ = set_reconcile_key_fingerprint_mask_for_test(self.0);
    }
}

// 强制全部 key 摘要碰撞，验证精确回退、身份顺序、焦点状态与卸载清理。
fn assert_collision_state_and_removal_semantics() {
    let initial = ViewNode::new(
        ReconcileProbe,
        vec![
            ViewNode::leaf(Input::new("保留状态")).key("collision-input"),
            ViewNode::leaf(Button::new("重排目标")).key("collision-button"),
            ViewNode::leaf(ReconcileProbe).key("collision-tail"),
        ],
    );
    let mut tree = build_view_tree_for_test(initial);
    let root = tree.root_id().expect("碰撞场景必须建立根节点");
    let initial_order = view_tree_children_for_test(&tree, root);
    assert_eq!(initial_order.len(), 3);
    let input_id = tree
        .focus_by_type::<Input>()
        .expect("Input 必须成为真实焦点状态所有者");
    assert_eq!(input_id, initial_order[0]);

    let previous_mask = set_reconcile_key_fingerprint_mask_for_test(0);
    let _mask_guard = FingerprintMaskGuard(previous_mask);
    reconcile_view_tree_for_test(
        &mut tree,
        ViewNode::new(
            ReconcileProbe,
            vec![
                ViewNode::leaf(Button::new("重排目标")).key("collision-button"),
                ViewNode::leaf(Input::new("保留状态")).key("collision-input"),
                ViewNode::leaf(ReconcileProbe).key("collision-tail"),
            ],
        ),
    );
    assert_eq!(
        view_tree_children_for_test(&tree, root),
        [initial_order[1], initial_order[0], initial_order[2]]
    );
    assert_eq!(tree.managers().focus.focused_widget(), Some(input_id));

    reconcile_view_tree_for_test(
        &mut tree,
        ViewNode::new(
            ReconcileProbe,
            vec![
                ViewNode::leaf(Button::new("重排目标")).key("collision-button"),
                ViewNode::leaf(ReconcileProbe).key("collision-tail"),
            ],
        ),
    );
    assert_eq!(
        view_tree_children_for_test(&tree, root),
        [initial_order[1], initial_order[2]]
    );
    assert!(tree.get(input_id).is_none());
    assert_eq!(tree.managers().focus.focused_widget(), None);
}

// 执行一轮相同 keyed 声明协调并返回精确资源指标。
fn run_round(tree: &mut WidgetTree, root: WidgetId, children: Vec<Vec<ViewNode>>) -> ProfileStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    KEY_WIDTH_ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    let started = Instant::now();
    for next in children {
        reconcile_view_tree_for_test(
            black_box(tree),
            ViewNode::new(ReconcileProbe, black_box(next)),
        );
        assert_eq!(tree.root_id(), Some(root));
    }
    let elapsed_ns = started.elapsed().as_nanos();
    MEASURING.store(false, Ordering::Release);
    ProfileStats {
        elapsed_ns,
        allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        key_width_allocations: KEY_WIDTH_ALLOCATION_COUNT.load(Ordering::Relaxed),
    }
}

#[test]
fn stable_keyed_reconcile_profile() {
    assert_collision_state_and_removal_semantics();
    let profile_label =
        std::env::var("UIX_RECONCILE_PROFILE_LABEL").unwrap_or_else(|_| "current".to_owned());
    let (mut tree, root) = keyed_tree();
    let stable_children = view_tree_children_for_test(&tree, root);
    // 先执行完整协调，隔离首次类型分派与运行时准备成本。
    let _ = run_round(&mut tree, root, vec![keyed_children()]);

    let mut samples = Vec::with_capacity(TIMING_ROUNDS);
    for round in 1..=TIMING_ROUNDS {
        let stats = run_round(&mut tree, root, prepared_rounds());
        eprintln!(
            "reconcile-profile label={profile_label} round={round} siblings={SIBLING_COUNT} reconciles={RECONCILES_PER_ROUND} ns_per_reconcile={:.2} allocations={} allocated_bytes={} peak_live_bytes={} key_width_allocations={}",
            stats.elapsed_ns as f64 / RECONCILES_PER_ROUND as f64,
            stats.allocations,
            stats.allocated_bytes,
            stats.peak_live_bytes,
            stats.key_width_allocations,
        );
        samples.push(stats);
    }
    samples.sort_unstable_by_key(|stats| stats.elapsed_ns);
    let median = samples[TIMING_ROUNDS / 2];
    // 计时完成后读取最终队列，稳定场景只应保留根 Layout。
    let retained_layout_roots = {
        let invalidation = tree
            .invalidation()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        invalidation.layout_roots().len()
    };
    eprintln!(
        "PROFILE stable_keyed_reconcile label={profile_label} median_ns_per_reconcile={:.2} allocations_per_reconcile={:.2} allocated_bytes_per_reconcile={:.2} peak_live_bytes={} key_width_allocations_per_reconcile={:.2} retained_layout_roots={retained_layout_roots}",
        median.elapsed_ns as f64 / RECONCILES_PER_ROUND as f64,
        median.allocations as f64 / RECONCILES_PER_ROUND as f64,
        median.allocated_bytes as f64 / RECONCILES_PER_ROUND as f64,
        median.peak_live_bytes,
        median.key_width_allocations as f64 / RECONCILES_PER_ROUND as f64,
    );

    assert_eq!(stable_children.len(), SIBLING_COUNT);
    assert_eq!(view_tree_children_for_test(&tree, root), stable_children);
    assert_eq!(retained_layout_roots, 1);

    let (mut button_tree, button_root) = keyed_button_tree();
    let stable_button_children = view_tree_children_for_test(&button_tree, button_root);
    // 先完成一次 Button 协调，隔离首次真实组件分派与运行时准备成本。
    let _ = run_round(&mut button_tree, button_root, vec![keyed_button_children()]);

    let mut button_samples = Vec::with_capacity(TIMING_ROUNDS);
    for round in 1..=TIMING_ROUNDS {
        let prepared = prepared_button_rounds();
        let stats = run_round(&mut button_tree, button_root, prepared);
        eprintln!(
            "reconcile-button-profile label={profile_label} round={round} siblings={SIBLING_COUNT} reconciles={RECONCILES_PER_ROUND} ns_per_reconcile={:.2} allocations={} allocated_bytes={} peak_live_bytes={} key_width_allocations={}",
            stats.elapsed_ns as f64 / RECONCILES_PER_ROUND as f64,
            stats.allocations,
            stats.allocated_bytes,
            stats.peak_live_bytes,
            stats.key_width_allocations,
        );
        button_samples.push(stats);
    }
    button_samples.sort_unstable_by_key(|stats| stats.elapsed_ns);
    let button_median = button_samples[TIMING_ROUNDS / 2];
    // Button 计时完成后读取最终队列，稳定场景只应保留根 Layout。
    let button_retained_layout_roots = {
        let invalidation = button_tree
            .invalidation()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        invalidation.layout_roots().len()
    };
    eprintln!(
        "PROFILE stable_keyed_button_reconcile label={profile_label} median_ns_per_reconcile={:.2} allocations_per_reconcile={:.2} allocated_bytes_per_reconcile={:.2} peak_live_bytes={} key_width_allocations_per_reconcile={:.2} retained_layout_roots={button_retained_layout_roots}",
        button_median.elapsed_ns as f64 / RECONCILES_PER_ROUND as f64,
        button_median.allocations as f64 / RECONCILES_PER_ROUND as f64,
        button_median.allocated_bytes as f64 / RECONCILES_PER_ROUND as f64,
        button_median.peak_live_bytes,
        button_median.key_width_allocations as f64 / RECONCILES_PER_ROUND as f64,
    );

    assert_eq!(stable_button_children.len(), SIBLING_COUNT);
    assert_eq!(
        view_tree_children_for_test(&button_tree, button_root),
        stable_button_children
    );
    assert_eq!(button_retained_layout_roots, 1);
}
