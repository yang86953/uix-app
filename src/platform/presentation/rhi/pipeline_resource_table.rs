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

    // 解析仍存活且语义匹配的 pipeline，并只向 Adapter 暴露原生资源可变借用。
    pub(crate) fn get_mut(&mut self, binding: PipelineBinding) -> Result<&mut T> {
        // 先由通用表验证句柄仍属于当前资源表。
        let entry = self.entries.get_mut(binding.handle())?;
        // 可变入口必须与只读入口执行相同的 kind 身份门禁。
        if entry.kind != binding.kind() {
            // 错配不触碰资源内容，保持稳定共享错误。
            return Err(stale_kind());
        }
        // 只有身份与语义同时匹配时才允许 Adapter 物化原生变体。
        Ok(&mut entry.resource)
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
