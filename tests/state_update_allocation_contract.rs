// 导入系统分配器与布局值，统计状态更新热段的堆活动。
use std::alloc::{GlobalAlloc, Layout, System};
// 防止编译器删除真实状态写入与协调回调观察。
use std::hint::black_box;
// 导入原子计数器与协调回调共享所有权。
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 导入多轮耗时测量。
use std::time::Instant;

// 只使用 UI System 的公开响应式状态契约。
use uix::ui::{Computed, Effect, State};

// 只在目标热段启用统计，隔离初始化和测试框架分配。
static MEASURING: AtomicBool = AtomicBool::new(false);
// 记录热段堆申请次数。
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
// 记录热段累计申请字节数。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
// 记录热段当前仍存活的临时字节数。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
// 记录热段临时分配的峰值 live 字节数。
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 使用系统分配器并旁路记录目标热段的申请与释放。
struct CountingAllocator;

// 将一次新增分配记入无锁统计。
fn record_allocation(size: usize) {
    // 每次成功申请对应一个堆活动。
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    // 累加本轮实际请求的字节数。
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    // 更新当前 live 字节并取得更新后的值。
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    // 以单调最大值维护峰值，不引入锁或分配。
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

// 从 live 统计中扣除已释放的目标热段临时分配。
fn record_deallocation(size: usize) {
    // 热段内部创建的快照应在同一热段释放。
    LIVE_BYTES.fetch_sub(size, Ordering::Relaxed);
}

// 为测试二进制安装旁路计数的系统分配器。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY：直接遵守调用方提供的有效 Layout 委托系统分配器。
        let pointer = unsafe { System.alloc(layout) };
        // 只有目标热段内的成功申请进入统计。
        if MEASURING.load(Ordering::Relaxed) && !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // 目标热段内析构的临时值必须先从 live 统计扣除。
        if MEASURING.load(Ordering::Relaxed) {
            record_deallocation(layout.size());
        }
        // SAFETY：pointer 与 Layout 来自同一个 System 分配器申请。
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY：直接遵守 GlobalAlloc 的原指针、旧布局和新尺寸契约。
        let next = unsafe { System.realloc(pointer, layout, new_size) };
        // 只记录目标热段中的成功扩缩容差额。
        if MEASURING.load(Ordering::Relaxed) && !next.is_null() {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            if new_size >= layout.size() {
                let growth = new_size - layout.size();
                ALLOCATED_BYTES.fetch_add(growth, Ordering::Relaxed);
                let live = LIVE_BYTES.fetch_add(growth, Ordering::Relaxed) + growth;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        next
    }
}

// 让本测试目标中的全部堆活动经过同一个无状态统计入口。
#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

// 保存一次热段测量的完整资源指标。
#[derive(Clone, Copy, Debug)]
struct AllocationStats {
    // 申请或扩容次数。
    count: usize,
    // 累计申请字节数。
    allocated_bytes: usize,
    // 同时存活的临时字节峰值。
    peak_live_bytes: usize,
}

// 清空计数并开始一次互不重叠的测量。
fn begin_measurement() {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
}

// 停止统计并取得本轮最终指标。
fn end_measurement() -> AllocationStats {
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    }
}

// 运行一轮业务列表状态写入到结构协调端口的生产热段。
fn run_round(
    state: &State<Vec<u64>>,
    reconcile_count: &AtomicUsize,
    iterations: usize,
) -> (u128, AllocationStats) {
    // 只从状态写入开始计时，排除一次性状态与回调注册成本。
    let started = Instant::now();
    begin_measurement();
    for index in 0..iterations {
        // 模拟列表项在事件处理器中的原地业务更新。
        state.update(|rows| {
            let target = index % rows.len();
            rows[target] = index as u64;
        });
        // 保留状态代数作为不可消除的外部观察。
        black_box(state.generation());
    }
    let stats = end_measurement();
    // 每次状态写入必须同步抵达 UI System 的结构协调端口。
    assert_eq!(
        reconcile_count.load(Ordering::Relaxed),
        state.generation() as usize
    );
    // 返回整轮耗时，调用方按每次更新归一化。
    (started.elapsed().as_nanos(), stats)
}

// 运行一轮高扇出公开观察器通知，覆盖 State 真实写入与锁外同步交付路径。
fn run_watched_round(
    state: &State<u64>,
    delivery_count: &AtomicUsize,
    watcher_count: usize,
    iterations: usize,
) -> (u128, AllocationStats) {
    // 只测已经完成订阅后的稳定更新热段。
    let started = Instant::now();
    begin_measurement();
    for value in 0..iterations {
        // 使用标量状态隔离 watcher 快照本身的时间与堆成本。
        state.set(black_box(value as u64));
    }
    let stats = end_measurement();
    // 每次更新必须按注册次序完整通知全部 watcher。
    assert_eq!(
        delivery_count.load(Ordering::Relaxed),
        state.generation() as usize * watcher_count
    );
    (started.elapsed().as_nanos(), stats)
}

// 运行一轮状态写入、Effect 唤醒、重复派生读取与依赖租约刷新生产链。
fn run_repeated_effect_reads_round(
    state: &State<u64>,
    effect: &Effect,
    executions: &AtomicUsize,
    iterations: usize,
) -> (u128, AllocationStats) {
    // 保存本轮前的执行数，使多轮测量共享同一 Effect 仍可独立验收。
    let before = executions.load(Ordering::Relaxed);
    // 排除 Effect 构造和首次订阅，只测稳定更新后的应用层响应式热段。
    let started = Instant::now();
    begin_measurement();
    for value in 1..=iterations {
        // 业务状态写入必须先只置 Effect pending。
        state.set(black_box(value as u64));
        // 应用 tick 同步消费失效并重新计算重复绑定表达式。
        assert!(effect.tick());
    }
    let stats = end_measurement();
    // 每次写入只执行一轮 Effect。
    assert_eq!(executions.load(Ordering::Relaxed) - before, iterations);
    (started.elapsed().as_nanos(), stats)
}

// 运行一轮条件绑定分支切换，覆盖 Effect 动态依赖退订、重订与快照刷新。
fn run_alternating_effect_dependencies_round(
    selector: &State<bool>,
    effect: &Effect,
    executions: &AtomicUsize,
    iterations: usize,
) -> (u128, AllocationStats) {
    // 保存轮前执行数，确保测得的是完整动态依赖刷新而非空 tick。
    let before = executions.load(Ordering::Relaxed);
    // 排除 State、Effect 与两组业务状态的一次性构造成本。
    let started = Instant::now();
    begin_measurement();
    for index in 0..iterations {
        // 偶数迭代进入右分支，奇数迭代返回左分支，保证每轮依赖集合都变化。
        selector.set(black_box(index % 2 == 0));
        // 应用调度同步消费失效并刷新下一轮精确租约集合。
        assert!(effect.tick());
    }
    let stats = end_measurement();
    // 每次条件切换必须恰好重跑一次 Effect。
    assert_eq!(executions.load(Ordering::Relaxed) - before, iterations);
    (started.elapsed().as_nanos(), stats)
}

// 验证大列表状态更新不会为仅失效通知建立无消费者快照。
#[test]
fn list_state_update_profile() {
    // 默认轮数足以形成稳定中位数，剖析时可通过环境变量放大。
    let rounds = std::env::var("UIX_PROFILE_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    // 常规回归保持快速，采样剖析可显式提高迭代数。
    let iterations = std::env::var("UIX_PROFILE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(512);
    // 使用真实列表体积放大按状态值线性增长的无效快照成本。
    let state = State::new(vec![0_u64; 8_192]);
    // 模拟 WidgetTree 持有的结构协调请求端口，保留真实 State 分发逻辑。
    let reconcile_count = Arc::new(AtomicUsize::new(0));
    let reconcile_sink = Arc::clone(&reconcile_count);
    state.set_reconcile_invalidation_fn(move || {
        reconcile_sink.fetch_add(1, Ordering::Relaxed);
    });
    // 预热锁、失效站点和结构协调回调路径。
    let _ = run_round(&state, &reconcile_count, 16);
    // 预分配多轮结果，避免统计关闭后的容器增长影响热段。
    let mut elapsed_per_update = Vec::with_capacity(rounds);
    let mut stats = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let (elapsed, round_stats) = run_round(&state, &reconcile_count, iterations);
        elapsed_per_update.push(elapsed / iterations as u128);
        stats.push(round_stats);
    }
    // 中位数抵御共享机器上的偶发调度噪声。
    elapsed_per_update.sort_unstable();
    stats.sort_unstable_by_key(|sample| sample.allocated_bytes);
    let elapsed_ns = elapsed_per_update[rounds / 2];
    let allocations = stats[rounds / 2];
    eprintln!(
        "PROFILE list_state_update: rounds={rounds} iterations={iterations} update_ns={elapsed_ns} allocations={} allocated_bytes={} peak_live_bytes={}",
        allocations.count, allocations.allocated_bytes, allocations.peak_live_bytes,
    );
    // 单树组件失效直接克隆并调用共享回调，不再建立临时回调向量。
    assert_eq!(allocations.count, 0);
    // 稳态状态发布不得按业务列表体积或更新次数申请临时存储。
    assert_eq!(allocations.allocated_bytes, 0);
    // 没有热段临时申请时峰值存活同样保持为零。
    assert_eq!(allocations.peak_live_bytes, 0);
    // 场景必须真实推进全部状态代数，防止零工作量误判为优化。
    assert_eq!(state.generation(), (16 + rounds * iterations) as u64);
}

// 验证仍有观察器时继续在状态锁外交付完整更新快照。
#[test]
fn watched_state_update_preserves_snapshot_and_reentrancy() {
    // 使用拥有型列表证明观察值不借用写锁内存。
    let state = State::new(vec![1_u64, 2, 3]);
    // 观察器重入读取同一个 State；若仍持有写锁，此调用将死锁。
    let reentrant = state.clone();
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed_sink = Arc::clone(&observed);
    state.watch(move |snapshot| {
        // 保存观察器收到的完整稳定快照。
        *observed_sink
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = snapshot.clone();
        // 锁外通知允许安全读取刚提交的新状态。
        assert_eq!(reentrant.get(), *snapshot);
    });
    // 先覆盖 set 入口，观察器必须收到完整替换值。
    state.set(vec![7, 8]);
    assert_eq!(
        *observed.lock().unwrap_or_else(|error| error.into_inner()),
        vec![7, 8]
    );
    // 再覆盖 update 入口的原地修改。
    state.update(|rows| rows.push(9));
    // 观察器仍精确收到更新后的拥有型值。
    assert_eq!(
        *observed.lock().unwrap_or_else(|error| error.into_inner()),
        vec![7, 8, 9]
    );
}

// 验证 watcher 重入注册不会改变正在交付的有序快照。
#[test]
fn watched_state_reentrant_registration_preserves_snapshot_order() {
    let state = State::new(0_u64);
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    // 用一次性持有槽避免观察器长期形成 State 自引用环。
    let reentrant_state = Arc::new(std::sync::Mutex::new(Some(state.clone())));
    let first_order = Arc::clone(&order);
    let first_reentrant = Arc::clone(&reentrant_state);
    state.watch(move |_| {
        first_order
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push("first");
        let Some(state) = first_reentrant
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        else {
            return;
        };
        let late_order = Arc::clone(&first_order);
        state.watch(move |_| {
            late_order
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("late");
        });
    });
    let second_order = Arc::clone(&order);
    state.watch(move |_| {
        second_order
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push("second");
    });

    // 本轮快照不包含第一个回调重入新增的 watcher。
    state.set(1);
    assert_eq!(
        *order.lock().unwrap_or_else(|error| error.into_inner()),
        vec!["first", "second"]
    );
    // 下一轮按原注册顺序追加交付新 watcher。
    state.set(2);
    assert_eq!(
        *order.lock().unwrap_or_else(|error| error.into_inner()),
        vec!["first", "second", "first", "second", "late"]
    );
}

// 剖析应用层共享状态向大量公开观察者同步广播时的快照成本。
#[test]
fn high_fanout_watched_state_profile() {
    // 真实主题、语言或应用设置状态可能同时驱动大量组件观察者。
    let watcher_count = std::env::var("UIX_PROFILE_WATCHERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(128);
    let rounds = std::env::var("UIX_PROFILE_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    let iterations = std::env::var("UIX_PROFILE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_024);
    let state = State::new(0_u64);
    let delivery_count = Arc::new(AtomicUsize::new(0));
    for _ in 0..watcher_count {
        let delivery_sink = Arc::clone(&delivery_count);
        state.watch(move |value| {
            // 保留真实同步消费并阻止编译器删除动态调用。
            black_box(*value);
            delivery_sink.fetch_add(1, Ordering::Relaxed);
        });
    }
    // 预热写锁、动态调用和分配器路径。
    let _ = run_watched_round(&state, &delivery_count, watcher_count, 16);
    let mut elapsed_per_update = Vec::with_capacity(rounds);
    let mut stats = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let (elapsed, round_stats) =
            run_watched_round(&state, &delivery_count, watcher_count, iterations);
        elapsed_per_update.push(elapsed / iterations as u128);
        stats.push(round_stats);
    }
    elapsed_per_update.sort_unstable();
    stats.sort_unstable_by_key(|sample| sample.allocated_bytes);
    let elapsed_ns = elapsed_per_update[rounds / 2];
    let allocations = stats[rounds / 2];
    eprintln!(
        "PROFILE high_fanout_watched_state: rounds={rounds} iterations={iterations} watchers={watcher_count} update_ns={elapsed_ns} allocations={} allocated_bytes={} peak_live_bytes={}",
        allocations.count, allocations.allocated_bytes, allocations.peak_live_bytes,
    );
    // 稳态通知只克隆共享有序快照句柄，不再按 watcher 数量申请或复制 Arc。
    assert_eq!(allocations.count, 0);
    assert_eq!(allocations.allocated_bytes, 0);
    assert_eq!(allocations.peak_live_bytes, 0);
}

// 剖析单个绑定表达式重复读取同一响应式槽时的依赖捕获与租约刷新成本。
#[test]
fn repeated_effect_dependency_profile() {
    // 模拟同一动态 View/格式化绑定在一轮内多处读取共享应用状态。
    let reads_per_tick = std::env::var("UIX_PROFILE_EFFECT_READS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let rounds = std::env::var("UIX_PROFILE_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    let iterations = std::env::var("UIX_PROFILE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(512);
    let state = State::new(0_u64);
    let executions = Arc::new(AtomicUsize::new(0));
    let watched = state.clone();
    let execution_sink = Arc::clone(&executions);
    let effect = Effect::new(move || {
        // 保留同一轮中每次公开 State::get，覆盖真实自动依赖捕获入口。
        let mut checksum = 0_u64;
        for _ in 0..reads_per_tick {
            checksum = checksum.wrapping_add(watched.get());
        }
        black_box(checksum);
        execution_sink.fetch_add(1, Ordering::Relaxed);
    });
    // 预热通知、捕获、差集刷新与分配器路径；计数仍用于最终语义核对。
    let _ = run_repeated_effect_reads_round(&state, &effect, &executions, 16);
    let execution_base = executions.load(Ordering::Relaxed);
    let mut elapsed_per_tick = Vec::with_capacity(rounds);
    let mut stats = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let before = executions.load(Ordering::Relaxed);
        let (elapsed, round_stats) =
            run_repeated_effect_reads_round(&state, &effect, &executions, iterations);
        // 每个独立测量轮都必须完整执行指定次数。
        assert_eq!(executions.load(Ordering::Relaxed) - before, iterations);
        elapsed_per_tick.push(elapsed / iterations as u128);
        stats.push(round_stats);
    }
    elapsed_per_tick.sort_unstable();
    stats.sort_unstable_by_key(|sample| sample.allocated_bytes);
    let elapsed_ns = elapsed_per_tick[rounds / 2];
    let allocations = stats[rounds / 2];
    eprintln!(
        "PROFILE repeated_effect_dependency: rounds={rounds} iterations={iterations} reads={reads_per_tick} tick_ns={elapsed_ns} allocations={} allocated_bytes={} peak_live_bytes={}",
        allocations.count, allocations.allocated_bytes, allocations.peak_live_bytes,
    );
    // 唯一依赖的稳定刷新当前只需七次小申请，重复读取不得使分配随读取数增长。
    assert!(
        allocations.count <= iterations * 8,
        "重复依赖捕获申请次数回退: {allocations:?}"
    );
    // 为 HashMap、快照和锁外通知保留平台余量，同时拒绝重新装箱每次重复读取。
    assert!(
        allocations.allocated_bytes <= iterations * 640,
        "重复依赖捕获申请字节回退: {allocations:?}"
    );
    // 稳态临时对象峰值必须保持常量级，不得再次接近完整重复依赖向量。
    assert!(
        allocations.peak_live_bytes <= 512,
        "重复依赖捕获峰值 live 回退: {allocations:?}"
    );
    // 场景必须覆盖全部预热和测量执行，防止零工作量数据进入比较。
    assert_eq!(
        executions.load(Ordering::Relaxed),
        execution_base + rounds * iterations
    );
}

// 验证 State → Computed → Effect 链在重复派生读取去重后仍精确传播一次失效。
#[test]
fn repeated_computed_reads_preserve_invalidation_chain() {
    let source = State::new(2_usize);
    let computed_source = source.clone();
    let derived = Computed::new(move || computed_source.get() * 3);
    let watched = derived.clone();
    let executions = Arc::new(AtomicUsize::new(0));
    let execution_sink = Arc::clone(&executions);
    let latest = Arc::new(AtomicUsize::new(0));
    let latest_sink = Arc::clone(&latest);
    let effect = Effect::new(move || {
        // 同一轮重复读取派生槽，覆盖 Computed::get 的提前去重入口。
        let sum = (0..64).fold(0_usize, |sum, _| sum + watched.get());
        latest_sink.store(sum, Ordering::Relaxed);
        execution_sink.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(executions.load(Ordering::Relaxed), 1);
    assert_eq!(latest.load(Ordering::Relaxed), 2 * 3 * 64);

    // 上游写入先同步失效 Computed，再只把下游 Effect 标记为待执行。
    source.set(5);
    assert!(effect.has_pending());
    assert!(effect.tick());
    assert_eq!(executions.load(Ordering::Relaxed), 2);
    assert_eq!(latest.load(Ordering::Relaxed), 5 * 3 * 64);
    // 同一失效不得被重复读取放大成多轮 Effect 执行。
    assert!(!effect.tick());
}

// 验证条件绑定切换后只保留当前分支租约，旧分支不再唤醒 Effect。
#[test]
fn alternating_effect_dependencies_release_stale_branch() {
    let selector = State::new(false);
    let left = State::new(11_u64);
    let right = State::new(29_u64);
    let watched_selector = selector.clone();
    let watched_left = left.clone();
    let watched_right = right.clone();
    let executions = Arc::new(AtomicUsize::new(0));
    let execution_sink = Arc::clone(&executions);
    let latest = Arc::new(AtomicUsize::new(0));
    let latest_sink = Arc::clone(&latest);
    let effect = Effect::new(move || {
        // 每轮只读取当前条件分支，形成一份会变化的动态依赖集合。
        let value = if watched_selector.get() {
            watched_right.get()
        } else {
            watched_left.get()
        };
        latest_sink.store(value as usize, Ordering::Relaxed);
        execution_sink.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(executions.load(Ordering::Relaxed), 1);
    assert_eq!(latest.load(Ordering::Relaxed), 11);

    // 切到右分支后，Effect 必须发布右值并释放左分支租约。
    selector.set(true);
    assert!(effect.has_pending());
    assert!(effect.tick());
    assert_eq!(latest.load(Ordering::Relaxed), 29);
    left.set(13);
    assert!(!effect.has_pending());
    right.set(31);
    assert!(effect.has_pending());
    assert!(effect.tick());
    assert_eq!(latest.load(Ordering::Relaxed), 31);

    // 再切回左分支，证明退订与重订可以反向重复且不残留旧通知。
    selector.set(false);
    assert!(effect.tick());
    assert_eq!(latest.load(Ordering::Relaxed), 13);
    right.set(37);
    assert!(!effect.has_pending());
    left.set(17);
    assert!(effect.has_pending());
    assert!(effect.tick());
    assert_eq!(latest.load(Ordering::Relaxed), 17);
    assert_eq!(executions.load(Ordering::Relaxed), 5);
}

// 剖析真实条件绑定在两组业务状态间切换时的动态依赖租约刷新成本。
#[test]
fn alternating_effect_dependencies_profile() {
    let branch_width = std::env::var("UIX_PROFILE_EFFECT_BRANCH_WIDTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(32);
    let rounds = std::env::var("UIX_PROFILE_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    let iterations = std::env::var("UIX_PROFILE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(512);
    // 模拟条件 View 两侧各自读取一组不同业务状态。
    let left = Arc::new(
        (0..branch_width)
            .map(|index| State::new(index as u64))
            .collect::<Vec<_>>(),
    );
    let right = Arc::new(
        (0..branch_width)
            .map(|index| State::new((index + branch_width) as u64))
            .collect::<Vec<_>>(),
    );
    let selector = State::new(false);
    let executions = Arc::new(AtomicUsize::new(0));
    let latest = Arc::new(AtomicUsize::new(0));
    let watched_selector = selector.clone();
    let watched_left = Arc::clone(&left);
    let watched_right = Arc::clone(&right);
    let execution_sink = Arc::clone(&executions);
    let latest_sink = Arc::clone(&latest);
    let effect = Effect::new(move || {
        // 首次读取选择器，随后只读取当前条件分支的完整依赖集合。
        let branch = if watched_selector.get() {
            &watched_right
        } else {
            &watched_left
        };
        let checksum = branch
            .iter()
            .fold(0_u64, |sum, state| sum.wrapping_add(state.get()));
        black_box(checksum);
        latest_sink.store(checksum as usize, Ordering::Relaxed);
        execution_sink.fetch_add(1, Ordering::Relaxed);
    });
    // 预热两侧注册表、租约映射与分配器路径。
    let _ = run_alternating_effect_dependencies_round(&selector, &effect, &executions, 16);
    let execution_base = executions.load(Ordering::Relaxed);
    let mut elapsed_per_tick = Vec::with_capacity(rounds);
    let mut stats = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let (elapsed, round_stats) =
            run_alternating_effect_dependencies_round(&selector, &effect, &executions, iterations);
        elapsed_per_tick.push(elapsed / iterations as u128);
        stats.push(round_stats);
    }
    elapsed_per_tick.sort_unstable();
    stats.sort_unstable_by_key(|sample| sample.allocated_bytes);
    let elapsed_ns = elapsed_per_tick[rounds / 2];
    let allocations = stats[rounds / 2];
    eprintln!(
        "PROFILE alternating_effect_dependencies: rounds={rounds} iterations={iterations} branch_width={branch_width} tick_ns={elapsed_ns} allocations={} allocated_bytes={} peak_live_bytes={}",
        allocations.count, allocations.allocated_bytes, allocations.peak_live_bytes,
    );
    // 场景必须覆盖全部预热和测量执行，防止零工作量样本进入比较。
    assert_eq!(
        executions.load(Ordering::Relaxed),
        execution_base + rounds * iterations
    );
    // 保留最终业务校验和观察；合法的单元素左分支可以恰好为零。
    black_box(latest.load(Ordering::Relaxed));
    // 具体依赖源与租约不得退化回按每个唯一依赖三次闭包装箱。
    assert!(
        allocations.count <= iterations * (branch_width / 2 + 12),
        "动态依赖刷新申请次数回退: {allocations:?}"
    );
    // 哈希差集仍按依赖数保存元数据，但不得重新分配三份闭包对象。
    assert!(
        allocations.allocated_bytes <= iterations * (512 + branch_width * 340),
        "动态依赖刷新申请字节回退: {allocations:?}"
    );
    // 单轮峰值只保留差集容器与具体租约，不得恢复完整闭包峰值。
    assert!(
        allocations.peak_live_bytes <= 256 + branch_width * 208,
        "动态依赖刷新峰值 live 回退: {allocations:?}"
    );
}
