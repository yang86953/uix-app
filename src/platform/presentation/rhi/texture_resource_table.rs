//! 共享 RHI texture 资源表与 render-target 能力提升。

// 引入统一资源表和 texture/render-target 类型。
use super::{
    RenderTargetHandle, RhiResourceTable, TextureCopy, TextureDesc, TextureHandle, TextureMove,
};
// 引入共享资源结果类型。
use crate::core::Result;

// 描述资源表必须能够读取的共享 texture 事实。
pub(crate) trait RhiTextureResource {
    // 返回资源创建时冻结的 texture 描述。
    fn desc(&self) -> TextureDesc;
}

// 保存由真实 texture 描述支撑的类型化资源身份。
pub(crate) struct RhiTextureResourceTable<T: RhiTextureResource> {
    // 复用通用资源表的句柄生命周期和陈旧身份门禁。
    entries: RhiResourceTable<TextureHandle, T>,
}

// 为 texture 资源提供统一的插入、解析、销毁和目标能力提升。
impl<T: RhiTextureResource> RhiTextureResourceTable<T> {
    // 创建没有任何 texture 资源的空表。
    pub(crate) const fn new() -> Self {
        // 委托共享资源表保存槽位状态。
        Self {
            // 初始状态不持有 Adapter 资源。
            entries: RhiResourceTable::new(),
        }
    }

    // 登记实际 texture 资源并由共享表签发句柄。
    pub(crate) fn insert(&mut self, resource: T) -> TextureHandle {
        // 资源表是 texture 句柄的唯一生产 owner。
        self.entries.insert(resource)
    }

    // 读取仍存活的 texture 资源。
    pub(crate) fn get(&self, texture: TextureHandle) -> Result<&T> {
        // 委托通用资源表验证句柄代际。
        self.entries.get(texture)
    }

    // 可变读取仍存活的 texture 资源。
    pub(crate) fn get_mut(&mut self, texture: TextureHandle) -> Result<&mut T> {
        // 委托通用资源表验证句柄代际。
        self.entries.get_mut(texture)
    }

    // 检查式取出 texture 资源。
    pub(crate) fn take(&mut self, texture: TextureHandle) -> Result<T> {
        // 委托通用资源表保持销毁后的陈旧身份语义。
        self.entries.take(texture)
    }

    // 按创建逆序取出所有仍存活的 texture 资源。
    pub(crate) fn drain_reverse(&mut self) -> impl Iterator<Item = T> + '_ {
        // 资源 owner 关闭时保持后创建先销毁顺序。
        self.entries.drain_reverse()
    }

    // 仅从真实 texture 描述提升可渲染目标身份。
    pub(crate) fn resolve_render_target(
        &self,
        texture: TextureHandle,
    ) -> Result<RenderTargetHandle> {
        // 先验证句柄仍属于当前资源表，再读取其真实描述。
        let resource = self.entries.get(texture)?;
        // 共享目标构造器统一执行格式能力门禁。
        RenderTargetHandle::for_texture(texture, resource.desc())
    }

    // 预检普通复制的真实资源描述与共享传输关系。
    pub(crate) fn validate_copy(&self, copy: TextureCopy) -> Result<()> {
        // 先解析源纹理并复制其共享描述事实。
        let source = self.entries.get(copy.source())?.desc();
        // 再解析目标纹理并复制其共享描述事实。
        let destination = self.entries.get(copy.destination())?.desc();
        // 只保留共享验证成功或失败，不向调用方泄漏资源描述。
        copy.validate_transfer(source, destination).map(|_| ())
    }

    // 预检重叠安全移动的真实资源描述与共享传输关系。
    pub(crate) fn validate_move(&self, movement: TextureMove) -> Result<()> {
        // 先解析源纹理并复制其共享描述事实。
        let source = self.entries.get(movement.source())?.desc();
        // 再解析目标纹理并复制其共享描述事实。
        let destination = self.entries.get(movement.destination())?.desc();
        // 只保留共享验证成功或失败，不向调用方泄漏资源描述。
        movement.validate_transfer(source, destination).map(|_| ())
    }
}
