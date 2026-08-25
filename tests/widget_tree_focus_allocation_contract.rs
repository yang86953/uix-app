//! 测量深层组件树在两个真实焦点目标之间切换时的耗时与临时堆成本。

use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use uix::prelude::{Button, Container, EventResult, Input, SystemEvent, Widget};
use uix::ui::__private::WidgetTree;
use uix::ui::{EventHandler, WidgetCapabilities};

// 只在焦点切换热段开启统计，排除组件树构造与首次容量预热。
static MEASURING: AtomicBool = AtomicBool::new(false);
// 记录热段申请或扩容次数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 记录热段累计申请字节数。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
// 记录热段当前仍存活的临时字节数。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
// 记录热段临时分配的峰值 live 字节数。
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 使用系统分配器执行真实申请，并精确记录目标热段的堆流量。
struct CountingAllocator;

// 以无锁方式更新测量热段的峰值 live 字节数。
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
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            let live = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            update_peak(live);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 原样把调用方提供的有效 Layout 委托给系统分配器。
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if MEASURING.load(Ordering::Relaxed) && !ptr.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
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

// 保存一次相同生产场景测量的完整资源指标。
#[derive(Clone, Copy, Debug)]
struct FocusStats {
    elapsed_ns: u128,
    allocations: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
}

// 以 const 类型参数生成不同的真实组件类型，同时共享焦点路由观察实现。
struct FocusProbe<const KIND: u8> {
    label: &'static str,
    focusable: bool,
    log: Arc<Mutex<Vec<String>>>,
}

impl<const KIND: u8> FocusProbe<KIND> {
    // 创建持有同一观察日志的焦点组件。
    fn new(label: &'static str, focusable: bool, log: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            label,
            focusable,
            log: Arc::clone(log),
        }
    }

    // 在线程内测试日志中追加一条稳定顺序记录。
    fn record(&self, value: String) {
        self.log
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(value);
    }
}

impl<const KIND: u8> Widget for FocusProbe<KIND> {
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
        WidgetCapabilities::from_bits(WidgetCapabilities::EVENT)
    }

    fn tab_index(&self) -> i32 {
        i32::from(self.focusable)
    }

    fn as_event(&self) -> Option<&dyn EventHandler> {
        Some(self)
    }

    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        Some(self)
    }
}

impl<const KIND: u8> EventHandler for FocusProbe<KIND> {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusOut => self.record(format!("event:{}:out", self.label)),
            SystemEvent::FocusIn => self.record(format!("event:{}:in", self.label)),
            _ => {}
        }
        EventResult::NotHandled
    }

    fn on_focus_within(&mut self, focused: bool) -> EventResult {
        self.record(format!("within:{}:{focused}", self.label));
        EventResult::NotHandled
    }
}

// 构造两个拥有深层共同根、不同叶目标的真实组件子树。
fn deep_focus_tree(depth: usize) -> WidgetTree {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new()));

    // 左分支使用 Button 作为第一个真实焦点目标。
    let mut left_parent = root;
    for _ in 0..depth {
        left_parent = tree.add_child(left_parent, Box::new(Container::new()));
    }
    tree.add_child(left_parent, Box::new(Button::new("左目标")));

    // 右分支使用 Input 作为第二个真实焦点目标。
    let mut right_parent = root;
    for _ in 0..depth {
        right_parent = tree.add_child(right_parent, Box::new(Container::new()));
    }
    tree.add_child(right_parent, Box::new(Input::new("右目标")));

    // 让后续切换覆盖窗口已聚焦时的 FocusOut/Within/FocusIn 完整生产路径。
    let _ = tree.dispatch_event(&SystemEvent::WindowFocus);
    assert!(tree.focus_by_type::<Button>().is_some());
    assert!(tree.focus_by_type::<Input>().is_some());
    tree
}

// 开始一次互不重叠的窄热段测量。
fn begin_measurement() {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
}

// 停止测量并返回全部指标。
fn end_measurement(elapsed_ns: u128) -> FocusStats {
    MEASURING.store(false, Ordering::Release);
    FocusStats {
        elapsed_ns,
        allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    }
}

// 在同一棵窗口树上交替切换两个不同类型的焦点叶节点。
fn run_focus_round(tree: &mut WidgetTree, iterations: usize) -> FocusStats {
    let started = Instant::now();
    begin_measurement();
    for index in 0..iterations {
        let focused = if index % 2 == 0 {
            tree.focus_by_type::<Button>()
        } else {
            tree.focus_by_type::<Input>()
        };
        black_box(focused.expect("两个真实焦点目标必须始终留在组件树中"));
    }
    let stats = end_measurement(started.elapsed().as_nanos());
    assert!(tree.is_focused_type::<Input>());
    stats
}

#[test]
fn sibling_focus_transition_keeps_frozen_path_and_callback_order() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(FocusProbe::<0>::new("root", false, &log)));
    let left = tree.add_child(root, Box::new(FocusProbe::<1>::new("left", false, &log)));
    tree.add_child(left, Box::new(FocusProbe::<2>::new("a", true, &log)));
    let right = tree.add_child(root, Box::new(FocusProbe::<3>::new("right", false, &log)));
    tree.add_child(right, Box::new(FocusProbe::<4>::new("b", true, &log)));

    // 首次聚焦只用于建立旧路径，随后清空观察日志。
    assert!(tree.focus_by_type::<FocusProbe<2>>().is_some());
    log.lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    // 兄弟分支切换必须使用回调前冻结的旧、新路径快照。
    assert!(tree.focus_by_type::<FocusProbe<4>>().is_some());

    let observed = log
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    assert_eq!(
        observed,
        [
            "event:a:out",
            "event:left:out",
            "event:root:out",
            "within:a:false",
            "within:left:false",
            "event:b:in",
            "event:right:in",
            "event:root:in",
            "within:right:true",
            "within:b:true",
        ]
    );
    assert!(tree.is_focused_type::<FocusProbe<4>>());
}

#[test]
fn focus_transition_scratch_keeps_reentrant_slot_and_pre_callback_snapshot() {
    let source = include_str!("../src/ui/widget_runtime/widget/tree_events/pointer.rs");
    let start = source
        .find("    pub(crate) fn set_focus(")
        .expect("应保留焦点事务入口");
    let end = source[start..]
        .find("    pub(super) fn focus_containment_path(")
        .map(|offset| start + offset)
        .expect("应保留窗口生命周期路径入口");
    let transition = &source[start..end];

    // take 在回调期间给同步重入留下独立空槽，不持有 RefCell 或树字段借用。
    let take = transition
        .find("std::mem::take(&mut self.focus_transition_path_scratch)")
        .expect("焦点事务必须独占取出工作区");
    assert!(!transition.contains("borrow_mut"));
    // 两条结构路径都必须在第一个 FocusOut 用户回调前冻结。
    let old_snapshot = transition[take..]
        .find("append_focus_containment_path(old_focus")
        .map(|offset| take + offset)
        .expect("应先冻结旧焦点路径");
    let new_snapshot = transition[old_snapshot + 1..]
        .find("append_focus_containment_path(new_focus")
        .map(|offset| old_snapshot + 1 + offset)
        .expect("应再冻结新焦点路径");
    let first_callback = transition
        .find("self.dispatch_to(old, &SystemEvent::FocusOut)")
        .expect("应保留 FocusOut 目标分发");
    assert!(take < old_snapshot && old_snapshot < new_snapshot && new_snapshot < first_callback);
    // 外层路径在生命周期协调前归还；重入只影响容量归属，不改变本次快照。
    let restore = transition
        .find("self.focus_transition_path_scratch = paths;")
        .expect("焦点事务结束前应归还工作区");
    let lifecycle = transition
        .find("self.reconcile_lifecycle_after_layout()")
        .expect("应保留焦点后的生命周期协调");
    assert!(first_callback < restore && restore < lifecycle);
}

#[test]
fn deep_branch_focus_switch_profile() {
    let profile_label =
        std::env::var("UIX_FOCUS_PROFILE_LABEL").unwrap_or_else(|_| "optimized".to_owned());
    let tree_depth = std::env::var("UIX_FOCUS_TREE_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let iterations = std::env::var("UIX_FOCUS_PERF_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 128 } else { 4_096 });
    let max_allocations_per_switch = std::env::var("UIX_FOCUS_MAX_ALLOCS_PER_SWITCH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2);
    let max_bytes_per_switch = std::env::var("UIX_FOCUS_MAX_BYTES_PER_SWITCH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(32);
    let max_peak_live_bytes = std::env::var("UIX_FOCUS_MAX_PEAK_LIVE_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let mut tree = deep_focus_tree(tree_depth);

    // 先完成一次完整交替，隔离首次焦点目标与遍历缓存准备成本。
    let _ = run_focus_round(&mut tree, 2);
    for round in 1..=5 {
        let stats = run_focus_round(&mut tree, iterations);
        eprintln!(
            "focus-profile label={profile_label} round={round} depth={tree_depth} iterations={iterations} ns_per_switch={:.2} allocations={} allocated_bytes={} peak_live_bytes={}",
            stats.elapsed_ns as f64 / iterations as f64,
            stats.allocations,
            stats.allocated_bytes,
            stats.peak_live_bytes,
        );
        // 焦点路径快照预热后只允许组件自身固定的小对象申请，不得恢复逐层 Vec 扩容。
        assert!(
            stats.allocations <= iterations * max_allocations_per_switch,
            "每次焦点切换不得超过 {max_allocations_per_switch} 次堆申请"
        );
        assert!(
            stats.allocated_bytes <= iterations * max_bytes_per_switch,
            "每次焦点切换不得超过 {max_bytes_per_switch} 字节累计堆申请"
        );
        assert!(
            stats.peak_live_bytes <= max_peak_live_bytes,
            "焦点切换临时堆峰值不得超过 {max_peak_live_bytes} 字节"
        );
    }
}
