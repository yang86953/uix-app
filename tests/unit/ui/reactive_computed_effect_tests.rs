// 引入受测响应式公开句柄。
use super::{Computed, Effect, State};
// 引入受控 panic 捕获工具。
use std::panic::{AssertUnwindSafe, catch_unwind};
// 引入并发计数和标记原子。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 引入通道、共享所有权和互斥容器。
use std::sync::{Arc, Mutex, mpsc};
// 引入所有有限等待的时长类型。
use std::time::Duration;

// 验证 State 经 Computed 会立即唤醒只读取派生值的 Effect。
#[test]
// 覆盖直接 Computed 下游订阅链。
fn computed_effect_direct_chain_wakes_effect() {
    // 创建直接上游状态。
    let source = State::new(1_u8);
    // 克隆上游供派生函数读取。
    let computed_source = source.clone();
    // 创建读取上游的派生值。
    let computed = Computed::new(move || computed_source.get() + 1);
    // 保存 Effect 实际读取到的派生值。
    let runs = Arc::new(AtomicUsize::new(0));
    // 克隆记录器供副作用写入。
    let effect_runs = runs.clone();
    // 克隆派生值供副作用读取。
    let effect_computed = computed.clone();
    // 创建只读取 Computed 的副作用。
    let effect = Effect::new(move || {
        // 读取派生值建立下游租约。
        let value = effect_computed.get();
        // 记录本轮副作用实际读取的值。
        effect_runs.store(value as usize, Ordering::Release);
    });
    // 写入 State 应同步令 Effect 进入 pending。
    source.set(2);
    // 确认 Computed 链路已经通知到 Effect。
    assert!(effect.has_pending());
    // 消费 pending 并执行第二轮副作用。
    assert!(effect.tick());
    // 确认 tick 实际读取到新的派生值三。
    assert_eq!(runs.load(Ordering::Acquire), 3);
}

// 验证嵌套 Computed 的失效会逐层同步传播。
#[test]
// 覆盖 State 到内层到外层到 Effect 的完整链。
fn computed_effect_nested_chain_wakes_effect() {
    // 创建基础状态。
    let source = State::new(1_u8);
    // 克隆基础状态供内层计算读取。
    let inner_source = source.clone();
    // 创建第一层派生值。
    let inner = Computed::new(move || inner_source.get() + 1);
    // 克隆第一层供第二层计算读取。
    let outer_inner = inner.clone();
    // 创建第二层派生值。
    let outer = Computed::new(move || outer_inner.get() + 1);
    // 保存 Effect 实际读取到的嵌套派生值。
    let observed = Arc::new(AtomicUsize::new(0));
    // 克隆最外层供副作用读取。
    let effect_outer = outer.clone();
    // 克隆记录器供副作用写入。
    let effect_observed = observed.clone();
    // 创建只订阅最外层的 Effect。
    let effect = Effect::new(move || {
        // 读取最外层建立末端租约。
        let value = effect_outer.get();
        // 记录本轮副作用实际读取的值。
        effect_observed.store(value as usize, Ordering::Release);
    });
    // 修改基础状态触发嵌套失效链。
    source.set(2);
    // 确认最终 Effect 已收到同步通知。
    assert!(effect.has_pending());
    // 确认 tick 会真正重算并运行。
    assert!(effect.tick());
    // 确认两层加一后实际读取到四。
    assert_eq!(observed.load(Ordering::Acquire), 4);
}

// 验证 Computed 重算后会释放动态分支的旧 State 租约。
#[test]
// 覆盖共享差集刷新逻辑在 Computed 上的使用。
fn computed_effect_dynamic_branch_releases_old_upstream() {
    // 创建选择分支的状态。
    let choose_left = State::new(true);
    // 创建左分支状态。
    let left = State::new(1_u8);
    // 创建右分支状态。
    let right = State::new(2_u8);
    // 克隆选择器供计算函数读取。
    let computed_choose_left = choose_left.clone();
    // 克隆左分支供计算函数读取。
    let computed_left = left.clone();
    // 克隆右分支供计算函数读取。
    let computed_right = right.clone();
    // 创建按选择器读取单个分支的派生值。
    let computed = Computed::new(move || {
        // 根据当前分支建立动态依赖。
        if computed_choose_left.get() {
            // 读取左分支。
            computed_left.get()
        } else {
            // 读取右分支。
            computed_right.get()
        }
    });
    // 初始只订阅左分支。
    assert_eq!(left.effect_subscriber_count(), 1);
    // 初始不订阅右分支。
    assert_eq!(right.effect_subscriber_count(), 0);
    // 切换到右分支。
    choose_left.set(false);
    // 同步读取以执行动态依赖差集更新。
    assert_eq!(computed.get(), 2);
    // 确认旧左分支租约已经释放。
    assert_eq!(left.effect_subscriber_count(), 0);
    // 确认新右分支租约已经安装。
    assert_eq!(right.effect_subscriber_count(), 1);
}

// 验证 Effect 与 Computed 的析构都会释放各自租约。
#[test]
// 覆盖下游和上游两个方向的确定性析构。
fn computed_effect_drop_releases_both_subscription_directions() {
    // 创建 Computed 的基础上游。
    let source = State::new(1_u8);
    // 克隆上游供派生函数读取。
    let computed_source = source.clone();
    // 创建派生值。
    let computed = Computed::new(move || computed_source.get());
    // 克隆派生值供 Effect 读取。
    let effect_computed = computed.clone();
    // 创建末端 Effect。
    let effect = Effect::new(move || {
        // 读取派生值安装下游租约。
        let _ = effect_computed.get();
    });
    // 确认 Computed 有一个 Effect 下游。
    assert_eq!(computed.effect_subscriber_count(), 1);
    // 释放末端 Effect。
    drop(effect);
    // 确认 Computed 下游立即清空。
    assert_eq!(computed.effect_subscriber_count(), 0);
    // 确认 Computed 仍占有一个 State 上游租约。
    assert_eq!(source.effect_subscriber_count(), 1);
    // 释放唯一 Computed 所有者。
    drop(computed);
    // 确认 State 上游租约也立即清空。
    assert_eq!(source.effect_subscriber_count(), 0);
}

// 验证 Computed 观察值与 Effect 订阅之间的竞态会补偿 pending。
#[test]
// 覆盖派生源注册后 revision 复核。
fn computed_effect_observe_then_subscribe_race_sets_pending() {
    // 创建派生值的直接上游。
    let source = State::new(1_u8);
    // 克隆上游供 Computed 读取。
    let computed_source = source.clone();
    // 创建派生值。
    let computed = Computed::new(move || computed_source.get());
    // 克隆派生值供 Effect 读取。
    let effect_computed = computed.clone();
    // 克隆 State 在读取后推进派生 revision。
    let effect_source = source.clone();
    // 创建在 observe 与 subscribe 之间写入 State 的 Effect。
    let effect = Effect::new(move || {
        // 先读取 Computed 取得旧观察 revision。
        let _ = effect_computed.get();
        // 再写入 State 使 Computed 同步推进 revision。
        effect_source.set(2);
    });
    // 注册后复核必须保留该竞态的 pending。
    assert!(effect.has_pending());
}

// 验证重算期间的上游写入会保留下一轮 dirty。
#[test]
// 覆盖缓存 revision 与并发失效不互相吞掉。
fn computed_effect_write_during_recompute_keeps_next_round_dirty() {
    // 创建计算函数读取的上游状态。
    let source = State::new(1_u8);
    // 创建控制仅在下一轮计算写入的标记。
    let write_during_compute = Arc::new(AtomicBool::new(false));
    // 克隆上游供计算函数使用。
    let computed_source = source.clone();
    // 克隆标记供计算函数读取。
    let computed_write_during_compute = write_during_compute.clone();
    // 创建会在受控重算中写回上游的派生值。
    let computed = Computed::new(move || {
        // 读取当前上游值建立依赖。
        let value = computed_source.get();
        // 仅在指定轮次触发重算中写入。
        if computed_write_during_compute.swap(false, Ordering::AcqRel) {
            // 写入上游以在计算期间发送失效。
            computed_source.set(value + 1);
        }
        // 返回本轮读取到的值。
        value
    });
    // 请求下一轮计算中执行写入。
    write_during_compute.store(true, Ordering::Release);
    // 先通过外部写入使缓存置脏。
    source.set(2);
    // 第一轮可返回旧快照但必须保留下一轮脏标记。
    let _ = computed.get();
    // 第二轮必须观察计算期写入后的最终值。
    assert_eq!(computed.get(), 3);
}

// 验证计算函数 panic 后会恢复运行权并保留 dirty。
#[test]
// 覆盖 Computed 的 panic RAII 生命周期。
fn computed_effect_panic_recovers_dirty_and_running() {
    // 创建会触发重算的上游状态。
    let source = State::new(1_u8);
    // 创建受控 panic 标记。
    let panic_once = Arc::new(AtomicBool::new(false));
    // 克隆上游供计算函数读取。
    let computed_source = source.clone();
    // 克隆 panic 标记供计算函数读取。
    let computed_panic_once = panic_once.clone();
    // 创建可受控 panic 的派生值。
    let computed = Computed::new(move || {
        // 读取上游建立租约。
        let value = computed_source.get();
        // 在指定轮次模拟用户计算失败。
        if computed_panic_once.swap(false, Ordering::AcqRel) {
            // 触发受控 panic。
            panic!("computed test panic");
        }
        // 正常路径返回上游值。
        value
    });
    // 请求下一次重算发生 panic。
    panic_once.store(true, Ordering::Release);
    // 写入上游使 Computed 进入 dirty。
    source.set(2);
    // 捕获预期 panic 以继续验证恢复状态。
    assert!(catch_unwind(AssertUnwindSafe(|| computed.get())).is_err());
    // 后续读取必须能再次取得运行权并成功重算。
    assert_eq!(computed.get(), 2);
}

// 验证两个 Effect 可共享同一个 Computed 的下游租约。
#[test]
// 覆盖释放其中一个下游不会误删另一个的令牌。
fn computed_effect_two_effects_share_and_release_independently() {
    // 创建派生值读取的上游状态。
    let source = State::new(1_u8);
    // 克隆上游供派生函数读取。
    let computed_source = source.clone();
    // 创建共享派生值。
    let computed = Computed::new(move || computed_source.get());
    // 克隆派生值供第一个 Effect 读取。
    let first_computed = computed.clone();
    // 创建第一个末端 Effect。
    let first = Effect::new(move || {
        // 读取共享派生值安装首个下游。
        let _ = first_computed.get();
    });
    // 克隆派生值供第二个 Effect 读取。
    let second_computed = computed.clone();
    // 创建第二个末端 Effect。
    let second = Effect::new(move || {
        // 读取共享派生值安装第二个下游。
        let _ = second_computed.get();
    });
    // 确认两个令牌独立登记。
    assert_eq!(computed.effect_subscriber_count(), 2);
    // 释放第一个 Effect。
    drop(first);
    // 确认第二个租约仍然存活。
    assert_eq!(computed.effect_subscriber_count(), 1);
    // 写入上游应继续通知第二个 Effect。
    source.set(2);
    // 确认第二个下游收到 pending。
    assert!(second.has_pending());
    // 释放剩余下游。
    drop(second);
    // 确认下游表已归零。
    assert_eq!(computed.effect_subscriber_count(), 0);
    // 释放最后一个 Computed 所有者。
    drop(computed);
    // 确认 State 上游租约也已归零。
    assert_eq!(source.effect_subscriber_count(), 0);
}

// 验证两个脏读取线程不会从重算门返回失效旧缓存。
#[test]
// 覆盖缓存值与 revision 同锁发布以及条件变量等待。
fn computed_effect_concurrent_dirty_gets_wait_for_fresh_cache() {
    // 创建将被重算读取的上游状态。
    let source = State::new(1_u8);
    // 创建控制下一次计算阻塞的标记。
    let block_compute = Arc::new(AtomicBool::new(false));
    // 创建计算已开始的通知通道。
    let (entered_tx, entered_rx) = mpsc::channel();
    // 创建主线程放行计算的通道。
    let (release_tx, release_rx) = mpsc::channel();
    // 使用互斥包装接收端以满足计算函数 Sync 契约。
    let release_rx = Arc::new(Mutex::new(release_rx));
    // 克隆上游供派生函数读取。
    let computed_source = source.clone();
    // 克隆阻塞标记供派生函数读取。
    let computed_block_compute = block_compute.clone();
    // 克隆开始通知端供派生函数使用。
    let computed_entered_tx = entered_tx.clone();
    // 克隆放行接收端供派生函数使用。
    let computed_release_rx = release_rx.clone();
    // 创建受控阻塞的派生值。
    let computed = Computed::new(move || {
        // 先读取当前上游值。
        let value = computed_source.get();
        // 仅在受控重算轮次阻塞。
        if computed_block_compute.swap(false, Ordering::AcqRel) {
            // 报告用户计算已在所有内部锁外开始。
            computed_entered_tx
                .send(())
                .expect("开始通知接收端不应提前关闭");
            // 有限等待主线程放行，避免测试无限挂起。
            computed_release_rx
                .lock()
                .expect("放行接收端互斥锁不应中毒")
                .recv_timeout(Duration::from_secs(3))
                .expect("主线程应在时限内放行计算");
        }
        // 返回当前上游值。
        value
    });
    // 请求下一次重算进入受控阻塞。
    block_compute.store(true, Ordering::Release);
    // 写入上游使缓存失效。
    source.set(2);
    // 创建第一条读取完成通知通道。
    let (first_done_tx, first_done_rx) = mpsc::channel();
    // 克隆 Computed 供第一线程取得计算权。
    let first_computed = computed.clone();
    // 启动第一条脏读取线程。
    let first = std::thread::spawn(move || {
        // 读取并发布新缓存。
        let value = first_computed.get();
        // 在退出前报告读取结果。
        first_done_tx
            .send(value)
            .expect("第一条完成接收端不应提前关闭");
    });
    // 有限等待确认第一线程已经在计算函数内。
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("第一条脏读取应在时限内进入计算函数");
    // 创建第二条读取完成通知通道。
    let (second_done_tx, second_done_rx) = mpsc::channel();
    // 克隆 Computed 供第二线程等待重算门。
    let second_computed = computed.clone();
    // 启动第二条脏读取线程。
    let second = std::thread::spawn(move || {
        // 读取必须等待新缓存而非返回旧值。
        let value = second_computed.get();
        // 将读取结果发回主线程。
        second_done_tx
            .send(value)
            .expect("第二条完成接收端不应提前关闭");
    });
    // 在计算尚未放行时第二条读取不得错误完成。
    assert!(
        second_done_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    // 放行第一轮用户计算。
    release_tx.send(()).expect("放行接收端不应提前关闭");
    // 在 join 前有限等待确认第一条读取发布新值。
    assert_eq!(
        first_done_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("第一条读取应在时限内完成"),
        2
    );
    // 确认第二条读取同样只获得新值。
    assert_eq!(
        second_done_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("第二条读取应在时限内完成"),
        2
    );
    // 确认第一线程没有 panic。
    first.join().expect("第一条读取线程不应 panic");
    // 确认第二线程没有 panic。
    second.join().expect("第二条读取线程不应 panic");
    // 读取同锁快照以确认当前值和 revision 成对存在。
    let (value, generation) = computed.cache_snapshot();
    // 确认缓存值来自本轮新计算。
    assert_eq!(value, 2);
    // 确认缓存 revision 至少反映一次上游失效。
    assert!(generation >= 1);
}

// 验证 Computed 自递归重算会明确 panic 而非等待自身。
#[test]
// 覆盖线程私有重算栈的自环死锁防护。
fn computed_effect_recursive_evaluation_panics_without_deadlock() {
    // 创建保存构造后自身句柄的受控槽位。
    let self_slot = Arc::new(Mutex::new(None::<Computed<u8>>));
    // 创建控制是否进入递归的标记。
    let recurse = Arc::new(AtomicBool::new(false));
    // 克隆自身句柄槽位供计算函数访问。
    let computed_self_slot = self_slot.clone();
    // 克隆递归标记供计算函数读取。
    let computed_recurse = recurse.clone();
    // 构造首次不会递归的派生值。
    let computed = Computed::new(move || {
        // 仅在受控轮次触发自引用。
        if computed_recurse.load(Ordering::Acquire) {
            // 取得构造完成后保存的自身句柄。
            let current = computed_self_slot
                .lock()
                .expect("自身句柄互斥锁不应中毒")
                .clone()
                .expect("构造后自身句柄应存在");
            // 对同一槽重入 get 必须立即 panic。
            return current.get();
        }
        // 首次构造返回稳定值。
        1
    });
    // 在构造完成后保存自身句柄。
    *self_slot.lock().expect("自身句柄互斥锁不应中毒") = Some(computed.clone());
    // 请求下一轮进入受控递归。
    recurse.store(true, Ordering::Release);
    // 创建后台结果通知通道。
    let (result_tx, result_rx) = mpsc::channel();
    // 克隆派生值供后台调用 invalidate。
    let worker_computed = computed.clone();
    // 启动可能暴露自等待死锁的后台线程。
    let worker = std::thread::spawn(move || {
        // 捕获明确递归 panic。
        let panicked = catch_unwind(AssertUnwindSafe(|| worker_computed.invalidate())).is_err();
        // 报告后台调用已经返回。
        result_tx.send(panicked).expect("结果接收端不应提前关闭");
    });
    // 有限等待确保不会永久等待同一槽。
    assert!(
        result_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("递归计算应在时限内返回")
    );
    // 确认后台线程没有逃逸 panic。
    worker.join().expect("递归保护线程不应逃逸 panic");
    // 清空自身句柄，打破计算闭包经槽位持有 Computed 的强环。
    *self_slot.lock().expect("自身句柄互斥锁不应中毒") = None;
}
