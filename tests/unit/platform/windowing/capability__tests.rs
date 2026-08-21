use super::*;

#[test]
fn capability_set_has_stable_value_semantics() {
    // 全集必须只包含权威枚举声明的三十三个唯一值。
    assert_eq!(WindowCapabilities::ALL.len(), WindowCapability::ALL.len());
    assert_eq!(WindowCapability::ALL.len(), 33);
    assert_eq!(WindowCapabilities::ALL.iter().count(), 33);

    // 重复插入同一能力不得改变集合值或计数。
    let duplicate = WindowCapabilities::from_slice(&[
        WindowCapability::Raise,
        WindowCapability::Raise,
        WindowCapability::SetPosition,
    ]);
    assert_eq!(duplicate.len(), 2);
    assert!(duplicate.supports(WindowCapability::Raise));
    assert!(duplicate.supports(WindowCapability::SetPosition));

    // 增删与并集均返回独立值，不修改原集合。
    let raised = WindowCapabilities::EMPTY.with(WindowCapability::Raise);
    let positioned = WindowCapabilities::EMPTY.with(WindowCapability::SetPosition);
    let combined = raised.union(positioned);
    assert!(WindowCapabilities::EMPTY.is_empty());
    assert_eq!(combined.len(), 2);
    assert_eq!(combined.without(WindowCapability::Raise), positioned);
}

#[test]
fn unsupported_error_uses_stable_operation_identity() {
    let error = WindowCapability::SetPosition.unsupported_error();
    assert_eq!(error.code(), Errc::NotImplemented);
    assert_eq!(
        error.message(),
        "IWindowProperties::set_position is not supported by this window"
    );

    // 每个能力都必须拥有非空且唯一的公开操作身份。
    let mut operations = WindowCapability::ALL
        .into_iter()
        .map(WindowCapability::operation)
        .collect::<Vec<_>>();
    assert!(operations.iter().all(|operation| !operation.is_empty()));
    operations.sort_unstable();
    operations.dedup();
    assert_eq!(operations.len(), WindowCapability::ALL.len());
}
