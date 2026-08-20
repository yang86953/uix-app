// 引入共享错误分类供断言 typed failure。
use crate::core::Errc;

// 引入被测 resize 事务和 Surface 值对象。
use super::{RhiExtent, RhiSurfaceResizeTransaction, SurfaceToken};

// 构造固定旧 Surface token，避免每个用例重复表达初始事实。
fn previous(extent: RhiExtent) -> SurfaceToken {
    // 使用非零 generation 覆盖回退检查。
    SurfaceToken::new(7, extent)
}

// 非法请求必须在进入任一 Adapter 前得到相同参数错误。
#[test]
fn resize_rejects_extent_outside_the_shared_native_domain() {
    // 构造零宽度请求。
    let requested = RhiExtent::new(0, 480);
    // 通过共享事务门禁执行验证。
    let result = RhiSurfaceResizeTransaction::validate(
        // 传入非法请求尺寸。
        requested,
        // 传入一个有效旧 Surface token。
        previous(RhiExtent::new(640, 480)),
    );
    // 两个 Adapter 必须观察到同一错误分类。
    assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
}

// 已验证事务必须只发布无损的请求和原生尺寸投影。
#[test]
fn resize_exposes_one_checked_native_projection() {
    // 构造有效的新物理尺寸。
    let requested = RhiExtent::new(800, 600);
    // 创建共享类型化事务。
    let transaction = RhiSurfaceResizeTransaction::validate(
        // 传入目标尺寸。
        requested,
        // 传入旧 Surface token。
        previous(RhiExtent::new(640, 480)),
    )
    // 有效请求必须成功进入已验证状态。
    .expect("valid surface resize request");
    // Adapter 读取的 extent 必须保持原值。
    assert_eq!(transaction.extent(), requested);
    // 原生尺寸必须由共享值域规则唯一投影。
    assert_eq!(transaction.native_size_i32(), (800, 600));
}

// Adapter 成功后返回不同尺寸必须被共享后置门禁拒绝。
#[test]
fn resize_rejects_a_result_with_the_wrong_extent() {
    // 构造目标尺寸。
    let requested = RhiExtent::new(800, 600);
    // 创建合法事务。
    let transaction = RhiSurfaceResizeTransaction::validate(
        // 传入请求尺寸。
        requested,
        // 传入旧尺寸。
        previous(RhiExtent::new(640, 480)),
    )
    // 合法输入不应在前置门禁失败。
    .expect("valid surface resize request");
    // 模拟 Adapter 声称成功但仍报告旧尺寸。
    let result = transaction.complete(SurfaceToken::new(8, RhiExtent::new(640, 480)));
    // 后置条件失败必须属于平台实现错误。
    assert!(matches!(result, Err(error) if error.code() == Errc::PlatformError));
}

// 成功 resize 的 generation 绝不能低于事务开始值。
#[test]
fn resize_rejects_generation_regression() {
    // 保持相同 extent 以单独验证代际回退。
    let extent = RhiExtent::new(640, 480);
    // 创建共享事务。
    let transaction = RhiSurfaceResizeTransaction::validate(
        // 请求保持相同尺寸。
        extent,
        // 旧 generation 固定为七。
        previous(extent),
    )
    // 相同合法尺寸必须通过前置门禁。
    .expect("valid surface resize request");
    // 模拟 Adapter 把代际错误地回退到六。
    let result = transaction.complete(SurfaceToken::new(6, extent));
    // 回退必须由共享后置门禁拒绝。
    assert!(matches!(result, Err(error) if error.code() == Errc::PlatformError));
}

// 物理尺寸变化后必须推进 generation 隔离旧 Surface image。
#[test]
fn resize_requires_generation_advance_when_extent_changes() {
    // 构造改变后的 extent。
    let requested = RhiExtent::new(800, 600);
    // 创建从旧尺寸到新尺寸的事务。
    let transaction = RhiSurfaceResizeTransaction::validate(
        // 传入新尺寸。
        requested,
        // 传入旧 token。
        previous(RhiExtent::new(640, 480)),
    )
    // 合法尺寸变化必须通过前置门禁。
    .expect("valid surface resize request");
    // 模拟 Adapter 改变 extent 却保留旧 generation。
    let result = transaction.complete(SurfaceToken::new(7, requested));
    // 缺少代际推进必须被拒绝。
    assert!(matches!(result, Err(error) if error.code() == Errc::PlatformError));
}

// 相同 extent 的同步允许保留 generation，真实状态变化也允许继续推进。
#[test]
fn resize_accepts_consistent_same_and_changed_extent_results() {
    // 构造当前 extent。
    let same_extent = RhiExtent::new(640, 480);
    // 创建不改变物理尺寸的事务。
    let same = RhiSurfaceResizeTransaction::validate(
        // 请求相同 extent。
        same_extent,
        // 传入旧 token。
        previous(same_extent),
    )
    // 有效请求必须创建事务。
    .expect("valid same-extent resize request");
    // 相同 extent 可以保持 generation 不变。
    assert_eq!(
        // 完成相同 extent 的同步事务。
        same.complete(SurfaceToken::new(7, same_extent)),
        // 返回值必须保持 Adapter 的当前 token。
        Ok(SurfaceToken::new(7, same_extent)),
    );
    // 构造变化后的 extent。
    let changed_extent = RhiExtent::new(800, 600);
    // 创建物理尺寸变化事务。
    let changed = RhiSurfaceResizeTransaction::validate(
        // 请求新 extent。
        changed_extent,
        // 从旧 extent 开始。
        previous(same_extent),
    )
    // 有效变化必须创建事务。
    .expect("valid changed-extent resize request");
    // 尺寸改变且 generation 推进的结果必须成功发布。
    assert_eq!(
        // 完成变化事务。
        changed.complete(SurfaceToken::new(8, changed_extent)),
        // 返回新 token。
        Ok(SurfaceToken::new(8, changed_extent)),
    );
}
