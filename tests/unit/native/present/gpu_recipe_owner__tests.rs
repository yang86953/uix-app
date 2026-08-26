// 引入共享计数与关闭状态。
use std::{cell::Cell, rc::Rc};

// 引入被测 owner。
use super::GpuRecipeOwner;
// 引入错误分类、coherency 与 surface 值。
use crate::core::{Errc, PresentCoherency, PresentSurface};
// 引入最小记录型 Device/Surface 实现需要的薄 RHI 值和契约。
use crate::platform::presentation::rhi::{
    DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, GraphicsSurface, LoadAction,
    RenderTargetHandle, RhiExtent, RhiPresentTransaction, RhiScissor, RhiViewport,
    SubmissionHandle, SurfaceFrame, SurfaceToken, TextureCopy,
};
// 引入类型化 GPU context 与 capability。
use crate::platform::presentation::{
    GpuRecipeContext, GraphicsApi, GraphicsContextCaps, GraphicsContextLifecycle,
};

// 提供只记录生命周期调用的类型化 GPU context。
struct TypedGpuContext {
    // 记录 resize 调用次数。
    resize_count: Rc<Cell<u32>>,
    // 记录 checked shutdown 是否执行。
    shutdown: Rc<Cell<bool>>,
    // 记录 owner 借用前执行的原生 context 激活次数。
    activate_count: Rc<Cell<u32>>,
}

// 为测试 context 提供最小薄 RHI Device，并记录激活边界。
impl GraphicsDevice for TypedGpuContext {
    // 返回完整 GPU 基线，owner 测试不验证具体原语能力。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 使用共享基线避免伪造平台能力字段。
        GraphicsDeviceCapabilities::full_gpu_baseline()
    }

    // 记录每次 owner 对 Device 或组合 context 的激活。
    fn activate(&mut self) -> crate::core::Result<()> {
        // 增加外部可观察的激活次数。
        self.activate_count.set(self.activate_count.get() + 1);
        // 测试激活固定成功。
        Ok(())
    }

    // 测试不执行真实 render pass。
    fn begin_render_pass(
        // 借用测试 device。
        &mut self,
        // 忽略不透明目标身份。
        _target: RenderTargetHandle,
        // 忽略加载动作。
        _load: LoadAction,
    ) -> crate::core::Result<()> {
        // 返回无原生副作用的成功结果。
        Ok(())
    }

    // 测试不执行真实 draw。
    fn draw(&mut self, _packet: DrawPacket) -> crate::core::Result<()> {
        // 返回无原生副作用的成功结果。
        Ok(())
    }

    // 测试不执行真实 texture copy。
    fn copy_texture(&mut self, _copy: TextureCopy) -> crate::core::Result<()> {
        // 返回无原生副作用的成功结果。
        Ok(())
    }

    // 测试不维护真实 pass 状态。
    fn end_render_pass(&mut self) -> crate::core::Result<()> {
        // 返回无原生副作用的成功结果。
        Ok(())
    }

    // 返回稳定的测试提交身份。
    fn submit(&mut self) -> crate::core::Result<SubmissionHandle> {
        // 使用非零句柄满足共享提交契约。
        Ok(SubmissionHandle::from_raw(1))
    }
}

// 为测试 context 提供不触碰窗口系统的最小 Surface。
impl GraphicsSurface for TypedGpuContext {
    // 返回稳定的一像素 surface token。
    fn token(&self) -> SurfaceToken {
        // 使用固定 generation 和正 extent。
        SurfaceToken::new(0, RhiExtent::new(1, 1))
    }

    // 返回与当前 token 匹配的测试 frame。
    fn acquire(&mut self) -> crate::core::Result<SurfaceFrame> {
        // 使用非零目标身份模拟成功 acquire。
        Ok(SurfaceFrame::new(
            // 保持 frame 与当前 surface 同代，目标种类由类型固定为 Surface。
            self.token(),
        ))
    }

    // 返回请求 extent 对应的新测试 token。
    fn resize(&mut self, extent: RhiExtent) -> crate::core::Result<SurfaceToken> {
        // 测试只验证角色借用，不保存动态 surface 状态。
        Ok(SurfaceToken::new(1, extent))
    }

    // 测试 present 不触碰任何原生窗口。
    fn present(
        // 借用测试 surface。
        &mut self,
        // 忽略不可拆的测试呈现事务。
        _transaction: RhiPresentTransaction,
    ) -> crate::core::Result<()> {
        // 返回无原生副作用的成功结果。
        Ok(())
    }
}

// 实现 GPU context 的共同生命周期。
impl GraphicsContextLifecycle for TypedGpuContext {
    // 返回稳定的最小 surface 快照。
    fn present_surface(&self) -> PresentSurface {
        // 测试不触碰真实 drawable。
        PresentSurface::identity(1, 1, 1.0, 0)
    }

    // 记录 checked shutdown。
    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        // 让 Box 被消费后仍可观察释放动作。
        self.shutdown.set(true);
        // 测试 context 固定关闭成功。
        Ok(())
    }
}

// 实现不可选的 GPU recipe 契约。
impl GpuRecipeContext for TypedGpuContext {
    // 返回由同一测试 owner 实现的组合 thin RHI。
    fn rhi_context(
        // 借用测试 GPU owner。
        &mut self,
    ) -> crate::core::Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        // 不创建第二个 Device 或 Surface 实例。
        Ok(self)
    }

    // 记录一次类型化 resize 委托。
    fn resize_surface(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        // 增加可观察调用次数。
        self.resize_count.set(self.resize_count.get() + 1);
        // 测试 resize 固定成功。
        Ok(())
    }
}

// 构造共享观察状态与类型化 context。
fn test_context() -> (
    // 返回可交给 owner 的类型化 context。
    TypedGpuContext,
    // 返回 resize 次数观察句柄。
    Rc<Cell<u32>>,
    // 返回 shutdown 结果观察句柄。
    Rc<Cell<bool>>,
    // 返回原生 context 激活次数观察句柄。
    Rc<Cell<u32>>,
) {
    // 创建 resize 计数器。
    let resize_count = Rc::new(Cell::new(0));
    // 创建 shutdown 标记。
    let shutdown = Rc::new(Cell::new(false));
    // 创建 owner-context 激活计数器。
    let activate_count = Rc::new(Cell::new(0));
    // 返回 context 与外部观察句柄。
    (
        // 组装测试 context。
        TypedGpuContext {
            // 共享 resize 计数器。
            resize_count: Rc::clone(&resize_count),
            // 共享 shutdown 标记。
            shutdown: Rc::clone(&shutdown),
            // 共享激活计数器。
            activate_count: Rc::clone(&activate_count),
        },
        // 返回 resize 观察句柄。
        resize_count,
        // 返回 shutdown 观察句柄。
        shutdown,
        // 返回激活次数观察句柄。
        activate_count,
    )
}

// 验证错误静态 recipe 会在拒绝前关闭类型化 owner。
#[test]
fn rejects_non_gpu_recipe_and_shuts_down() {
    // 构造可观察的类型化 GPU context。
    let (context, _resize_count, shutdown, _activate_count) = test_context();
    // 故意使用 CPU PixelUpload capability。
    let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
    // GPU owner 必须拒绝静态 recipe 错配。
    let result = GpuRecipeOwner::try_new(Box::new(context), caps);
    // 错配保持参数错误分类。
    assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
    // 拒绝前必须执行 checked shutdown。
    assert!(shutdown.get());
}

// 验证静态 recipe 不得与实际 Surface capability 形成两个真相来源。
#[test]
// 使用默认 FullOnly Surface 与伪造 TrackedSwapchain recipe 制造明确漂移。
fn rejects_surface_coherency_drift_and_shuts_down() {
    // 构造默认报告 FullOnly Surface capability 的测试 context。
    let (context, _resize_count, shutdown, _activate_count) = test_context();
    // 故意让 registry recipe 声称不存在的 tracked swapchain 证明。
    let caps = GraphicsContextCaps::gpu_native_swapchain(
        // 使用 D3D11 身份模拟创建边界错误冻结的快照。
        GraphicsApi::D3d11,
        // 与测试 Surface 默认 FullOnly 事实冲突。
        PresentCoherency::TrackedSwapchain,
    );
    // GPU owner 必须在 Drawing 取得 context 前拒绝 capability 漂移。
    let result = GpuRecipeOwner::try_new(Box::new(context), caps);
    // 漂移必须保持稳定的无效参数分类。
    assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
    // 拒绝候选 context 前必须完成 checked shutdown。
    assert!(shutdown.get());
}

// 验证合法类型化 owner 直接委托 lifecycle 操作。
#[test]
fn delegates_typed_gpu_lifecycle_without_capability_queries() {
    // 构造可观察的类型化 GPU context。
    let (context, resize_count, shutdown, _activate_count) = test_context();
    // 使用合法 GPU-native × Swapchain capability。
    let caps = GraphicsContextCaps::gpu_native_swapchain(
        // 使用 D3D11 身份代表生产 GPU recipe。
        GraphicsApi::D3d11,
        // 测试只需要完整重绘 coherency。
        PresentCoherency::FullOnly,
    );
    // 类型已证明的 context 必须直接进入 owner。
    let mut owner = GpuRecipeOwner::try_new(Box::new(context), caps)
        // 合法 recipe 构造失败属于测试错误。
        .expect("typed GPU recipe must construct");
    // 执行一次 GPU surface resize。
    owner
        // 传入任意正尺寸验证直接委托。
        .resize_surface(8, 6)
        // 类型化委托应成功。
        .expect("typed GPU resize must succeed");
    // resize 只能到达底层一次。
    assert_eq!(resize_count.get(), 1);
    // 显式关闭 owner。
    owner
        // 保留 typed shutdown 返回通道。
        .try_shutdown()
        // 测试 context 固定关闭成功。
        .expect("typed GPU shutdown must succeed");
    // 验证 checked shutdown 已到达底层。
    assert!(shutdown.get());
}

// 验证 Device 和组合 context 借用会激活 owner，而 Surface 借用保持角色正交。
#[test]
fn activates_device_borrows_without_mixing_surface_role() {
    // 构造带激活计数器的类型化 GPU context。
    let (context, _resize_count, _shutdown, activate_count) = test_context();
    // 使用合法 GPU-native × Swapchain capability。
    let caps = GraphicsContextCaps::gpu_native_swapchain(
        // 使用 D3D11 身份代表任意生产 GPU recipe。
        GraphicsApi::D3d11,
        // 测试只需要完整重绘 coherency。
        PresentCoherency::FullOnly,
    );
    // 构造唯一 GPU recipe owner。
    let mut owner = GpuRecipeOwner::try_new(Box::new(context), caps)
        // 合法 recipe 构造失败属于测试错误。
        .expect("typed GPU recipe must construct");
    // 借用窄 Device 角色。
    owner
        // Device 借用必须执行 owner-context 激活。
        .rhi_device()
        // 测试 context 固定激活成功。
        .expect("device borrow should activate");
    // 第一次 Device 借用必须只激活一次。
    assert_eq!(activate_count.get(), 1);
    // 借用完整 FramePlan 组合 context。
    owner
        // 组合借用同样必须执行 owner-context 激活。
        .rhi_context()
        // 测试 context 固定激活成功。
        .expect("context borrow should activate");
    // 第二次组合借用必须再激活当前 owner。
    assert_eq!(activate_count.get(), 2);
    // 只借用 Surface 角色。
    owner
        // Surface 元数据和 resize 不得偷偷触发 Device 激活。
        .rhi_surface()
        // 测试 Surface 借用固定成功。
        .expect("surface borrow should not activate device");
    // Surface 借用必须保持两个角色正交。
    assert_eq!(activate_count.get(), 2);
}
