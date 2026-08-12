// 引入组件状态捕获与存储的私有测试边界。
use super::{
    // 引入仅由事务协调器使用的批量回执接纳入口。
    accept_component_state_receipts,
    // 引入代码生成器使用的状态取得入口。
    uix_component_state,
    // 引入带异常回滚的捕获事务入口。
    with_component_state_capture,
    // 引入单树拥有的组件状态存储。
    ComponentStateStore,
    // 引入测试构造稳定身份所需的作用域。
    UixComponentScope,
};
// 引入单线程测试记录器。
use std::cell::Cell;
// 引入可验证原始 panic 语义的展开边界。
use std::panic::{catch_unwind, AssertUnwindSafe};

// 构造可跨多次捕获复用的确定组件作用域。
fn test_scope(declaration: u64) -> UixComponentScope {
    // 返回与代码生成身份形状一致的测试值。
    UixComponentScope {
        // 使用固定调用点便于不同捕获复用。
        callsite: "component-state-tests",
        // 用参数隔离不同测试作用域。
        declaration,
        // 单一测试实例始终使用第一次出现。
        occurrence: 0,
    }
}

// 验证成功捕获在挂载前丢弃回执会撤销新建状态。
#[test]
// 执行未提交回执的 Drop 回滚回归。
fn dropping_successful_capture_receipt_rolls_back_insertions() {
    // 创建未提交捕获独占的树私有存储。
    let store = ComponentStateStore::new();
    // 创建可在回滚后重新请求的稳定作用域。
    let scope = test_scope(10);
    // 正常结束捕获并保留尚未提交的回执。
    let (captured, receipt) = with_component_state_capture(store.clone(), || {
        // 在成功构建阶段新建一个状态槽。
        uix_component_state(&scope, 1, || 10_i32)
    });
    // 保存即将因未挂载而回滚的槽身份。
    let rolled_back_slot = captured.slot_id();
    // 模拟构建成功但挂载失败，直接丢弃回执。
    drop(receipt);
    // 记录同字段是否在后续捕获中重新初始化。
    let reinitialized = Cell::new(false);
    // 再次捕获同一作用域字段。
    let (next, next_receipt) = with_component_state_capture(store, || {
        // 新初始器必须在上次回执回滚后执行。
        uix_component_state(&scope, 1, || {
            // 标记字段已经重新初始化。
            reinitialized.set(true);
            // 返回重新建立的状态值。
            11_i32
        })
    });
    // 丢弃成功回执后同字段必须重新初始化。
    assert!(reinitialized.get());
    // 重建字段必须使用不同于已回滚槽的新身份。
    assert_ne!(next.slot_id(), rolled_back_slot);
    // 提交第二次捕获以结束测试的挂载语义。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![next_receipt]);
}

// 验证显式提交回执会保留成功挂载的状态槽。
#[test]
// 执行回执 commit 与后续槽复用回归。
fn committing_capture_receipt_preserves_insertions() {
    // 创建显式提交捕获独占的树私有存储。
    let store = ComponentStateStore::new();
    // 创建前后捕获共用的稳定作用域。
    let scope = test_scope(11);
    // 建立待挂载提交的状态字段。
    let (captured, receipt) = with_component_state_capture(store.clone(), || {
        // 返回初次建立的状态句柄。
        uix_component_state(&scope, 1, || 20_i32)
    });
    // 保存已挂载字段的槽身份。
    let committed_slot = captured.slot_id();
    // 模拟 WidgetTree 挂载成功并接受该 journal。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![receipt]);
    // 记录后续复用时的后备初始器是否被误执行。
    let initializer_ran = Cell::new(false);
    // 在后续捕获中请求同一字段。
    let (reused, reused_receipt) = with_component_state_capture(store, || {
        // 为已提交槽提供不应运行的后备初始器。
        uix_component_state(&scope, 1, || {
            // 标记错误的重复初始化。
            initializer_ran.set(true);
            // 返回不应被采用的值。
            21_i32
        })
    });
    // 显式提交的槽必须跨捕获保持身份。
    assert_eq!(reused.slot_id(), committed_slot);
    // 已提交槽不得再次执行初始器。
    assert!(!initializer_ran.get());
    // 提交后续只读复用捕获。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![reused_receipt]);
}

// 验证同存储嵌套捕获各自产生独立可撤销回执。
#[test]
// 模拟内层构建成功但整体挂载失败时两份回执均未提交。
fn nested_success_receipts_roll_back_independently_when_uncommitted() {
    // 创建内外层共用的树私有存储。
    let store = ComponentStateStore::new();
    // 创建内外层字段共用的稳定作用域。
    let scope = test_scope(12);
    // 正常结束外层与内层捕获并取回两份独立回执。
    let ((outer_state, inner_state, inner_receipt), outer_receipt) =
        // 开始外层捕获并建立它自己的字段。
        with_component_state_capture(store.clone(), || {
            // 在外层 journal 中建立第一字段。
            let outer_state = uix_component_state(&scope, 1, || 30_i32);
            // 临时替换外层上下文并建立内层字段。
            let (inner_state, inner_receipt) = with_component_state_capture(store.clone(), || {
                // 在内层独立 journal 中建立第二字段。
                uix_component_state(&scope, 2, || 31_i32)
            });
            // 返回两个句柄并把内层回执交给模拟挂载方。
            (outer_state, inner_state, inner_receipt)
        });
    // 保存将被独立回滚的外层槽身份。
    let outer_slot = outer_state.slot_id();
    // 保存将被独立回滚的内层槽身份。
    let inner_slot = inner_state.slot_id();
    // 模拟整体挂载失败时外层回执未提交。
    drop(outer_receipt);
    // 模拟整体挂载失败时内层回执也未提交。
    drop(inner_receipt);
    // 记录内外两个字段后续初始器的执行次数。
    let initialization_count = Cell::new(0_u32);
    // 再次捕获同一作用域中的两个字段。
    let ((next_outer, next_inner), next_receipt) = with_component_state_capture(store, || {
        // 重新建立已由外层回执撤销的第一字段。
        let next_outer = uix_component_state(&scope, 1, || {
            // 记录外层字段的重新初始化。
            initialization_count.set(initialization_count.get().saturating_add(1));
            // 返回新的外层状态值。
            32_i32
        });
        // 重新建立已由内层回执撤销的第二字段。
        let next_inner = uix_component_state(&scope, 2, || {
            // 记录内层字段的重新初始化。
            initialization_count.set(initialization_count.get().saturating_add(1));
            // 返回新的内层状态值。
            33_i32
        });
        // 返回两个重新建立的状态句柄。
        (next_outer, next_inner)
    });
    // 两个未提交回执必须使各自字段都重新初始化。
    assert_eq!(initialization_count.get(), 2);
    // 外层字段必须使用新槽身份。
    assert_ne!(next_outer.slot_id(), outer_slot);
    // 内层字段必须使用新槽身份。
    assert_ne!(next_inner.slot_id(), inner_slot);
    // 提交重建后的单层捕获以结束测试语义。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![next_receipt]);
}

// 验证异常捕获只撤销本层新字段并允许后续重建。
#[test]
// 执行已有字段与新字段混合的 panic 回滚回归。
fn panic_rolls_back_only_new_field_and_allows_reinitialization() {
    // 创建本测试独占的树私有存储。
    let store = ComponentStateStore::new();
    // 创建多次捕获共用的组件作用域。
    let scope = test_scope(1);
    // 先通过正常捕获提交一个已有字段。
    let (existing, existing_receipt) = with_component_state_capture(store.clone(), || {
        // 建立并返回第一个状态字段。
        uix_component_state(&scope, 1, || 11_i32)
    });
    // 模拟首次挂载成功并提交已有字段。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![existing_receipt]);
    // 保存已提交字段的槽身份。
    let existing_slot = existing.slot_id();
    // 记录失败捕获新建字段的槽身份。
    let failed_slot = Cell::new(None);
    // 记录已有字段的后备初始器是否被误执行。
    let existing_initializer_ran = Cell::new(false);
    // 捕获预期的用户构建 panic。
    let result = catch_unwind(AssertUnwindSafe(|| {
        // 在同一存储中执行会失败的第二次捕获。
        with_component_state_capture(store.clone(), || {
            // 请求已有字段时提供一个不应执行的初始器。
            let reused = uix_component_state(&scope, 1, || {
                // 标记错误的重复初始化。
                existing_initializer_ran.set(true);
                // 返回不应写入的后备值。
                99_i32
            });
            // 已有字段必须保持原槽身份。
            assert_eq!(reused.slot_id(), existing_slot);
            // 在失败捕获中建立一个新字段。
            let inserted = uix_component_state(&scope, 2, || 22_i32);
            // 保存待回滚的新槽身份。
            failed_slot.set(Some(inserted.slot_id()));
            // 触发捕获事务回滚。
            panic!("component state capture rollback");
        });
    }));
    // 捕获入口必须继续传播原始 panic。
    assert!(result.is_err());
    // 已有字段的初始器不得重复执行。
    assert!(!existing_initializer_ran.get());
    // 记录回滚后查询已有字段时的后备初始化。
    let existing_after_initializer_ran = Cell::new(false);
    // 再次捕获并查询捕获前已经存在的字段。
    let (existing_after, existing_after_receipt) = with_component_state_capture(store.clone(), || {
        // 请求同一字段并提供可观测后备初始器。
        uix_component_state(&scope, 1, || {
            // 标记不应发生的旧字段重建。
            existing_after_initializer_ran.set(true);
            // 返回不应被采用的后备值。
            101_i32
        })
    });
    // 提交只复用旧槽的查询捕获。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![existing_after_receipt]);
    // 回滚后已有字段仍必须使用原槽。
    assert_eq!(existing_after.slot_id(), existing_slot);
    // 回滚后已有字段不得重新初始化。
    assert!(!existing_after_initializer_ran.get());
    // 记录失败字段后续重新初始化的次数。
    let reinitialize_count = Cell::new(0_u32);
    // 对已回滚的同一作用域字段再次执行正常捕获。
    let (reinitialized, reinitialized_receipt) = with_component_state_capture(store.clone(), || {
        // 请求之前失败的第二个字段。
        uix_component_state(&scope, 2, || {
            // 记录该字段确实重新初始化一次。
            reinitialize_count.set(reinitialize_count.get().saturating_add(1));
            // 返回重新建立的状态值。
            33_i32
        })
    });
    // 模拟后续挂载成功并保留重建字段。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![reinitialized_receipt]);
    // 失败字段的初始器必须在后续捕获中执行一次。
    assert_eq!(reinitialize_count.get(), 1);
    // 重新建立的字段必须使用新槽身份。
    assert_ne!(reinitialized.slot_id(), failed_slot.get().expect("失败槽应已记录"));
    // 重新建立的字段必须保留新初始值。
    assert_eq!(reinitialized.get(), 33_i32);
}

// 验证内层捕获失败不会撤销外层已经插入的槽。
#[test]
// 执行同存储嵌套捕获的独立 journal 回归。
fn inner_panic_preserves_outer_insertions_and_capture() {
    // 创建嵌套捕获共用的树私有存储。
    let store = ComponentStateStore::new();
    // 创建内外层共用的稳定组件作用域。
    let scope = test_scope(2);
    // 记录内层捕获会被回滚的新槽。
    let failed_inner_slot = Cell::new(None);
    // 在外层捕获内建立状态并运行失败的内层捕获。
    let ((outer, outer_followup), outer_receipt) = with_component_state_capture(store.clone(), || {
        // 先将第一个字段登记到外层 journal。
        let outer = uix_component_state(&scope, 1, || 41_i32);
        // 保存外层新槽的身份以供内层核对。
        let outer_slot = outer.slot_id();
        // 捕获预期的内层构建 panic。
        let inner_result = catch_unwind(AssertUnwindSafe(|| {
            // 临时用新捕获替换线程当前的外层上下文。
            with_component_state_capture(store.clone(), || {
                // 内层查询外层已插入字段时必须只复用。
                let reused_outer = uix_component_state(&scope, 1, || 99_i32);
                // 复用的外层字段必须保持原槽身份。
                assert_eq!(reused_outer.slot_id(), outer_slot);
                // 在内层 journal 中建立独有的第二个字段。
                let inner_only = uix_component_state(&scope, 2, || 42_i32);
                // 保存内层独有字段的失败槽身份。
                failed_inner_slot.set(Some(inner_only.slot_id()));
                // 触发只针对内层 journal 的回滚。
                panic!("nested component state capture rollback");
            });
        }));
        // 内层捕获必须传播 panic 供外层决定处理。
        assert!(inner_result.is_err());
        // 内层返回后再次请求外层已插入字段。
        let outer_after_inner = uix_component_state(&scope, 1, || 100_i32);
        // 内层回滚不得删除外层 journal 中的槽。
        assert_eq!(outer_after_inner.slot_id(), outer_slot);
        // 在已恢复的外层捕获中重新建立第二字段。
        let outer_followup = uix_component_state(&scope, 2, || 43_i32);
        // 返回外层的两个已提交句柄。
        (outer, outer_followup)
    });
    // 外层构建成功后提交它自己的独立 journal。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![outer_receipt]);
    // 外层第一字段必须保留初始值。
    assert_eq!(outer.get(), 41_i32);
    // 外层后续字段必须使用不同于失败内层的新槽。
    assert_ne!(
        // 读取外层后续提交槽身份。
        outer_followup.slot_id(),
        // 读取已被回滚的内层槽身份。
        failed_inner_slot.get().expect("内层失败槽应已记录"),
    );
    // 外层恢复后建立的字段必须保留初始值。
    assert_eq!(outer_followup.get(), 43_i32);
}

// 验证初始器在存储锁外可以重入同一捕获。
#[test]
// 执行一个在初始值构造期间建立另一字段的回归。
fn initializer_can_reenter_same_store_without_losing_slots() {
    // 创建重入初始化独占的树私有存储。
    let store = ComponentStateStore::new();
    // 创建两个重入字段共用的稳定作用域。
    let scope = test_scope(3);
    // 记录初始器内建立字段的槽身份。
    let nested_slot = Cell::new(None);
    // 在单次捕获中执行可重入的外层字段初始化。
    let (outer, outer_receipt) = with_component_state_capture(store.clone(), || {
        // 初始化第一字段时重入建立第二字段。
        uix_component_state(&scope, 1, || {
            // 在未持有存储锁时建立嵌套字段。
            let nested = uix_component_state(&scope, 2, || 72_i32);
            // 保存嵌套字段的槽身份。
            nested_slot.set(Some(nested.slot_id()));
            // 返回外层字段的初始值。
            71_i32
        })
    });
    // 提交包含初始器重入插入的单层 journal。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![outer_receipt]);
    // 外层字段必须完成初始化而非死锁。
    assert_eq!(outer.get(), 71_i32);
    // 记录嵌套字段的后备初始器是否被误执行。
    let nested_initializer_ran = Cell::new(false);
    // 在新捕获中查询已提交的嵌套字段。
    let (nested_after, nested_after_receipt) = with_component_state_capture(store, || {
        // 请求第二字段并提供可观测后备初始器。
        uix_component_state(&scope, 2, || {
            // 标记不应发生的嵌套字段重建。
            nested_initializer_ran.set(true);
            // 返回不应被采用的后备值。
            73_i32
        })
    });
    // 提交只复用已有槽的查询捕获。
    // 以与树事务相同的批量入口接纳本次成功捕获。
    accept_component_state_receipts(vec![nested_after_receipt]);
    // 嵌套字段必须保留原始槽身份。
    assert_eq!(nested_after.slot_id(), nested_slot.get().expect("嵌套槽应已记录"));
    // 已提交嵌套字段不得重新初始化。
    assert!(!nested_initializer_ran.get());
}

// 验证内层成功事务的回执仍受外层 panic 回滚控制。
#[test]
// 执行嵌套树事务回执 checkpoint 的回滚回归。
fn nested_transaction_receipts_roll_back_with_outer_panic() {
    // 创建将由嵌套事务共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 用相同存储建立仅用于承载事务协调器的最小树。
    let mut tree = crate::ui::WidgetTree::with_component_state_store(store.clone());
    // 创建两个字段共用的稳定组件作用域。
    let scope = test_scope(18);
    // 捕获将被外层事务暂存的第一个新增字段。
    let (_, outer_receipt) = with_component_state_capture(store.clone(), || {
        // 建立只属于外层事务的字段。
        uix_component_state(&scope, 1, || 81_i32)
    });
    // 捕获外层事务随后 panic 的完整过程。
    let result = catch_unwind(AssertUnwindSafe(|| {
        // 让外层事务持有第一个 journal。
        tree.with_component_state_transaction(vec![outer_receipt], |tree| {
            // 捕获仅属于成功内层事务的第二个新增字段。
            let (_, inner_receipt) = with_component_state_capture(store.clone(), || {
                // 建立只属于内层事务的字段。
                uix_component_state(&scope, 2, || 82_i32)
            });
            // 正常结束内层事务，但不允许它提前接纳 journal。
            tree.with_component_state_transaction(vec![inner_receipt], |_| {});
            // 模拟内层成功后外层构建或协调失败。
            panic!("outer transaction failure");
        });
    }));
    // 外层失败必须继续向调用方传播。
    assert!(result.is_err());
    // 记录两个字段在回滚后是否重新执行初始器。
    let outer_reinitialized = Cell::new(false);
    // 记录内层字段在回滚后是否重新执行初始器。
    let inner_reinitialized = Cell::new(false);
    // 在同一窗口存储中再次请求两个字段。
    let ((outer_after, inner_after), retry_receipt) = with_component_state_capture(store, || {
        // 外层字段必须因回滚而重新初始化。
        let outer_after = uix_component_state(&scope, 1, || {
            // 标记外层字段确实没有残留。
            outer_reinitialized.set(true);
            // 返回重建后的外层值。
            91_i32
        });
        // 内层字段也必须因外层 panic 一并回滚。
        let inner_after = uix_component_state(&scope, 2, || {
            // 标记内层字段确实没有被提前接纳。
            inner_reinitialized.set(true);
            // 返回重建后的内层值。
            92_i32
        });
        // 返回两个重建字段供断言检查。
        (outer_after, inner_after)
    });
    // 外层字段必须重新初始化。
    assert!(outer_reinitialized.get());
    // 内层字段必须重新初始化。
    assert!(inner_reinitialized.get());
    // 外层字段必须使用新的初始化值。
    assert_eq!(outer_after.get(), 91_i32);
    // 内层字段必须使用新的初始化值。
    assert_eq!(inner_after.get(), 92_i32);
    // 丢弃重试回执以保持本测试只断言回滚而不承诺树半成品状态。
    drop(retry_receipt);
}
