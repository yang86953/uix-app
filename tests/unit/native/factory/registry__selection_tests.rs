// 复用 registry 的私有候选筛选与排序实现。
use super::*;
// 引入 PixelUpload 测试载荷与稳定 surface 快照。
use crate::core::{PresentDamage, PresentSurface};
// 引入 CPU recipe 能力、共享生命周期与专用 PixelUpload surface 契约。
use crate::platform::presentation::{
    GraphicsContextCaps, GraphicsContextLifecycle, PixelUploadSurface,
};

// 构造不暴露 GPU recipe 视图的合法 CPU PixelUpload context。
struct CpuPixelUploadContext;

// 为 registry 行实现最小共享生命周期。
impl GraphicsContextLifecycle for CpuPixelUploadContext {
    // 返回稳定的最小 drawable 快照。
    fn present_surface(&self) -> PresentSurface {
        // 测试只需要证明 owner 构造，不执行真实提交。
        PresentSurface::identity(1, 1, 1.0, 0)
    }

    // 测试 context 没有需要失败的原生资源。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 保持 owner Drop 与拒绝路径可检查式关闭。
        Ok(())
    }
}

// 为测试 context 实现最小 CPU PixelUpload surface。
impl PixelUploadSurface for CpuPixelUploadContext {
    // 测试不执行真实 resize，只证明专用契约可借用。
    fn resize_pixel_upload_surface(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // 最小测试 surface 接受所有尺寸。
        Ok(())
    }

    // 测试不执行真实上传，只保留完整 trait 形状。
    fn present_pixels(
        // 借用测试 surface owner。
        &mut self,
        // 测试不读取像素载荷。
        _pixels: &[u32],
        // 测试不读取物理宽度。
        _width: i32,
        // 测试不读取物理高度。
        _height: i32,
        // 测试不读取 damage。
        _damage: PresentDamage,
    ) -> Result<(), Error> {
        // 最小测试 surface 接受提交。
        Ok(())
    }
}

// 构造合法 CPU PixelUpload registry context。
fn cpu_pixel_upload_factory(
    // 测试不需要真实原生 surface。
    _surface: *mut c_void,
    // 测试不需要保存初始宽度。
    _width: i32,
    // 测试不需要保存初始高度。
    _height: i32,
    // 测试不产生异步故障。
    _pending: PendingFailureQueue,
    // 返回测试 adapter 已组装的 candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建只实现 CPU 专用契约的具体 context。
    let context = CpuPixelUploadContext;
    // 在测试 adapter 创建边界直接组装静态 capability。
    let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
    // 把 context 与同源快照交给 registry 校验。
    Ok(GraphicsContextCandidate::pixel_upload(
        Box::new(context),
        caps,
    ))
}

// 测试 factory 永远不应被候选排序测试实际调用。
fn unused_factory(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
    _pending: PendingFailureQueue,
    // 保持候选排序测试与生产 factory 的返回形状一致。
) -> Result<GraphicsContextCandidate, Error> {
    // 若误入构造路径则返回稳定 typed 错误。
    Err(Error::new(Errc::InvalidState, "selection test factory"))
}

// 构造同时覆盖 Active、Disabled、GPU-native 与 CPU-present 的候选表。
const TEST_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        // 高优先级 Vulkan PixelUpload 条目用于验证平台参考 API 优先级。
        id: GraphicsApi::Vulkan,
        // CPU-present 故意使用最高数值优先级。
        priority: 100,
        // 自动策略应保留该 Active 条目。
        status: BackendStatus::Active,
        // 该条目属于 CPU raster tier。
        raster: RasterMode::Cpu,
        // CPU raster 通过像素上传呈现。
        present: PresentMode::PixelUpload,
        // 候选测试不会实际调用 factory。
        create: unused_factory,
    },
    GraphicsBackendEntry {
        // D3D11 提供可用的 GPU-native 兼容对照条目。
        id: GraphicsApi::D3d11,
        // 较低数值优先级用于证明 API priority 是第一排序键。
        priority: 10,
        // 自动策略应保留该 Active 条目。
        status: BackendStatus::Active,
        // 该条目属于 GPU-native tier。
        raster: RasterMode::GpuNative,
        // GPU-native 使用 swapchain 呈现。
        present: PresentMode::Swapchain,
        // 候选测试不会实际调用 factory。
        create: unused_factory,
    },
    GraphicsBackendEntry {
        // Metal 提供禁用但可显式诊断的对照条目。
        id: GraphicsApi::Metal,
        // 最高 API 优先级不得让禁用条目进入自动候选。
        priority: 200,
        // 显式策略保留 Disabled 条目供 typed 失败使用。
        status: BackendStatus::Disabled,
        // 该条目声明 GPU-native raster。
        raster: RasterMode::GpuNative,
        // 该条目声明 swapchain 呈现。
        present: PresentMode::Swapchain,
        // 候选测试不会实际调用 factory。
        create: unused_factory,
    },
    GraphicsBackendEntry {
        // OpenGL ES 条目验证同层兼容后端仍按数值优先级排序。
        id: GraphicsApi::OpenGlEs,
        // 最低数值优先级排在其余活动条目之后。
        priority: 5,
        // 自动策略应保留该 Active 条目。
        status: BackendStatus::Active,
        // 该条目属于 GPU-native tier。
        raster: RasterMode::GpuNative,
        // GPU-native 使用 swapchain 呈现。
        present: PresentMode::Swapchain,
        // 候选测试不会实际调用 factory。
        create: unused_factory,
    },
];

#[test]
// 验证自动策略只探测 Active recipe，并保持平台声明的参考 API 优先。
fn automatic_selection_filters_status_and_orders_platform_priority() {
    // 执行私有自动候选策略。
    let recipes = active_recipes_by_priority(TEST_ENTRIES, GraphicsSelection::Automatic);
    // Vulkan PixelUpload 的平台优先级高于兼容 GPU-native 条目。
    assert_eq!(
        recipes,
        vec![
            GraphicsRecipe::new(
                GraphicsApi::Vulkan,
                RasterMode::Cpu,
                PresentMode::PixelUpload,
            ),
            GraphicsRecipe::new(
                GraphicsApi::D3d11,
                RasterMode::GpuNative,
                PresentMode::Swapchain,
            ),
            GraphicsRecipe::new(
                GraphicsApi::OpenGlEs,
                RasterMode::GpuNative,
                PresentMode::Swapchain,
            ),
        ]
    );
}

#[test]
// 验证显式策略不偷换 API，并保留禁用条目供 typed 失败使用。
fn explicit_selection_keeps_only_requested_api_without_fallback() {
    // 请求禁用的 Metal 时仍只返回 Metal 条目。
    let recipes = active_recipes_by_priority(
        TEST_ENTRIES,
        GraphicsSelection::Explicit(GraphicsApi::Metal),
    );
    // 结果不得混入任何可用的其他 GPU API。
    assert_eq!(
        recipes,
        vec![GraphicsRecipe::new(
            GraphicsApi::Metal,
            RasterMode::GpuNative,
            PresentMode::Swapchain,
        )]
    );
}

#[test]
// 验证 CPU PixelUpload registry 行跳过 GPU RHI probe 并进入专用 owner。
fn cpu_pixel_upload_registry_row_reaches_recipe_owner_without_gpu_probe() {
    // 构造与测试 context 静态能力一致的合法 registry 行。
    let entry = GraphicsBackendEntry {
        // 使用 Vulkan 身份代表当前跨平台 CPU PixelUpload adapter。
        id: GraphicsApi::Vulkan,
        // 单行构造测试不依赖候选优先级。
        priority: 1,
        // 只有 Active 行可以进入构造路径。
        status: BackendStatus::Active,
        // 声明 CPU raster 轴。
        raster: RasterMode::Cpu,
        // 声明专用像素上传 present 轴。
        present: PresentMode::PixelUpload,
        // 绑定不暴露 GPU 私有契约的测试 factory。
        create: cpu_pixel_upload_factory,
    };
    // 先通过 registry 的能力与配方门禁创建已验证 context 记录。
    let ValidatedGraphicsContext { context, caps } = match try_create_context(
        // 使用刚构造的精确 recipe 行。
        &entry,
        // 测试 factory 不解引用原生 surface。
        std::ptr::null_mut(),
        // 使用最小初始宽度。
        1,
        // 使用最小初始高度。
        1,
        // 传入独立的运行时故障队列。
        PendingFailureQueue::new(),
    ) {
        // 合法 CPU recipe 必须通过 registry。
        Ok(validated) => validated,
        // 任何 GPU 契约依赖都会使该分支暴露具体错误。
        Err(error) => panic!("CPU PixelUpload registry row must be accepted: {error:?}"),
    };
    // 在 native factory 边界完成正交 owner 构造。
    let owner = match GraphicsRecipeOwner::try_new(context, caps) {
        // 专用 surface 完整时必须构造成功。
        Ok(owner) => owner,
        // owner 拒绝说明 registry 没有保留合法 PixelUpload 契约。
        Err(error) => panic!("CPU PixelUpload owner must be constructed: {error:?}"),
    };
    // 最终值必须进入 PixelUpload 分支，不能伪装成 GPU owner。
    assert!(matches!(owner, GraphicsRecipeOwner::PixelUpload(_)));
}
