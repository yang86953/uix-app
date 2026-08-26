// 引入被测资源表和句柄契约。
use super::*;

// 定义只属于当前测试表的句柄类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestHandle(u64);

// 让测试句柄进入共享资源表。
impl RhiResourceHandle for TestHandle {
    // 使用稳定测试资源名称。
    const KIND: &'static str = "test";

    // 保存共享表签发的非零身份。
    fn from_resource_raw(raw: u64) -> Self {
        // 构造不暴露其它资源类型的句柄。
        Self(raw)
    }

    // 返回共享表需要解释的原始身份。
    fn resource_raw(self) -> u64 {
        // 读取测试句柄值。
        self.0
    }
}

// 验证资源身份从一开始递增且可查询修改。
#[test]
fn inserts_and_queries_typed_resources() {
    // 创建空测试表。
    let mut table = RhiResourceTable::<TestHandle, u32>::new();
    // 插入第一个资源并取得身份一。
    let first = table.insert(10);
    // 插入第二个资源并取得身份二。
    let second = table.insert(20);
    // 身份必须从一开始且不复用零值。
    assert_eq!((first, second), (TestHandle(1), TestHandle(2)));
    // 只读查询必须取得原始资源。
    assert_eq!(*table.get(first).expect("first resource should exist"), 10);
    // 可变查询必须修改同一槽位。
    *table.get_mut(second).expect("second resource should exist") = 21;
    // 修改后查询保持同一身份。
    assert_eq!(
        *table.get(second).expect("second resource should exist"),
        21
    );
}

// 验证零值、越界、已销毁与重复销毁拥有稳定差异。
#[test]
fn rejects_null_stale_and_duplicate_destroy() {
    // 创建只有一个资源的测试表。
    let mut table = RhiResourceTable::<TestHandle, u32>::new();
    // 签发唯一有效身份。
    let handle = table.insert(7);
    // 零句柄必须由共享门禁拒绝。
    let null_error = table.get(TestHandle(0)).expect_err("null should fail");
    // 零句柄使用稳定参数错误。
    assert_eq!(null_error.code(), Errc::InvalidArgument);
    // 越界身份必须作为陈旧身份拒绝。
    assert!(table.get(TestHandle(9)).is_err());
    // 第一次检查式销毁必须返回资源所有权。
    assert_eq!(table.take(handle).expect("take should succeed"), 7);
    // 已销毁身份不能继续查询。
    assert!(table.get(handle).is_err());
    // 第二次销毁必须显式失败。
    let duplicate = table.take(handle).expect_err("duplicate take should fail");
    // 诊断必须保留重复销毁而非普通陈旧语义。
    assert!(duplicate.message().contains("already destroyed"));
}

// 验证 owner 关闭按资源签发逆序取出存活项。
#[test]
fn drains_live_resources_in_reverse_order() {
    // 创建三个资源的测试表。
    let mut table = RhiResourceTable::<TestHandle, u32>::new();
    // 插入第一个资源。
    let first = table.insert(1);
    // 插入第二个资源。
    table.insert(2);
    // 插入第三个资源。
    table.insert(3);
    // 预先销毁第一项以验证 drain 跳过空槽位。
    table.take(first).expect("first take should succeed");
    // 收集仍存活资源的逆序关闭序列。
    let drained = table.drain_reverse().collect::<Vec<_>>();
    // 只返回三、二且保持后创建先销毁。
    assert_eq!(drained, vec![3, 2]);
}
