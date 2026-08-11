//! CPU PixelUpload recipe 的已验证 owner 门面。

// 引入统一错误、结果与呈现 damage。
use crate::core::{Errc, Error, Result};
// 引入兼容 context、静态 recipe 与原子 surface 快照。
use crate::native::present::{
    GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentMode, PresentSurface, RasterMode,
};

// 保存已经通过 CPU PixelUpload recipe 门禁的原生 context owner。
pub(crate) struct PixelUploadRecipeOwner {
    // 固化构造门禁验证过的静态 recipe 与 backend 事实。
    caps: GraphicsContextCaps,
    // 兼容 context 只留在本门面内部，renderer 不再直接依赖可选 surface 视图。
    context: Box<dyn IGraphicsContext>,
}

// 为 PixelUpload presentation 提供不带可选能力分支的窄 owner 契约。
impl PixelUploadRecipeOwner {
    // 校验顶层 owner 已捕获的静态 recipe 与专用 PixelUpload surface，并在失败前检查式关闭。
    pub(super) fn try_new(
        // 接收仍由本门面唯一拥有的迁移期 context。
        mut context: Box<dyn IGraphicsContext>,
        // 接收正交 owner 分派时已经捕获的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // PixelUpload owner 只接受 CPU × PixelUpload 组合。
        if caps.raster != RasterMode::Cpu || caps.present != PresentMode::PixelUpload {
            // 保存稳定诊断，避免关闭借用影响错误文本。
            let error = Error::new(
                Errc::InvalidArgument,
                format!(
                    "PixelUploadRecipeOwner requires cpu x pixel_upload, got {} raster={} present={}",
                    caps.backend, caps.raster, caps.present
                ),
            );
            // 构造失败前检查式释放原生资源并保留原始原因。
            return match context.try_shutdown() {
                // shutdown 成功时返回 recipe 错误。
                Ok(()) => Err(error),
                // shutdown 失败时链接原始 recipe 错误。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 构造期必须证明专用 PixelUpload surface 存在。
        if context.pixel_upload_surface().is_none() {
            // 保存缺失专用 surface 的稳定状态错误。
            let error = Error::new(
                Errc::InvalidState,
                format!(
                    "GraphicsBackend {} PixelUpload recipe lacks a dedicated surface owner",
                    caps.backend
                ),
            );
            // 缺失必需视图时同样先检查式关闭 native owner。
            return match context.try_shutdown() {
                // shutdown 成功时返回 owner 缺失错误。
                Ok(()) => Err(error),
                // shutdown 失败时链接 owner 缺失错误。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 只有通过完整门禁的 context 才能进入 renderer presentation。
        Ok(Self { caps, context })
    }

    // 返回构造期已验证的静态 recipe 事实。
    pub(crate) fn caps(&self) -> GraphicsContextCaps {
        // 返回构造期快照，禁止运行期 recipe 身份漂移。
        self.caps
    }

    // 返回当前 drawable extent、DPR、transform 与 generation 的原子快照。
    pub(crate) fn present_surface(&self) -> PresentSurface {
        // live 元数据不在门面内拆分或重复缓存。
        self.context.present_surface()
    }

    // 通过专用 PixelUpload surface 重建 native drawable。
    pub(crate) fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 将迁移期 Option 收口为 PixelUpload owner 的稳定 Result 契约。
        let surface = self.context.pixel_upload_surface().ok_or_else(|| {
            // 构造后丢失必需视图属于可恢复层识别的状态破坏。
            Error::new(
                Errc::InvalidState,
                "PixelUpload recipe lost its dedicated resize surface owner",
            )
        })?;
        // 归一化逻辑尺寸后交给唯一 native surface owner。
        surface.resize_pixel_upload_surface(width.max(1), height.max(1))
    }

    // 把 CPU retained pixels 提交给专用 PixelUpload surface。
    pub(crate) fn present_pixels(
        // 借用已经验证的 PixelUpload owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // 接收最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // 将运行期视图丢失映射为稳定 typed state error。
        let surface = self.context.pixel_upload_surface().ok_or_else(|| {
            // 禁止回退已移除的统一 present 入口。
            Error::new(
                Errc::InvalidState,
                "PixelUpload recipe lost its dedicated presentation surface owner",
            )
        })?;
        // 最终像素提交只经过 recipe 专用 surface。
        surface.present_pixels(pixels, width, height, damage)
    }

    // 在 owner-thread 边界检查式关闭原生资源。
    pub(crate) fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure，不降级为日志。
        self.context.try_shutdown()
    }
}

// 验证构造门禁会拒绝错误 recipe 与缺失 PixelUpload surface 的 context。
#[cfg(test)]
mod tests {
    // 引入共享关闭状态所需的 Cell 与 Rc。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner。
    use super::PixelUploadRecipeOwner;
    // 引入错误分类与 coherency 事实。
    use crate::core::{Errc, PresentCoherency};
    // 引入最小测试 context 所需契约。
    use crate::native::present::{
        GraphicsApi, GraphicsContextCaps, IGraphicsContext, PixelUploadSurface, PresentDamage,
        PresentSurface,
    };

    // 提供不暴露 PixelUpload surface 的最小 context。
    struct MissingPixelUploadContext {
        // 允许测试在 owner 消费 Box 后观察 checked shutdown。
        shutdown: Rc<Cell<bool>>,
    }

    // 实现构造门禁消费的最小兼容 context 契约。
    impl IGraphicsContext for MissingPixelUploadContext {
        // 返回稳定的最小 surface 元数据。
        fn present_surface(&self) -> PresentSurface {
            // 测试不涉及真实 drawable 或 surface generation。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 记录构造拒绝路径执行了 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 将共享观察状态置为已关闭。
            self.shutdown.set(true);
            // 测试 context 的关闭固定成功。
            Ok(())
        }
    }

    // 提供可在构造后撤销 surface 视图的 PixelUpload context。
    struct SwitchablePixelUploadContext {
        // 控制下一次可选 surface 查询是否成功。
        available: Rc<Cell<bool>>,
        // 记录专用 resize 被调用的次数。
        resize_count: Rc<Cell<u32>>,
        // 记录最终 pixels 提交次数。
        present_count: Rc<Cell<u32>>,
        // 记录 checked shutdown 是否执行。
        shutdown: Rc<Cell<bool>>,
    }

    // 实现 PixelUpload owner 消费的兼容 context 门面。
    impl IGraphicsContext for SwitchablePixelUploadContext {
        // 按共享开关暴露或撤销专用 surface 视图。
        fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
            // 构造期与正常运行期返回唯一 owner。
            if self.available.get() {
                // 当前 context 同时实现专用 surface 契约。
                Some(self)
            } else {
                // 模拟构造后 native 状态破坏。
                None
            }
        }

        // 返回稳定的最小 surface 元数据。
        fn present_surface(&self) -> PresentSurface {
            // 测试使用 identity DPR 与稳定 generation。
            PresentSurface::identity(2, 2, 1.0, 0)
        }

        // 记录 owner 执行 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 将共享观察状态置为已关闭。
            self.shutdown.set(true);
            // 测试 context 的关闭固定成功。
            Ok(())
        }
    }

    // 实现记录型专用 PixelUpload surface。
    impl PixelUploadSurface for SwitchablePixelUploadContext {
        // 记录一次逻辑 resize。
        fn resize_pixel_upload_surface(
            // 借用记录型 surface owner。
            &mut self,
            // 测试不需要保存归一化后的宽度。
            _width: i32,
            // 测试不需要保存归一化后的高度。
            _height: i32,
        ) -> crate::core::Result<()> {
            // 增加专用 resize 调用计数。
            self.resize_count.set(self.resize_count.get() + 1);
            // 记录型 resize 固定成功。
            Ok(())
        }

        // 记录一次最终 pixels 提交。
        fn present_pixels(
            // 借用记录型 surface owner。
            &mut self,
            // 测试不检查像素内容。
            _pixels: &[u32],
            // 测试不检查物理宽度。
            _width: i32,
            // 测试不检查物理高度。
            _height: i32,
            // 测试不检查 damage 细节。
            _damage: PresentDamage,
        ) -> crate::core::Result<()> {
            // 增加最终提交调用计数。
            self.present_count.set(self.present_count.get() + 1);
            // 记录型提交固定成功。
            Ok(())
        }
    }

    // 验证 GPU recipe 在进入 PixelUpload presentation 前被拒绝并关闭。
    #[test]
    fn rejects_gpu_recipe_before_presentation_construction() {
        // 保存 owner 消费后仍可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 在模拟 adapter 创建边界组装合法 GPU-native × swapchain 快照。
        let caps = GraphicsContextCaps::gpu_native_swapchain(
            // 使用 Windows 参考 backend 身份。
            GraphicsApi::D3d11,
            // 测试只需要稳定的完整重绘 coherency。
            PresentCoherency::FullOnly,
        );
        // 构造 GPU recipe，故意违反 PixelUpload owner 门禁。
        let context = MissingPixelUploadContext {
            // 共享关闭状态给测试断言。
            shutdown: Rc::clone(&shutdown),
        };
        // 尝试构造 PixelUpload owner 并取得稳定失败。
        let result = PixelUploadRecipeOwner::try_new(Box::new(context), caps);
        // recipe 不匹配必须保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }

    // 验证 CPU PixelUpload recipe 缺少专用 surface 时不会进入 presentation。
    #[test]
    fn rejects_pixel_upload_recipe_without_surface_owner() {
        // 保存 owner 消费后仍可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 在模拟 adapter 创建边界组装正式 CPU × PixelUpload 快照。
        let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
        // 构造声明 PixelUpload recipe 但不实现专用 surface 的错误 context。
        let context = MissingPixelUploadContext {
            // 共享关闭状态给测试断言。
            shutdown: Rc::clone(&shutdown),
        };
        // 尝试构造 PixelUpload owner 并取得稳定失败。
        let result = PixelUploadRecipeOwner::try_new(Box::new(context), caps);
        // 必需 surface 缺失必须保持状态错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }

    // 验证正常调用只经过专用 surface，运行期视图丢失保持 typed failure。
    #[test]
    fn delegates_to_surface_and_reports_runtime_owner_loss() {
        // 创建可从测试外部切换的 surface 可用状态。
        let available = Rc::new(Cell::new(true));
        // 创建专用 resize 调用计数。
        let resize_count = Rc::new(Cell::new(0));
        // 创建最终 pixels 提交计数。
        let present_count = Rc::new(Cell::new(0));
        // 创建 checked shutdown 观察状态。
        let shutdown = Rc::new(Cell::new(false));
        // 构造具备完整 PixelUpload surface 的 context。
        let context = SwitchablePixelUploadContext {
            // 共享 surface 可用开关。
            available: Rc::clone(&available),
            // 共享 resize 调用计数。
            resize_count: Rc::clone(&resize_count),
            // 共享 present 调用计数。
            present_count: Rc::clone(&present_count),
            // 共享关闭观察状态。
            shutdown: Rc::clone(&shutdown),
        };
        // 使用与测试 context 声明一致的单次构造快照。
        let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
        // 构造已验证 owner；测试 context 初始提供专用 surface。
        let mut owner = match PixelUploadRecipeOwner::try_new(Box::new(context), caps) {
            // 有效 recipe 必须成功进入 owner。
            Ok(owner) => owner,
            // 构造失败说明门禁错误拒绝了合法 context。
            Err(error) => panic!("valid PixelUpload owner must construct: {error:?}"),
        };
        // 通过 owner 执行一次专用 resize。
        assert!(owner.resize_surface(8, 6).is_ok());
        // 通过 owner 执行一次最终像素提交。
        assert!(owner
            .present_pixels(&[0; 4], 2, 2, PresentDamage::Full)
            .is_ok());
        // 两个操作都必须准确委托一次。
        assert_eq!(resize_count.get(), 1);
        // 最终提交同样只能执行一次。
        assert_eq!(present_count.get(), 1);
        // 模拟构造后 native context 丢失专用 surface 视图。
        available.set(false);
        // 运行期 owner 丢失必须返回稳定状态错误。
        assert!(matches!(
            owner.resize_surface(8, 6),
            Err(error) if error.code() == Errc::InvalidState
        ));
        // 显式关闭仍必须到达底层 context。
        assert!(owner.try_shutdown().is_ok());
        // 验证 checked shutdown 已执行。
        assert!(shutdown.get());
    }
}
