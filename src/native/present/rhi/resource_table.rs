//! 跨图形 API 共用的类型化资源句柄槽位状态机。

// 引入幽灵类型以绑定句柄种类而不额外占用运行时存储。
use std::marker::PhantomData;

// 引入统一错误、错误码与结果类型。
use crate::core::error::{Errc, Error, Result};

// 约束能够由共享资源表签发和解析的类型化句柄。
pub(crate) trait RhiResourceHandle: Copy {
    // 保存跨 Adapter 稳定的资源种类名称。
    const KIND: &'static str;

    // 从共享表已经验证的非零原始身份构造句柄。
    fn from_resource_raw(raw: u64) -> Self;

    // 读取只允许共享表解释的原始身份。
    fn resource_raw(self) -> u64;
}

// 保存一类原生 Adapter 资源及其不可复用的类型化身份。
pub(crate) struct RhiResourceTable<H, T> {
    // 槽位从零存储，但对外句柄始终从一开始且销毁后不复用。
    slots: Vec<Option<T>>,
    // 把资源表与唯一句柄种类绑定，禁止跨表误用。
    handle: PhantomData<H>,
}

// 为共享资源表提供唯一分配、查询、销毁与清空语义。
impl<H: RhiResourceHandle, T> RhiResourceTable<H, T> {
    // 创建一个没有资源槽位的空表。
    pub(crate) const fn new() -> Self {
        // 初始表不签发任何句柄。
        Self {
            // 空向量不会分配原生资源。
            slots: Vec::new(),
            // 幽灵类型只参与编译期类型约束。
            handle: PhantomData,
        }
    }

    // 追加一个永不复用旧槽位的新资源并签发类型化句柄。
    pub(crate) fn insert(&mut self, resource: T) -> H {
        // Vec 的最大长度受 isize::MAX 限制，因此所有支持目标都能无损加一到 u64。
        let raw = self.slots.len() as u64 + 1;
        // 只在句柄身份确定后提交新资源槽位。
        self.slots.push(Some(resource));
        // 返回由当前句柄类型唯一构造的身份。
        H::from_resource_raw(raw)
    }

    // 按类型化身份只读借用仍然存活的资源。
    pub(crate) fn get(&self, handle: H) -> Result<&T> {
        // 先把非零身份无损投影为零基索引。
        let index = index::<H>(handle)?;
        // 越界和已经销毁都统一属于不能再次使用的陈旧身份。
        self.slots
            // 读取可能存在的槽位。
            .get(index)
            // 只接受仍然拥有资源的槽位。
            .and_then(Option::as_ref)
            // 返回跨 Adapter 稳定的陈旧身份错误。
            .ok_or_else(|| stale::<H>())
    }

    // 按类型化身份可变借用仍然存活的资源。
    pub(crate) fn get_mut(&mut self, handle: H) -> Result<&mut T> {
        // 先把非零身份无损投影为零基索引。
        let index = index::<H>(handle)?;
        // 可变查询与只读查询共享同一陈旧身份语义。
        self.slots
            // 读取可能存在的可变槽位。
            .get_mut(index)
            // 只接受仍然拥有资源的槽位。
            .and_then(Option::as_mut)
            // 返回跨 Adapter 稳定的陈旧身份错误。
            .ok_or_else(|| stale::<H>())
    }

    // 检查式取出资源，使同一身份立即进入已销毁状态。
    pub(crate) fn take(&mut self, handle: H) -> Result<T> {
        // 零句柄和无法投影的原始值先由共享索引门禁拒绝。
        let index = index::<H>(handle)?;
        // 越界身份从未由当前表签发，属于陈旧身份。
        let slot = self
            // 读取目标可变槽位。
            .slots
            // 不允许 Adapter 自行扩大表范围。
            .get_mut(index)
            // 使用与查询一致的陈旧身份错误。
            .ok_or_else(|| stale::<H>())?;
        // 只有第一次销毁能够取得资源所有权。
        slot.take().ok_or_else(|| already_destroyed::<H>())
    }

    // 按签发的逆序取出所有仍然存活的资源，供原生 owner 检查式关闭。
    pub(crate) fn drain_reverse(&mut self) -> impl Iterator<Item = T> + '_ {
        // 逆序与 Adapter 既有后创建先销毁顺序一致。
        self.slots.iter_mut().rev().filter_map(Option::take)
    }
}

// 把类型化非零身份投影为当前进程可索引的零基槽位。
fn index<H: RhiResourceHandle>(handle: H) -> Result<usize> {
    // 零值永远不代表已经创建的资源。
    let raw = handle
        // 读取共享表独占解释的原始身份。
        .resource_raw()
        // 从一基身份转换为零基身份。
        .checked_sub(1)
        // 零句柄使用独立稳定错误。
        .ok_or_else(|| null::<H>())?;
    // 超出当前进程索引值域的身份也不可能由本表签发。
    usize::try_from(raw).map_err(|_| stale::<H>())
}

// 构造统一的零句柄参数错误。
fn null<H: RhiResourceHandle>() -> Error {
    // 零值违例与具体原生 API 无关。
    Error::new(
        // 句柄值不合法属于调用参数错误。
        Errc::InvalidArgument,
        // 保存稳定资源种类供诊断。
        format!("RHI {} handle is null", H::KIND),
    )
}

// 构造统一的陈旧句柄参数错误。
fn stale<H: RhiResourceHandle>() -> Error {
    // 越界或已销毁身份都不能继续用于资源操作。
    Error::new(
        // 陈旧身份仍属于调用参数错误。
        Errc::InvalidArgument,
        // 两个 Adapter 输出相同诊断。
        format!("RHI {} handle is stale", H::KIND),
    )
}

// 构造统一的重复销毁参数错误。
fn already_destroyed<H: RhiResourceHandle>() -> Error {
    // 第二次 take 不能伪装成幂等销毁成功。
    Error::new(
        // 重复销毁是无效资源身份操作。
        Errc::InvalidArgument,
        // 两个 Adapter 输出相同诊断。
        format!("RHI {} handle was already destroyed", H::KIND),
    )
}

// 验证共享资源表的类型化分配、查询、销毁与关闭顺序。
#[cfg(test)]
mod tests {
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
}
