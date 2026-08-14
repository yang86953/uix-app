// 引入组件状态捕获与存储的私有测试边界。
use super::{
    // 引入测试构造动态宿主身份所需的命名空间类型。
    ComponentStateCaptureNamespace,
    // 引入单树拥有的组件状态存储。
    ComponentStateStore,
    // 引入测试构造稳定身份所需的作用域。
    UixComponentScope,
    // 引入仅由事务协调器使用的批量回执接纳入口。
    accept_component_state_receipts,
    // 引入按最终挂载作用域解析回执批次的入口。
    resolve_component_state_receipts,
    // 引入动态节点稳定子作用域派生入口。
    uix_component_child_scope,
    // 引入测试生成声明作用域的代码生成器入口。
    uix_component_scope,
    // 引入代码生成器使用的状态取得入口。
    uix_component_state,
    // 引入带异常回滚的捕获事务入口。
    with_component_state_capture,
    // 引入动态宿主命名空间感知的捕获事务入口。
    with_component_state_capture_in_namespace,
};
// 引入动态命名空间宿主的稳定组件身份。
use crate::core::ComponentId;
// 引入单线程测试记录器。
use std::cell::Cell;
// 引入可验证原始 panic 语义的展开边界。
use std::panic::{AssertUnwindSafe, catch_unwind};

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
        // 手工构造的静态作用域不附加动态实例命名空间。
        namespace: None,
        // 测试根作用域不包含组件内部动态节点路径。
        dynamic_path: Vec::new(),
    }
}

// 验证动态节点子作用域同时受组件实例、静态声明与实际 key 隔离。
#[test]
fn dynamic_child_scope_uses_declaration_and_stable_key_identity() {
    // 创建同一组件实例的稳定父作用域。
    let parent = test_scope(91);
    // 相同声明与 key 必须跨 reconcile 派生相等身份。
    let first = uix_component_child_scope(&parent, 7, "row-a");
    // 重复派生同一实际节点身份。
    let repeated = uix_component_child_scope(&parent, 7, "row-a");
    // 相同实际节点必须复用状态作用域。
    assert_eq!(first, repeated);
    // key 变化必须形成不同作用域并释放旧实例状态。
    let changed_key = uix_component_child_scope(&parent, 7, "row-b");
    // 不同 key 不得共享动态样式状态。
    assert_ne!(first, changed_key);
    // 节点类型或静态位置变化通过声明标识形成不同作用域。
    let changed_declaration = uix_component_child_scope(&parent, 8, "row-a");
    // 不同声明不得复活旧动态样式状态。
    assert_ne!(first, changed_declaration);
}

// 构造带固定静态槽位的动态实例命名空间。
fn test_namespace(owner_slot: usize, stable_key: &str) -> ComponentStateCaptureNamespace {
    // 使用不同组件槽位模拟不同动态宿主。
    ComponentStateCaptureNamespace::new(ComponentId::new(owner_slot), "dynamic-row", stable_key)
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
    let (existing_after, existing_after_receipt) =
        with_component_state_capture(store.clone(), || {
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
    let (reinitialized, reinitialized_receipt) =
        with_component_state_capture(store.clone(), || {
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
    assert_ne!(
        reinitialized.slot_id(),
        failed_slot.get().expect("失败槽应已记录")
    );
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
    let ((outer, outer_followup), outer_receipt) =
        with_component_state_capture(store.clone(), || {
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
    assert_eq!(
        nested_after.slot_id(),
        nested_slot.get().expect("嵌套槽应已记录")
    );
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

// 验证同一静态声明在同一宿主的不同业务键下隔离状态。
#[test]
// 执行动态命名空间业务键隔离回归。
fn dynamic_namespace_different_keys_isolate_same_static_scope() {
    // 创建供两个动态实例共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建同一宿主的第一个稳定业务实例命名空间。
    let first_namespace = test_namespace(31, "first-key");
    // 创建同一宿主的第二个稳定业务实例命名空间。
    let second_namespace = test_namespace(31, "second-key");
    // 在第一个动态实例内建立相同静态声明的字段。
    let ((first_scope, first), first_receipt) = with_component_state_capture_in_namespace(
        // 复用同一窗口存储以排除存储边界影响。
        store.clone(),
        // 传入第一个业务键命名空间。
        first_namespace,
        // 构建第一个动态实例的声明作用域与字段。
        || {
            // 申请与第二个实例完全相同的静态声明身份。
            let scope = uix_component_scope("dynamic-keyed-callsite", 71);
            // 建立第一实例的同一字段号状态。
            let state = uix_component_state(&scope, 1, || 101_i32);
            // 返回作用域与状态以验证完整身份。
            (scope, state)
        },
    );
    // 让第一实例的新增字段成为已挂载状态。
    accept_component_state_receipts(vec![first_receipt]);
    // 在第二个动态实例内重复相同静态声明与字段号。
    let ((second_scope, second), second_receipt) = with_component_state_capture_in_namespace(
        // 复用相同窗口存储以只改变业务键维度。
        store,
        // 传入不同业务键的同宿主命名空间。
        second_namespace,
        // 构建第二个动态实例的声明作用域与字段。
        || {
            // 重复第一个实例的静态调用点与声明号。
            let scope = uix_component_scope("dynamic-keyed-callsite", 71);
            // 重复第一实例的字段号但使用不同初始值。
            let state = uix_component_state(&scope, 1, || 202_i32);
            // 返回作用域与状态以验证完整身份。
            (scope, state)
        },
    );
    // 让第二实例的新增字段成为已挂载状态。
    accept_component_state_receipts(vec![second_receipt]);
    // 不同业务键必须生成不同的完整组件作用域。
    assert_ne!(first_scope, second_scope);
    // 不同业务键不得复用同一个响应式状态槽。
    assert_ne!(first.slot_id(), second.slot_id());
    // 第一实例必须保留自己的初始值。
    assert_eq!(first.get(), 101_i32);
    // 第二实例必须保留自己的初始值。
    assert_eq!(second.get(), 202_i32);
}

// 验证相同动态业务键跨捕获复用已挂载状态。
#[test]
// 执行动态命名空间稳定键复用回归。
fn dynamic_namespace_same_key_reuses_state_across_captures() {
    // 创建两次捕获共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建将在两次捕获中重建的稳定动态命名空间。
    let namespace = test_namespace(41, "reused-key");
    // 在首次动态捕获中建立状态字段。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 提供窗口唯一的状态存储。
        store.clone(),
        // 提供首次构造的稳定命名空间。
        namespace.clone(),
        // 构建首次动态实例状态。
        || {
            // 申请固定的静态声明身份。
            let scope = uix_component_scope("dynamic-reuse-callsite", 72);
            // 建立将由后续捕获复用的字段。
            uix_component_state(&scope, 1, || 301_i32)
        },
    );
    // 接纳首次捕获的状态写入。
    accept_component_state_receipts(vec![first_receipt]);
    // 记录后备初始器是否被错误调用。
    let fallback_ran = Cell::new(false);
    // 在第二次动态捕获中请求相同身份的字段。
    let (second, second_receipt) = with_component_state_capture_in_namespace(
        // 复用同一个窗口存储。
        store,
        // 复用完全相同的动态命名空间。
        namespace,
        // 构建第二次动态实例状态查询。
        || {
            // 重复首次捕获的静态声明身份。
            let scope = uix_component_scope("dynamic-reuse-callsite", 72);
            // 请求同一字段并提供可观测后备初始器。
            uix_component_state(&scope, 1, || {
                // 标记不应发生的重新初始化。
                fallback_ran.set(true);
                // 返回仅在错误重建时可见的后备值。
                302_i32
            })
        },
    );
    // 接纳只读复用捕获以结束事务语义。
    accept_component_state_receipts(vec![second_receipt]);
    // 相同完整命名空间必须复用首次状态槽。
    assert_eq!(first.slot_id(), second.slot_id());
    // 已挂载状态必须保留首次初始值。
    assert_eq!(second.get(), 301_i32);
    // 复用路径不得执行后备初始器。
    assert!(!fallback_ran.get());
}

// 验证不同宿主和不同窗口存储均不共享动态状态。
#[test]
// 执行动态命名空间宿主与存储隔离回归。
fn dynamic_namespace_different_owners_and_stores_are_isolated() {
    // 创建第一个窗口树拥有的状态存储。
    let first_store = ComponentStateStore::new();
    // 创建第二个窗口树拥有的独立状态存储。
    let second_store = ComponentStateStore::new();
    // 创建第一个宿主的稳定动态命名空间。
    let first_namespace = test_namespace(51, "shared-key");
    // 创建第二个宿主的同业务键动态命名空间。
    let second_namespace = test_namespace(52, "shared-key");
    // 在第一个宿主与第一个存储中建立字段。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 使用第一个窗口树存储。
        first_store.clone(),
        // 使用第一个动态宿主。
        first_namespace.clone(),
        // 构建第一个动态状态字段。
        || {
            // 申请将由其余隔离场景重复的静态声明。
            let scope = uix_component_scope("dynamic-owner-callsite", 73);
            // 建立第一个宿主的状态值。
            uix_component_state(&scope, 1, || 401_i32)
        },
    );
    // 接纳第一个宿主的字段。
    accept_component_state_receipts(vec![first_receipt]);
    // 记录不同宿主是否独立执行初始化。
    let different_owner_initialized = Cell::new(false);
    // 在同一存储但不同宿主中请求同一静态字段。
    let (different_owner, different_owner_receipt) = with_component_state_capture_in_namespace(
        // 复用第一个窗口树存储以只改变宿主维度。
        first_store,
        // 切换至不同组件宿主。
        second_namespace,
        // 构建第二宿主的同静态字段。
        || {
            // 重复第一个宿主的静态声明。
            let scope = uix_component_scope("dynamic-owner-callsite", 73);
            // 以可观测初始器建立隔离字段。
            uix_component_state(&scope, 1, || {
                // 标记宿主隔离导致的新建。
                different_owner_initialized.set(true);
                // 返回第二宿主独有的状态值。
                402_i32
            })
        },
    );
    // 接纳第二宿主的字段。
    accept_component_state_receipts(vec![different_owner_receipt]);
    // 记录不同存储是否独立执行初始化。
    let different_store_initialized = Cell::new(false);
    // 在不同存储但相同宿主中请求同一静态字段。
    let (different_store, different_store_receipt) = with_component_state_capture_in_namespace(
        // 切换至另一个窗口树存储。
        second_store,
        // 保留第一个宿主命名空间以只改变存储维度。
        first_namespace,
        // 构建第二窗口树的同静态字段。
        || {
            // 重复第一个宿主的静态声明。
            let scope = uix_component_scope("dynamic-owner-callsite", 73);
            // 以可观测初始器建立另一存储的字段。
            uix_component_state(&scope, 1, || {
                // 标记存储隔离导致的新建。
                different_store_initialized.set(true);
                // 返回第二窗口树独有的状态值。
                403_i32
            })
        },
    );
    // 接纳第二窗口树的字段。
    accept_component_state_receipts(vec![different_store_receipt]);
    // 不同宿主不得复用第一个宿主的状态槽。
    assert_ne!(first.slot_id(), different_owner.slot_id());
    // 不同状态存储不得复用第一个窗口树的状态槽。
    assert_ne!(first.slot_id(), different_store.slot_id());
    // 不同宿主必须实际建立独立状态。
    assert!(different_owner_initialized.get());
    // 不同状态存储必须实际建立独立状态。
    assert!(different_store_initialized.get());
    // 第一个宿主必须保留自己的值。
    assert_eq!(first.get(), 401_i32);
    // 第二宿主必须保留自己的值。
    assert_eq!(different_owner.get(), 402_i32);
    // 第二窗口树必须保留自己的值。
    assert_eq!(different_store.get(), 403_i32);
}

// 验证动态捕获 panic 会恢复 TLS 并回滚命名空间字段。
#[test]
// 执行动态命名空间 panic 恢复回归。
fn dynamic_namespace_panic_restores_tls_and_rolls_back_state() {
    // 创建将经历异常与重试的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建将经历异常与重试的稳定动态命名空间。
    let namespace = test_namespace(61, "panic-key");
    // 在展开边界内执行会失败的动态捕获。
    let result = catch_unwind(AssertUnwindSafe(|| {
        // 在动态命名空间中建立字段后故意 panic。
        with_component_state_capture_in_namespace(
            // 传入将用于重试的窗口存储。
            store.clone(),
            // 传入将用于重试的动态命名空间。
            namespace.clone(),
            // 构建会在建立字段后失败的动态内容。
            || {
                // 申请动态命名空间内的静态声明身份。
                let scope = uix_component_scope("dynamic-panic-callsite", 74);
                // 建立应由 panic 回滚的状态字段。
                let _ = uix_component_state(&scope, 1, || 501_i32);
                // 模拟动态 View 构建失败。
                panic!("dynamic namespace failure");
            },
        );
    }));
    // 原始 panic 必须继续传播到调用方。
    assert!(result.is_err());
    // 在普通静态捕获中读取恢复后的作用域身份。
    let (static_scope, static_receipt) = with_component_state_capture(store.clone(), || {
        // 申请与失败捕获相同的静态声明以检查 TLS 是否泄漏命名空间。
        uix_component_scope("dynamic-panic-callsite", 74)
    });
    // 恢复后的静态根作用域不得保留失败动态命名空间。
    assert!(static_scope.namespace.is_none());
    // 丢弃静态查询回执以保持测试不接纳无节点根。
    drop(static_receipt);
    // 记录失败动态字段是否会被正确重新初始化。
    let reinitialized = Cell::new(false);
    // 在相同动态命名空间内重试失败前建立的字段。
    let (retry, retry_receipt) = with_component_state_capture_in_namespace(
        // 复用已恢复的同一窗口存储。
        store,
        // 复用失败前的同一动态实例身份。
        namespace,
        // 构建重试动态实例字段。
        || {
            // 重复失败前的静态声明身份。
            let scope = uix_component_scope("dynamic-panic-callsite", 74);
            // 请求字段并验证失败路径确实已经回滚。
            uix_component_state(&scope, 1, || {
                // 标记异常路径未泄漏状态。
                reinitialized.set(true);
                // 返回重试后的新初始值。
                502_i32
            })
        },
    );
    // 失败后的同身份重试必须重新执行初始器。
    assert!(reinitialized.get());
    // 重试状态必须采用新的初始值。
    assert_eq!(retry.get(), 502_i32);
    // 丢弃重试回执以验证未挂载构建不会残留状态。
    drop(retry_receipt);
}

// 从独立子文件挂载 provisional claim 生命周期门禁。
#[path = "component_state_tests/pending_claims.rs"]
// 隔离并发未提交捕获的状态槽认领协议测试。
mod pending_claims;
