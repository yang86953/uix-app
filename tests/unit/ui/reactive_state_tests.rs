// 引入被测私有依赖收集器与状态类型。
use super::{collect_deps, State, StateSlotId, TRACKING_DEPS};
// 引入集合类型以便无序比较依赖槽身份。
use std::collections::HashSet;
// 引入受控 unwind 捕获工具以验证 panic 生命周期。
use std::panic::{catch_unwind, AssertUnwindSafe};

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
