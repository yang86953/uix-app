// 引入 provisional claim 门禁所需的父模块私有入口。
use super::{
    // 引入单树拥有的组件状态存储。
    ComponentStateStore,
    // 引入挂载事务批量接纳回执的入口。
    accept_component_state_receipts,
    // 引入按最终挂载作用域解析本批回执的入口。
    resolve_component_state_receipts,
    // 引入测试生成动态命名空间的辅助函数。
    test_namespace,
    // 引入代码生成器使用的作用域入口。
    uix_component_scope,
    // 引入代码生成器使用的状态入口。
    uix_component_state,
    // 引入动态命名空间捕获入口。
    with_component_state_capture_in_namespace,
};
// 引入可观测初始化路径的单线程记录器。
use std::cell::Cell;
// 引入构造最终挂载作用域真相所需的集合。
use std::collections::HashSet;

// 验证后续捕获的 claim 可保护它复用的 provisional 槽。
#[test]
// 执行创建者回执丢弃而复用者回执接纳的生命周期回归。
fn provisional_slot_survives_creator_drop_when_reuser_is_accepted() {
    // 创建三次动态捕获共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建三次捕获复用的稳定动态命名空间。
    let namespace = test_namespace(71, "provisional-shared-key");
    // 捕获 A 首次建立尚未提交的动态状态槽。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建捕获 A 的固定静态字段。
        || {
            // 申请后续捕获将重复的静态声明身份。
            let scope = uix_component_scope("provisional-claim-callsite", 81);
            // 建立处于 provisional 阶段的初始状态。
            uix_component_state(&scope, 1, || 601_i32)
        },
    );
    // 捕获 B 在 A 回执仍未决时复用同一个状态槽。
    let (second, second_receipt) = with_component_state_capture_in_namespace(
        // 复用同一窗口状态存储。
        store.clone(),
        // 复用完全相同的动态实例身份。
        namespace.clone(),
        // 构建捕获 B 的相同静态字段。
        || {
            // 重复捕获 A 的静态声明身份。
            let scope = uix_component_scope("provisional-claim-callsite", 81);
            // 请求相同字段并提供不应执行的后备初始值。
            uix_component_state(&scope, 1, || 602_i32)
        },
    );
    // 捕获 B 必须复用捕获 A 创建的 provisional 槽。
    assert_eq!(first.slot_id(), second.slot_id());
    // 丢弃捕获 A 的 claim，但捕获 B 的 claim 必须继续保护该槽。
    drop(first_receipt);
    // 接纳捕获 B，使它复用的 provisional 槽转为 committed。
    accept_component_state_receipts(vec![second_receipt]);
    // 记录第三次捕获是否错误地重新初始化状态。
    let fallback_ran = Cell::new(false);
    // 第三次捕获查询已经由 B 提交的同一动态状态槽。
    let (third, third_receipt) = with_component_state_capture_in_namespace(
        // 继续复用同一窗口状态存储。
        store,
        // 继续复用相同动态实例身份。
        namespace,
        // 构建第三次捕获的相同静态字段。
        || {
            // 重复前两次捕获的静态声明身份。
            let scope = uix_component_scope("provisional-claim-callsite", 81);
            // 请求已提交字段并提供可观测后备初始器。
            uix_component_state(&scope, 1, || {
                // 标记不应发生的状态重建。
                fallback_ran.set(true);
                // 返回仅在协议失效时可见的后备值。
                603_i32
            })
        },
    );
    // 第三次捕获必须继续复用最初创建的槽。
    assert_eq!(third.slot_id(), first.slot_id());
    // 已提交槽必须保留首次初始化值。
    assert_eq!(third.get(), 601_i32);
    // 复用已提交槽不得执行后备初始器。
    assert!(!fallback_ran.get());
    // 接纳第三次只读复用捕获以结束事务语义。
    accept_component_state_receipts(vec![third_receipt]);
}

// 验证所有 provisional claim 都释放后状态槽会被清理。
#[test]
// 执行两个未提交复用捕获全部丢弃后的重新初始化回归。
fn provisional_slot_is_removed_after_all_claims_are_dropped() {
    // 创建三次动态捕获共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建三次捕获复用的稳定动态命名空间。
    let namespace = test_namespace(72, "provisional-dropped-key");
    // 捕获 A 首次建立尚未提交的动态状态槽。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建捕获 A 的固定静态字段。
        || {
            // 申请后续捕获将重复的静态声明身份。
            let scope = uix_component_scope("provisional-drop-callsite", 82);
            // 建立处于 provisional 阶段的初始状态。
            uix_component_state(&scope, 1, || 701_i32)
        },
    );
    // 捕获 B 在 A 回执仍未决时复用同一个状态槽。
    let (second, second_receipt) = with_component_state_capture_in_namespace(
        // 复用同一窗口状态存储。
        store.clone(),
        // 复用完全相同的动态实例身份。
        namespace.clone(),
        // 构建捕获 B 的相同静态字段。
        || {
            // 重复捕获 A 的静态声明身份。
            let scope = uix_component_scope("provisional-drop-callsite", 82);
            // 请求相同字段并提供不应执行的后备初始值。
            uix_component_state(&scope, 1, || 702_i32)
        },
    );
    // 捕获 B 必须复用捕获 A 创建的 provisional 槽。
    assert_eq!(first.slot_id(), second.slot_id());
    // 先释放创建者 claim，此时复用者仍保护槽。
    drop(first_receipt);
    // 再释放最后一个复用者 claim，使未提交槽可被清理。
    drop(second_receipt);
    // 记录第三次捕获是否正确重新初始化状态。
    let reinitialized = Cell::new(false);
    // 第三次捕获查询已失去全部 claim 的同一动态字段。
    let (third, third_receipt) = with_component_state_capture_in_namespace(
        // 继续复用同一窗口状态存储。
        store,
        // 继续复用相同动态实例身份。
        namespace,
        // 构建第三次捕获的相同静态字段。
        || {
            // 重复前两次捕获的静态声明身份。
            let scope = uix_component_scope("provisional-drop-callsite", 82);
            // 重新建立已失去全部 claim 的字段。
            uix_component_state(&scope, 1, || {
                // 标记 provisional 槽确实已经被清理。
                reinitialized.set(true);
                // 返回重新初始化后的状态值。
                703_i32
            })
        },
    );
    // 所有旧 claim 释放后必须执行重新初始化。
    assert!(reinitialized.get());
    // 重新初始化必须获得不同于旧 provisional 槽的新身份。
    assert_ne!(third.slot_id(), first.slot_id());
    // 新槽必须采用第三次捕获的初始值。
    assert_eq!(third.get(), 703_i32);
    // 丢弃第三次回执以保持测试不承诺未挂载状态。
    drop(third_receipt);
}

// 验证成功事务中没有节点标记承载的作用域不会被提交。
#[test]
// 执行无最终 scope marker 的回执释放回归。
fn successful_transaction_without_scope_marker_releases_receipt_claim() {
    // 创建失败挂载与重试共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建两次捕获复用的稳定动态命名空间。
    let namespace = test_namespace(73, "missing-marker-key");
    // 捕获一个最终不会由节点标记承载的状态槽。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建没有最终节点标记的状态字段。
        || {
            // 申请未挂载组件的静态声明身份。
            let scope = uix_component_scope("missing-marker-callsite", 83);
            // 建立本批回执暂时认领的状态。
            uix_component_state(&scope, 1, || 801_i32)
        },
    );
    // 构造最终树不含任何组件作用域标记的真相。
    let live_scopes = HashSet::new();
    // 解析成功事务回执，但不得提交没有 marker 的状态槽。
    resolve_component_state_receipts(&store, vec![first_receipt], &live_scopes);
    // 记录后续同身份捕获是否重新初始化。
    let reinitialized = Cell::new(false);
    // 在同一动态身份下重新请求未挂载字段。
    let (retry, retry_receipt) = with_component_state_capture_in_namespace(
        // 复用同一窗口状态存储。
        store,
        // 复用同一动态实例身份。
        namespace,
        // 构建重试状态字段。
        || {
            // 重复未挂载捕获的静态声明身份。
            let scope = uix_component_scope("missing-marker-callsite", 83);
            // 请求字段并观测旧槽是否已经被释放。
            uix_component_state(&scope, 1, || {
                // 标记最终缺席回执没有泄漏状态。
                reinitialized.set(true);
                // 返回重试后的新初始值。
                802_i32
            })
        },
    );
    // 最终没有 marker 的状态必须重新初始化。
    assert!(reinitialized.get());
    // 重试必须取得不同于未挂载槽的新身份。
    assert_ne!(retry.slot_id(), first.slot_id());
    // 重试必须采用新的初始值。
    assert_eq!(retry.get(), 802_i32);
    // 丢弃重试回执以保持测试不挂载重试结果。
    drop(retry_receipt);
}

// 验证本批缺席回执不会删除同槽的外部 pending claim。
#[test]
// 执行同槽外部 claim 与无 marker 本批回执的隔离回归。
fn absent_batch_receipt_does_not_delete_external_pending_claim() {
    // 创建外部捕获与本批捕获共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建所有捕获复用的稳定动态命名空间。
    let namespace = test_namespace(74, "external-claim-key");
    // 建立先于当前树事务存在的外部 pending 捕获。
    let ((scope, external), external_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建外部捕获的状态字段。
        || {
            // 申请后续本批捕获将重复的静态声明身份。
            let scope = uix_component_scope("external-claim-callsite", 84);
            // 建立由外部 receipt 保护的 provisional 状态。
            let state = uix_component_state(&scope, 1, || 901_i32);
            // 返回作用域以便后续模拟实际挂载。
            (scope, state)
        },
    );
    // 当前树事务捕获并复用同一个 provisional 槽。
    let (batch, batch_receipt) = with_component_state_capture_in_namespace(
        // 复用同一窗口状态存储。
        store.clone(),
        // 复用同一动态实例身份。
        namespace.clone(),
        // 构建当前本批的相同状态字段。
        || {
            // 重复外部捕获的静态声明身份。
            let scope = uix_component_scope("external-claim-callsite", 84);
            // 请求同槽并提供不应执行的后备值。
            uix_component_state(&scope, 1, || 902_i32)
        },
    );
    // 当前本批必须复用外部捕获创建的槽。
    assert_eq!(batch.slot_id(), external.slot_id());
    // 构造当前本批最终没有 marker 的树真相。
    let absent_scopes = HashSet::new();
    // 释放本批 claim，但外部 pending claim 必须继续保护同槽。
    resolve_component_state_receipts(&store, vec![batch_receipt], &absent_scopes);
    // 构造外部捕获随后真正挂载的最终作用域集合。
    let mut mounted_scopes = HashSet::new();
    // 把外部捕获的作用域登记为实际节点标记。
    mounted_scopes.insert(scope);
    // 接纳外部回执并按最终挂载真相提交原槽。
    resolve_component_state_receipts(&store, vec![external_receipt], &mounted_scopes);
    // 第三次捕获查询已由外部回执提交的状态。
    let (third, third_receipt) = with_component_state_capture_in_namespace(
        // 继续复用同一窗口状态存储。
        store,
        // 继续复用同一动态实例身份。
        namespace,
        // 构建第三次相同状态字段。
        || {
            // 重复前两次捕获的静态声明身份。
            let scope = uix_component_scope("external-claim-callsite", 84);
            // 请求已挂载状态并提供不应执行的后备值。
            uix_component_state(&scope, 1, || 903_i32)
        },
    );
    // 外部 claim 必须使第三次捕获继续复用原槽。
    assert_eq!(third.slot_id(), external.slot_id());
    // 原槽必须保留首次初始化值。
    assert_eq!(third.get(), 901_i32);
    // 直接接纳第三次只读回执以结束测试事务语义。
    accept_component_state_receipts(vec![third_receipt]);
}

// 验证外部 pending 捕获可跨无关 prune 后再成功挂载。
#[test]
// 执行树外 claim 跨无关协调清理的存活回归。
fn external_pending_claim_survives_unrelated_prune_before_mount() {
    // 创建外部捕获与无关 prune 共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建外部捕获的稳定动态命名空间。
    let namespace = test_namespace(75, "pending-across-prune-key");
    // 建立尚未交给 WidgetTree 的外部 pending 捕获。
    let ((scope, first), first_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建外部捕获状态与作用域。
        || {
            // 申请后续挂载将承载的静态声明身份。
            let scope = uix_component_scope("pending-prune-callsite", 85);
            // 建立由树外 receipt 保护的状态槽。
            let state = uix_component_state(&scope, 1, || 1001_i32);
            // 返回作用域与状态以供挂载验证。
            (scope, state)
        },
    );
    // 构造一次与外部捕获无关的空树最终真相。
    let unrelated_live_scopes = HashSet::new();
    // 模拟其他 reconcile 完成后的 idle prune。
    store.retain_scopes(&unrelated_live_scopes);
    // 构造外部捕获随后真正挂载的作用域集合。
    let mut mounted_scopes = HashSet::new();
    // 登记外部捕获作用域对应的实际节点标记。
    mounted_scopes.insert(scope);
    // 在经过无关 prune 后接纳外部捕获回执。
    resolve_component_state_receipts(&store, vec![first_receipt], &mounted_scopes);
    // 再次捕获同一动态实例以验证原槽存活。
    let (second, second_receipt) = with_component_state_capture_in_namespace(
        // 继续复用同一窗口状态存储。
        store,
        // 继续复用同一动态实例身份。
        namespace,
        // 构建同一静态状态字段。
        || {
            // 重复已挂载捕获的静态声明身份。
            let scope = uix_component_scope("pending-prune-callsite", 85);
            // 请求已挂载状态并提供不应执行的后备值。
            uix_component_state(&scope, 1, || 1002_i32)
        },
    );
    // 无关 prune 不得删除外部 pending claim 保护的槽。
    assert_eq!(second.slot_id(), first.slot_id());
    // 挂载后必须保留首次初始化值。
    assert_eq!(second.get(), 1001_i32);
    // 直接接纳第二次只读回执以结束测试事务语义。
    accept_component_state_receipts(vec![second_receipt]);
}

// 验证跨 prune 的外部 pending 捕获最终丢弃时会完成清理。
#[test]
// 执行树外 claim 延迟 prune 后未挂载释放的回归。
fn external_pending_claim_is_removed_when_dropped_after_prune() {
    // 创建外部捕获与无关 prune 共享的窗口私有状态存储。
    let store = ComponentStateStore::new();
    // 创建外部捕获与重试共用的稳定动态命名空间。
    let namespace = test_namespace(76, "pending-drop-after-prune-key");
    // 建立尚未交给 WidgetTree 的外部 pending 捕获。
    let (first, first_receipt) = with_component_state_capture_in_namespace(
        // 传入共享窗口状态存储。
        store.clone(),
        // 传入稳定动态实例身份。
        namespace.clone(),
        // 构建外部捕获状态字段。
        || {
            // 申请未挂载捕获的静态声明身份。
            let scope = uix_component_scope("pending-drop-callsite", 86);
            // 建立由树外 receipt 暂时保护的状态槽。
            uix_component_state(&scope, 1, || 1101_i32)
        },
    );
    // 构造一次与外部捕获无关的空树最终真相。
    let unrelated_live_scopes = HashSet::new();
    // 模拟其他 reconcile 完成后的 idle prune。
    store.retain_scopes(&unrelated_live_scopes);
    // 最终不挂载外部捕获并释放最后一个 claim。
    drop(first_receipt);
    // 记录同身份重试是否重新初始化。
    let reinitialized = Cell::new(false);
    // 在外部 claim 释放后重新请求同一状态字段。
    let (retry, retry_receipt) = with_component_state_capture_in_namespace(
        // 继续复用同一窗口状态存储。
        store,
        // 继续复用同一动态实例身份。
        namespace,
        // 构建重试状态字段。
        || {
            // 重复未挂载捕获的静态声明身份。
            let scope = uix_component_scope("pending-drop-callsite", 86);
            // 请求字段并观测延迟 prune 是否完成。
            uix_component_state(&scope, 1, || {
                // 标记最后一个 claim 释放后槽已被清理。
                reinitialized.set(true);
                // 返回重试后的新初始值。
                1102_i32
            })
        },
    );
    // 跨 prune 未挂载的外部捕获必须重新初始化。
    assert!(reinitialized.get());
    // 重试必须取得不同于旧槽的新身份。
    assert_ne!(retry.slot_id(), first.slot_id());
    // 重试必须采用新的初始值。
    assert_eq!(retry.get(), 1102_i32);
    // 丢弃重试回执以保持测试不挂载重试结果。
    drop(retry_receipt);
}
