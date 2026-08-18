//! 共享 RHI texture 资源表与 render-target 能力提升。

// 引入统一资源表和 texture/render-target 类型。
use super::{RenderTargetHandle, RhiResourceTable, TextureDesc, TextureHandle};
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
}

// 验证真实 texture 描述决定 render-target 能力，而不是句柄数值。
#[cfg(test)]
mod tests {
    // 引入共享尺寸、格式和句柄类型。
    use super::{RhiTextureResource, RhiTextureResourceTable};
    // 引入稳定错误分类。
    use crate::core::Errc;
    // 引入测试资源描述所需的共享 RHI 值对象。
    use crate::native::present::rhi::{RhiExtent, TextureDesc, TextureFormat, TextureHandle};

    // 保存测试资源的共享描述事实。
    struct TestTexture {
        // 保存不可变 texture 创建描述。
        desc: TextureDesc,
    }

    // 让测试资源满足共享 texture 资源表契约。
    impl RhiTextureResource for TestTexture {
        // 返回资源创建时冻结的描述。
        fn desc(&self) -> TextureDesc {
            // 只读复制描述值，不暴露资源内部状态。
            self.desc
        }
    }

    // 验证两种颜色纹理可以提升而 coverage 纹理被共享门禁拒绝。
    #[test]
    fn resolves_only_renderable_texture_formats() {
        // 创建空的测试 texture 资源表。
        let mut table = RhiTextureResourceTable::new();
        // 登记共享颜色目标描述。
        let color = table.insert(TestTexture {
            // 使用两个 Adapter 都支持的 BGRA 目标格式。
            desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Bgra8Unorm),
        });
        // 登记另一种共享颜色目标描述。
        let rgba_color = table.insert(TestTexture {
            // RGBA 必须与 BGRA 经过同一共享能力门禁。
            desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Rgba8Unorm),
        });
        // 登记 sampled-only coverage 描述。
        let coverage = table.insert(TestTexture {
            // 使用不能作为 render target 的 R8 格式。
            desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::R8Unorm),
        });
        // 颜色纹理必须成功提升为 render target。
        assert!(table.resolve_render_target(color).is_ok());
        // RGBA 颜色纹理也必须成功提升为 render target。
        assert!(table.resolve_render_target(rgba_color).is_ok());
        // coverage 纹理必须在共享层返回参数错误。
        let error = table
            .resolve_render_target(coverage)
            .expect_err("coverage texture must not become a render target");
        // 错误分类不得依赖任一 Adapter。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    // 验证陈旧 texture 身份在能力提升前被资源表拒绝。
    #[test]
    fn rejects_stale_texture_before_target_resolution() {
        // 创建没有资源的测试表。
        let table = RhiTextureResourceTable::<TestTexture>::new();
        // 不存在的句柄不能进入目标构造器。
        let error = table
            .resolve_render_target(TextureHandle::from_raw(1))
            .expect_err("stale texture must fail");
        // 共享资源表保持统一参数错误分类。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }
}
