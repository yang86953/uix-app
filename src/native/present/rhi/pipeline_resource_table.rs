//! 共享 RHI pipeline 资源表与不可伪造语义绑定。

// 引入统一错误分类和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入共享 pipeline 身份与通用资源表。
use super::{PipelineBinding, PipelineHandle, PipelineKind, RhiResourceTable};

// 保存 Adapter 私有资源及其创建时冻结的共享 pipeline 语义。
struct Entry<T> {
    // 保存创建成功后由 Adapter 拥有的原生资源。
    resource: T,
    // 保存资源实际对应的共享 pipeline kind。
    kind: PipelineKind,
}

// 由共享 RHI Module 统一签发、解析和销毁 pipeline 身份。
pub(crate) struct RhiPipelineResourceTable<T> {
    // 复用通用资源表的句柄生命周期与陈旧身份门禁。
    entries: RhiResourceTable<PipelineHandle, Entry<T>>,
}

// 为 pipeline 资源表提供唯一的绑定构造和语义校验入口。
impl<T> RhiPipelineResourceTable<T> {
    // 创建一个没有已登记 pipeline 的共享表。
    pub(crate) const fn new() -> Self {
        // 初始状态不持有任何 Adapter 资源。
        Self {
            // 委托通用资源表保存槽位状态。
            entries: RhiResourceTable::new(),
        }
    }

    // 登记实际资源并由同一入口签发匹配的 pipeline 绑定。
    pub(crate) fn insert(&mut self, kind: PipelineKind, resource: T) -> PipelineBinding {
        // 先保存资源实际 kind，再由共享构造器组合不可拆身份。
        let handle = self.entries.insert(Entry { resource, kind });
        // 只有资源表能把新句柄与真实 kind 绑定。
        PipelineBinding::new(handle, kind)
    }

    // 解析仍存活且语义匹配的 pipeline 资源。
    pub(crate) fn get(&self, binding: PipelineBinding) -> Result<&T> {
        // 先由通用表验证句柄仍属于当前资源表。
        let entry = self.entries.get(binding.handle())?;
        // 再拒绝句柄与实际资源语义不一致的伪造绑定。
        if entry.kind != binding.kind() {
            // 错配不触碰资源表，保持稳定共享错误。
            return Err(stale_kind());
        }
        // 仅在两项身份都匹配时暴露 Adapter 私有资源借用。
        Ok(&entry.resource)
    }

    // 检查语义后取出资源，使成功销毁立即失效绑定。
    pub(crate) fn take(&mut self, binding: PipelineBinding) -> Result<T> {
        // 先验证句柄存在与真实 kind，失败不得移除资源。
        let entry = self.entries.get(binding.handle())?;
        // 错配绑定不能销毁真实资源。
        if entry.kind != binding.kind() {
            // 返回统一错误并保留原槽位。
            return Err(stale_kind());
        }
        // 语义验证成功后才执行检查式资源取出。
        Ok(self.entries.take(binding.handle())?.resource)
    }

    // 按签发逆序取出所有仍存活的 Adapter 资源，供原生 owner 统一关闭。
    pub(crate) fn drain_reverse(&mut self) -> impl Iterator<Item = T> + '_ {
        // 丢弃只用于存活期校验的 kind，只把原生资源所有权交回 Adapter。
        self.entries.drain_reverse().map(|entry| entry.resource)
    }
}

// 构造不包含 Adapter 名称的稳定 pipeline 语义错配错误。
fn stale_kind() -> Error {
    // 共享层统一把伪造或陈旧语义归类为参数错误。
    Error::new(Errc::InvalidArgument, "RHI pipeline binding kind is stale")
}

// 验证资源表只接受资源表签发的匹配绑定并保留错配资源。
#[cfg(test)]
mod tests {
    // 引入被测资源表。
    use super::RhiPipelineResourceTable;
    // 引入统一错误码以验证失败分类。
    use crate::core::Errc;
    // 引入共享 pipeline 身份与语义。
    use crate::native::present::rhi::{PipelineBinding, PipelineKind};

    // 验证插入时绑定语义来自资源表保存的 kind。
    #[test]
    fn insert_derives_binding_and_gets_resource() {
        // 创建空的测试资源表。
        let mut table = RhiPipelineResourceTable::new();
        // 由表登记资源并签发绑定。
        let binding = table.insert(PipelineKind::BlurPass, 7_u32);
        // 绑定 kind 必须等于插入时的共享语义。
        assert_eq!(binding.kind(), PipelineKind::BlurPass);
        // 正确绑定必须取得原始资源。
        assert_eq!(
            *table.get(binding).expect("matching binding should resolve"),
            7
        );
    }

    // 验证伪造 kind 被拒绝且资源仍可由正确绑定取得。
    #[test]
    fn rejects_forged_kind_and_preserves_resource() {
        // 创建并登记一个真实 blur pipeline。
        let mut table = RhiPipelineResourceTable::new();
        // 保存由资源表签发的正确身份。
        let binding = table.insert(PipelineKind::BlurPass, 9_u32);
        // 测试入口只用于构造错误 kind 的同句柄绑定。
        let forged = PipelineBinding::for_test(binding.handle(), PipelineKind::SolidMesh);
        // 错配必须返回共享 InvalidArgument。
        let error = table.get(forged).expect_err("forged kind must fail");
        // 错配统一归类为无效参数。
        assert_eq!(error.code(), Errc::InvalidArgument);
        // 错配查询不得破坏真实资源。
        assert_eq!(*table.get(binding).expect("resource must remain"), 9);
    }

    // 验证正确 take 后旧绑定失效且错误仍来自共享表。
    #[test]
    fn takes_matching_resource_and_rejects_stale_binding() {
        // 创建并登记一个真实 pipeline 资源。
        let mut table = RhiPipelineResourceTable::new();
        // 保存资源表签发的完整身份。
        let binding = table.insert(PipelineKind::SolidMesh, 11_u32);
        // 第一次 take 必须移交资源所有权。
        assert_eq!(table.take(binding).expect("take should succeed"), 11);
        // 资源移交后同一绑定必须稳定失败。
        let error = table.get(binding).expect_err("stale binding must fail");
        // 销毁后的身份统一归类为无效参数。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    // 验证 owner 关闭时只从共享表逆序取得仍存活资源。
    #[test]
    fn drains_live_resources_in_reverse_order() {
        // 创建带三个资源的表，并在释放前销毁中间资源。
        let mut table = RhiPipelineResourceTable::new();
        // 第一个资源保持存活。
        table.insert(PipelineKind::SolidMesh, 1_u32);
        // 保存中间资源身份以模拟显式销毁。
        let removed = table.insert(PipelineKind::TexturedQuad, 2_u32);
        // 最后一个资源保持存活。
        table.insert(PipelineKind::BlurPass, 3_u32);
        // 显式销毁不得让 release 再次取得同一资源。
        assert_eq!(table.take(removed).expect("take should succeed"), 2);
        // release 只应按签发逆序取得仍存活资源。
        assert_eq!(table.drain_reverse().collect::<Vec<_>>(), vec![3, 1]);
    }
}
