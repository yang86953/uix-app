// 引入共享计数与关闭状态。
use std::{cell::Cell, rc::Rc};

// 引入被测 owner。
use super::PixelUploadRecipeOwner;
// 引入错误分类、coherency 与 surface 值。
use crate::core::{Errc, PresentCoherency, PresentSurface};
// 引入类型化 PixelUpload context 与 capability。
use crate::platform::presentation::{
    GraphicsApi, GraphicsContextCaps, GraphicsContextLifecycle, PixelUploadSurface, PresentDamage,
};

// 提供只记录生命周期调用的类型化 PixelUpload context。
struct TypedPixelUploadContext {
    // 记录 resize 调用次数。
    resize_count: Rc<Cell<u32>>,
    // 记录最终提交次数。
    present_count: Rc<Cell<u32>>,
    // 记录 checked shutdown 是否执行。
    shutdown: Rc<Cell<bool>>,
}

// 实现 PixelUpload context 的共同生命周期。
impl GraphicsContextLifecycle for TypedPixelUploadContext {
    // 返回稳定的最小 surface 快照。
    fn present_surface(&self) -> PresentSurface {
        // 测试不触碰真实 drawable。
        PresentSurface::identity(2, 2, 1.0, 0)
    }

    // 记录 checked shutdown。
    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        // 让 Box 被消费后仍可观察释放动作。
        self.shutdown.set(true);
        // 测试 context 固定关闭成功。
        Ok(())
    }
}

// 实现不可选的 PixelUpload recipe 契约。
impl PixelUploadSurface for TypedPixelUploadContext {
    // 记录一次类型化 resize 委托。
    fn resize_pixel_upload_surface(
        // 借用上传 owner。
        &mut self,
        // 本测试不保存归一化后的宽度。
        _width: i32,
        // 本测试不保存归一化后的高度。
        _height: i32,
    ) -> crate::core::Result<()> {
        // 增加可观察调用次数。
        self.resize_count.set(self.resize_count.get() + 1);
        // 测试 resize 固定成功。
        Ok(())
    }

    // 记录一次类型化像素提交。
    fn present_pixels(
        // 借用上传 owner。
        &mut self,
        // 本测试不读取像素内容。
        _pixels: &[u32],
        // 本测试不读取物理宽度。
        _width: i32,
        // 本测试不读取物理高度。
        _height: i32,
        // 本测试不读取 damage 细节。
        _damage: PresentDamage,
    ) -> crate::core::Result<()> {
        // 增加可观察提交次数。
        self.present_count.set(self.present_count.get() + 1);
        // 测试提交固定成功。
        Ok(())
    }
}

// 构造共享观察状态与类型化 context。
fn test_context() -> (
    // 返回具体类型化 context。
    TypedPixelUploadContext,
    // 返回 resize 观察句柄。
    Rc<Cell<u32>>,
    // 返回 present 观察句柄。
    Rc<Cell<u32>>,
    // 返回 shutdown 观察句柄。
    Rc<Cell<bool>>,
) {
    // 创建 resize 计数器。
    let resize_count = Rc::new(Cell::new(0));
    // 创建 present 计数器。
    let present_count = Rc::new(Cell::new(0));
    // 创建 shutdown 标记。
    let shutdown = Rc::new(Cell::new(false));
    // 返回 context 与外部观察句柄。
    (
        // 组装测试 context。
        TypedPixelUploadContext {
            // 共享 resize 计数器。
            resize_count: Rc::clone(&resize_count),
            // 共享 present 计数器。
            present_count: Rc::clone(&present_count),
            // 共享 shutdown 标记。
            shutdown: Rc::clone(&shutdown),
        },
        // 返回 resize 观察句柄。
        resize_count,
        // 返回 present 观察句柄。
        present_count,
        // 返回 shutdown 观察句柄。
        shutdown,
    )
}

// 验证错误静态 recipe 会在拒绝前关闭类型化 owner。
#[test]
fn rejects_gpu_recipe_and_shuts_down() {
    // 构造可观察的类型化 PixelUpload context。
    let (context, _resize_count, _present_count, shutdown) = test_context();
    // 故意使用 GPU-native capability。
    let caps = GraphicsContextCaps::gpu_native_swapchain(
        // 使用 D3D11 身份代表生产 GPU recipe。
        GraphicsApi::D3d11,
        // 测试只需要完整重绘 coherency。
        PresentCoherency::FullOnly,
    );
    // PixelUpload owner 必须拒绝静态 recipe 错配。
    let result = PixelUploadRecipeOwner::try_new(Box::new(context), caps);
    // 错配保持参数错误分类。
    assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
    // 拒绝前必须执行 checked shutdown。
    assert!(shutdown.get());
}

// 验证合法类型化 owner 直接委托 lifecycle 操作。
#[test]
fn delegates_typed_pixel_upload_lifecycle_without_capability_queries() {
    // 构造可观察的类型化 PixelUpload context。
    let (context, resize_count, present_count, shutdown) = test_context();
    // 使用合法 CPU × PixelUpload capability。
    let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
    // 类型已证明的 context 必须直接进入 owner。
    let mut owner = PixelUploadRecipeOwner::try_new(Box::new(context), caps)
        // 合法 recipe 构造失败属于测试错误。
        .expect("typed PixelUpload recipe must construct");
    // 执行一次 surface resize。
    owner
        // 传入任意正尺寸验证直接委托。
        .resize_surface(8, 6)
        // 类型化 resize 应成功。
        .expect("typed PixelUpload resize must succeed");
    // 执行一次最终像素提交。
    owner
        // 提交最小完整像素缓冲。
        .present_pixels(&[0; 4], 2, 2, PresentDamage::Full)
        // 类型化提交应成功。
        .expect("typed PixelUpload present must succeed");
    // resize 只能到达底层一次。
    assert_eq!(resize_count.get(), 1);
    // present 只能到达底层一次。
    assert_eq!(present_count.get(), 1);
    // 显式关闭 owner。
    owner
        // 保留 typed shutdown 返回通道。
        .try_shutdown()
        // 测试 context 固定关闭成功。
        .expect("typed PixelUpload shutdown must succeed");
    // 验证 checked shutdown 已到达底层。
    assert!(shutdown.get());
}
