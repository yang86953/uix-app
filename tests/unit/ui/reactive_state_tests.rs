// 引入被测私有依赖收集器与状态类型。
use super::{Effect, State, StateSlotId, TRACKING_DEPS, collect_deps};
// 引入集合类型以便无序比较依赖槽身份。
use std::collections::HashSet;
// 引入受控 unwind 捕获工具以验证 panic 生命周期。
use std::panic::{AssertUnwindSafe, catch_unwind};
// 引入并发测试所需的共享原子计数器。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 引入带超时收据的线程通道、共享所有权与互斥接收端。
use std::sync::{Arc, Mutex, mpsc};
// 引入并发回归测试的有限等待时长。
use std::time::Duration;

// 验证内层 panic 被捕获后外层前后读取不会丢失，且追踪器不会残留。
#[test]
// 执行内层 panic 后恢复外层追踪器的回归。
fn capture_runtime_collect_deps_restores_outer_tracker_after_nested_panic() {
    // 创建外层 panic 前读取的状态。
    let before_panic = State::new(1_u8);
    // 创建只应属于内层失败调用的状态。
    let nested_only = State::new(2_u8);
    // 创建外层捕获 panic 后读取的状态。
    let after_panic = State::new(3_u8);
    // 创建正常调用后用于残留探测的状态。
    let ordinary_read = State::new(4_u8);
    // 预先记录外层两个状态的稳定槽身份。
    let expected_outer_slots = HashSet::from([before_panic.slot_id(), after_panic.slot_id()]);

    // 在外层依赖收集器中执行内层失败调用。
    let (_, outer_dependencies) = collect_deps(|| {
        // 记录 panic 前的外层读取。
        let _ = before_panic.get();
        // 捕获内层依赖收集闭包的预期 panic。
        let nested_result = catch_unwind(AssertUnwindSafe(|| {
            // 创建独立内层收集器并读取仅属于它的状态。
            let _ = collect_deps(|| {
                // 记录内层依赖。
                let _ = nested_only.get();
                // 模拟用户计算函数失败。
                panic!("nested dependency tracking panic");
            });
        }));
        // 确认测试确实走过受控 panic 路径。
        assert!(nested_result.is_err());
        // 记录 panic 后的外层读取。
        let _ = after_panic.get();
    });
    // 投影外层结果，忽略无法比较的 generation 闭包。
    let collected_outer_slots: HashSet<StateSlotId> = outer_dependencies
        // 逐项读取依赖保存的状态槽身份。
        .into_iter()
        // 仅保留可比较的槽身份。
        .map(|dependency| dependency.slot_id)
        // 汇总为集合以避免顺序影响断言。
        .collect();
    // 确认外层完整保留了 panic 前后的依赖而没有混入内层依赖。
    assert_eq!(collected_outer_slots, expected_outer_slots);

    // 在没有活跃收集器时执行普通读取。
    let _ = ordinary_read.get();
    // 确认普通读取没有向线程局部追踪器遗留依赖。
    TRACKING_DEPS.with(|deps| assert!(deps.borrow().is_none()));
    // 通过新的收集周期观察普通读取只登记在新的收集器内。
    let (_, ordinary_dependencies) = collect_deps(|| ordinary_read.get());
    // 投影后续收集周期的槽身份。
    let ordinary_slots: HashSet<StateSlotId> = ordinary_dependencies
        // 逐项读取依赖保存的状态槽身份。
        .into_iter()
        // 仅保留可比较的槽身份。
        .map(|dependency| dependency.slot_id)
        // 汇总为集合以便精确比较。
        .collect();
    // 确认后续收集器只包含其自身的普通读取。
    assert_eq!(ordinary_slots, HashSet::from([ordinary_read.slot_id()]));
}

// 验证最外层依赖追踪自身 panic 后也会清空线程局部上下文。
#[test]
// 执行最外层 panic 后清空追踪器的回归。
fn capture_runtime_collect_deps_clears_tracker_after_outer_panic() {
    // 创建会在失败闭包中读取的状态。
    let failed_read = State::new(1_u8);
    // 捕获最外层依赖追踪闭包的预期 panic。
    let result = catch_unwind(AssertUnwindSafe(|| {
        // 创建会在执行过程中 unwind 的依赖收集器。
        let _ = collect_deps(|| {
            // 在 panic 前登记一个依赖。
            let _ = failed_read.get();
            // 模拟用户计算函数失败。
            panic!("outer dependency tracking panic");
        });
    }));
    // 确认测试确实走过受控 panic 路径。
    assert!(result.is_err());
    // 确认异常路径没有留下不可见的线程局部收集器。
    TRACKING_DEPS.with(|deps| assert!(deps.borrow().is_none()));
}

// 验证 Effect 析构会立即注销 State 内部订阅而无需再次写入状态。
#[test]
// 覆盖订阅租约的确定性析构生命周期。
fn effect_subscription_drop_removes_state_subscriber_immediately() {
    // 创建将被 Effect 直接读取的 State。
    let state = State::new(1_u8);
    // 克隆 State 所有权供初始副作用闭包读取。
    let observed_state = state.clone();
    // 创建并保留一个直接依赖该 State 的 Effect。
    let effect = Effect::new(move || {
        // 读取 State 以建立自动订阅。
        let _ = observed_state.get();
    });
    // 确认活跃 Effect 已在 State 私有注册表中留下一个订阅。
    assert_eq!(state.effect_subscriber_count(), 1);
    // 释放唯一 Effect 所有者以触发私有租约析构。
    drop(effect);
    // 无需额外写入 State 也必须立即移除订阅。
    assert_eq!(state.effect_subscriber_count(), 0);
}

// 验证动态读取切换会释放旧 State 并仅让新 State 触发 pending。
#[test]
// 覆盖每轮依赖去重、差集订阅与旧租约释放。
fn effect_subscription_dynamic_switch_releases_old_state_and_keeps_new_state() {
    // 创建控制 Effect 读取分支的 State。
    let choose_a = State::new(true);
    // 创建 A 分支的直接 State 依赖。
    let state_a = State::new(10_u8);
    // 创建 B 分支的直接 State 依赖。
    let state_b = State::new(20_u8);
    // 克隆三份状态所有权供 Effect 闭包跨轮读取。
    let effect_choose_a = choose_a.clone();
    // 克隆 A 状态供分支读取。
    let effect_state_a = state_a.clone();
    // 克隆 B 状态供分支读取。
    let effect_state_b = state_b.clone();
    // 创建会按选择器动态切换依赖的 Effect。
    let effect = Effect::new(move || {
        // 读取选择器以跟踪分支变化。
        if effect_choose_a.get() {
            // 仅在 A 分支读取 A State。
            let _ = effect_state_a.get();
        } else {
            // 仅在 B 分支读取 B State。
            let _ = effect_state_b.get();
        }
    });
    // 初始分支只订阅 A。
    assert_eq!(state_a.effect_subscriber_count(), 1);
    // 初始分支不订阅 B。
    assert_eq!(state_b.effect_subscriber_count(), 0);
    // 切换到 B 分支以触发下一轮 Effect。
    choose_a.set(false);
    // 执行切换分支对应的 Effect 重捕获。
    assert!(effect.tick());
    // 旧 A 租约必须在本轮 diff 中被立即释放。
    assert_eq!(state_a.effect_subscriber_count(), 0);
    // 新 B State 必须获得唯一订阅。
    assert_eq!(state_b.effect_subscriber_count(), 1);
    // 写入旧 A 不应再产生 pending。
    state_a.set(11);
    // 确认旧 State 已不会唤醒 Effect。
    assert!(!effect.has_pending());
    // 写入新 B 应通知 Effect。
    state_b.set(21);
    // 确认新 State 的订阅仍有效。
    assert!(effect.has_pending());
    // 消费 B 写入对应的一轮更新。
    assert!(effect.tick());
    // 反复切换验证订阅数始终受当前依赖集合约束。
    for expect_a in [true, false, true, false] {
        // 更新选择器触发动态依赖重建。
        choose_a.set(expect_a);
        // 执行本次选择器变更。
        assert!(effect.tick());
        // A 的注册数只能为当前分支对应的零或一。
        assert_eq!(
            state_a.effect_subscriber_count(),
            if expect_a { 1 } else { 0 }
        );
        // B 的注册数只能为当前分支对应的零或一。
        assert_eq!(
            state_b.effect_subscriber_count(),
            if expect_a { 0 } else { 1 }
        );
    }
}

// 验证两个 Effect 同时订阅一个 State 时可独立释放。
#[test]
// 覆盖同一 State 的多订阅者租约隔离。
fn effect_subscription_dropping_one_effect_keeps_other_effect_active() {
    // 创建会被两个 Effect 读取的共享 State。
    let state = State::new(1_u8);
    // 克隆第一个闭包需要的 State 所有权。
    let first_state = state.clone();
    // 创建第一个订阅者。
    let first = Effect::new(move || {
        // 读取共享 State 建立第一份订阅。
        let _ = first_state.get();
    });
    // 克隆第二个闭包需要的 State 所有权。
    let second_state = state.clone();
    // 创建第二个独立订阅者。
    let second = Effect::new(move || {
        // 读取共享 State 建立第二份订阅。
        let _ = second_state.get();
    });
    // 确认 State 注册表保存两份独立租约。
    assert_eq!(state.effect_subscriber_count(), 2);
    // 释放第一份租约。
    drop(first);
    // 确认第二份订阅未被同槽旧租约误删。
    assert_eq!(state.effect_subscriber_count(), 1);
    // 写入共享 State 通知仍存活的第二个 Effect。
    state.set(2);
    // 确认第二个 Effect 保留 pending。
    assert!(second.has_pending());
}

// 验证观察 generation 与完成订阅之间的 State 写入不会被遗漏。
#[test]
// 覆盖先注册再核对 generation 的竞争补偿。
fn effect_subscription_state_write_between_observe_and_subscribe_sets_pending() {
    // 创建初始 generation 为零的 State。
    let state = State::new(0_u8);
    // 克隆 State 所有权供初始 Effect 执行使用。
    let effect_state = state.clone();
    // 在首次读取之后、订阅建立之前同步写入 State。
    let effect = Effect::new(move || {
        // 先捕获 generation 为零的依赖快照。
        let _ = effect_state.get();
        // 将 State 推进到新的 generation。
        effect_state.set(1);
    });
    // 注册后核对必须保留这次竞争写入的 pending 信号。
    assert!(effect.has_pending());
}

// 验证同一 Effect 的并发 tick 不会并行执行，且运行中通知保留下一轮。
#[test]
// 覆盖 running RAII、竞争 tick 与运行期 State 写入。
fn effect_subscription_concurrent_tick_is_serial_and_retains_running_notification() {
    // 创建会唤醒 Effect 的直接 State 依赖。
    let state = State::new(0_u8);
    // 创建供 Effect 闭包读取的 State 克隆。
    let effect_state = state.clone();
    // 控制仅在下一次实际 tick 执行时阻塞用户闭包。
    let block_next = Arc::new(AtomicBool::new(false));
    // 克隆阻塞开关供用户闭包读取。
    let effect_block_next = block_next.clone();
    // 创建用户闭包进入后的单向超时收据通道。
    let (entered_tx, entered_rx) = mpsc::channel();
    // 克隆进入收据发送端供用户闭包报告已开始运行。
    let effect_entered_tx = entered_tx.clone();
    // 创建主线程释放用户闭包的单向通道。
    let (release_tx, release_rx) = mpsc::channel();
    // 使用互斥包装接收端以满足 Effect 闭包的 Sync 契约。
    let effect_release_rx = Arc::new(Mutex::new(release_rx));
    // 克隆受互斥保护的接收端供用户闭包有限等待。
    let effect_release_rx = effect_release_rx.clone();
    // 记录用户闭包是否在超时前收到主线程释放收据。
    let release_received = Arc::new(AtomicBool::new(false));
    // 克隆释放收据标志供用户闭包更新。
    let effect_release_received = release_received.clone();
    // 记录同时进入用户闭包的最大并发数。
    let active = Arc::new(AtomicUsize::new(0));
    // 克隆活动计数供用户闭包更新。
    let effect_active = active.clone();
    // 记录历史最大并发数。
    let maximum = Arc::new(AtomicUsize::new(0));
    // 克隆最大值供用户闭包更新。
    let effect_maximum = maximum.clone();
    // 创建会在受控 tick 中阻塞的 Effect。
    let effect = Arc::new(Effect::new(move || {
        // 建立直接 State 依赖。
        let _ = effect_state.get();
        // 记录本次用户闭包进入数量。
        let now = effect_active.fetch_add(1, Ordering::AcqRel) + 1;
        // 以无锁方式提高观察到的最大并发数。
        effect_maximum.fetch_max(now, Ordering::AcqRel);
        // 仅让指定的下一次执行与主线程同步。
        if effect_block_next.swap(false, Ordering::AcqRel) {
            // 向主线程发送用户闭包已在锁外开始运行的收据。
            effect_entered_tx
                .send(())
                .expect("进入收据接收端不应提前关闭");
            // 有限等待主线程完成竞争 tick 与运行期写入后的释放收据。
            let released = effect_release_rx
                .lock()
                .expect("释放接收端互斥锁不应中毒")
                .recv_timeout(Duration::from_secs(1))
                .is_ok();
            // 保存有限等待的收据结果供主线程断言。
            effect_release_received.store(released, Ordering::Release);
        }
        // 离开用户闭包前减少活动计数。
        effect_active.fetch_sub(1, Ordering::AcqRel);
    }));
    // 请求下一次 tick 在用户闭包内阻塞。
    block_next.store(true, Ordering::Release);
    // 写入 State 触发实际重新执行。
    state.set(1);
    // 克隆 Arc 所有权供后台竞争 tick 使用。
    let first_effect = effect.clone();
    // 启动持有本轮运行权的后台 tick。
    let first_tick = std::thread::spawn(move || first_effect.tick());
    // 有限等待后台 tick 确认已经进入用户闭包。
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("后台 tick 未在时限内进入用户闭包");
    // 竞争 tick 不能获得运行权，也不能清除 pending。
    assert!(!effect.tick());
    // 在第一轮用户闭包运行中再次写入 State。
    state.set(2);
    // 确认运行期通知仍被保留给后续 tick。
    assert!(effect.has_pending());
    // 向后台用户闭包发送释放收据。
    release_tx
        .send(())
        .expect("后台 tick 的释放接收端不应提前关闭");
    // 确认第一轮实际执行完成。
    assert!(first_tick.join().expect("后台 tick 不应 panic"));
    // 确认后台用户闭包在时限内收到主线程释放收据。
    assert!(release_received.load(Ordering::Acquire));
    // 确认用户闭包从未并行运行。
    assert_eq!(maximum.load(Ordering::Acquire), 1);
    // 消费运行期保留的 pending，并执行遗漏写入对应的下一轮副作用。
    assert!(effect.tick());
    // 确认下一轮已更新 generation 快照而不再重复执行。
    assert!(!effect.tick());
}

// 验证公开 watcher 可重入写入同一 State 而不持有 State 锁。
#[test]
// 覆盖 State watcher 的锁外回调与可重入语义。
fn effect_subscription_public_watch_callback_can_reenter_without_deadlock() {
    // 创建公开 watcher 将重入写入的 State。
    let state = State::new(0_u8);
    // 克隆 State 所有权供 watcher 回调重新写入。
    let callback_state = state.clone();
    // 注册只在第一次写入时重入的公开 watcher。
    state.watch(move |value| {
        // 避免第二次通知再次触发无限递归。
        if *value == 1 {
            // 在回调中安全地重入 State 写入。
            callback_state.set(2);
        }
    });
    // 创建公开 watcher 重入写入完成后的超时收据通道。
    let (completed_tx, completed_rx) = mpsc::channel();
    // 克隆 State 所有权供受控后台写入使用。
    let setter_state = state.clone();
    // 启动可能暴露重入死锁的后台首次写入。
    let setter = std::thread::spawn(move || {
        // 触发公开 watcher 的首次通知。
        setter_state.set(1);
        // 向主线程报告重入写入已经返回。
        completed_tx.send(()).expect("完成收据接收端不应提前关闭");
    });
    // 有限等待公开 watcher 重入写入返回，避免回归无限挂起测试。
    completed_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("公开 watcher 重入写入未在时限内返回");
    // 确认后台写入线程未 panic。
    setter.join().expect("公开 watcher 重入线程不应 panic");
    // 确认回调已完成可重入写入而未发生死锁。
    assert_eq!(state.get(), 2);
}

// 验证 State 值锁内的 update 闭包可安全释放最后一个同 State Effect。
#[test]
// 覆盖值锁与独立 Effect 注册表不嵌套的析构路径。
fn effect_subscription_update_can_drop_last_same_state_effect_without_deadlock() {
    // 创建将由 Effect 读取并由 update 修改的 State。
    let state = State::new(0_u8);
    // 克隆 State 所有权供 Effect 闭包建立同 State 订阅。
    let effect_state = state.clone();
    // 创建唯一持有的同 State Effect，避免测试形成永久 Arc 循环。
    let effect = Effect::new(move || {
        // 读取 State 以建立需要在 update 中注销的订阅。
        let _ = effect_state.get();
    });
    // 确认租约已经登记到 State 的独立注册表。
    assert_eq!(state.effect_subscriber_count(), 1);
    // 创建 update 析构路径完成后的超时收据通道。
    let (completed_tx, completed_rx) = mpsc::channel();
    // 克隆 State 所有权供受控后台 update 使用。
    let update_state = state.clone();
    // 启动会在持有值锁时析构最后一个 Effect 的后台 update。
    let update = std::thread::spawn(move || {
        // 在持有 State 值写锁的 update 闭包中释放最后一个 Effect。
        update_state.update(move |value| {
            // 仍完成一次普通值修改以覆盖实际 update 路径。
            *value = 1;
            // 释放唯一 Effect 并要求租约注销不重入 State 值锁。
            drop(effect);
        });
        // 向主线程报告析构路径已经返回。
        completed_tx.send(()).expect("完成收据接收端不应提前关闭");
    });
    // 有限等待 update 析构路径返回，避免死锁回归永久挂起测试。
    completed_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("update 内析构最后一个 Effect 未在时限内返回");
    // 确认后台 update 线程未 panic。
    update.join().expect("update 析构线程不应 panic");
    // 确认 update 返回且独立注册表中不再保留订阅。
    assert_eq!(state.effect_subscriber_count(), 0);
}
