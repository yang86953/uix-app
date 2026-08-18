//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::thread_bound::bind_to_current_thread;
// 引入离开 native factory 前必须构造的已验证 recipe owner。
use crate::native::present::GraphicsRecipeOwner;
// 引入 registry 的 recipe 事实、类型化 context 与原生 surface 句柄。
use crate::native::present::{
    GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps, GraphicsRecipeContext,
    GraphicsSelection, NativeSurfaceHandle, PresentMode, RasterMode,
};

/// One probeable graphics configuration.
///
/// A backend identity is diagnostic data only: one API may expose multiple
/// legal raster × present combinations, each of which must be probed and
/// validated independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GraphicsRecipe {
    pub(crate) backend: GraphicsApi,
    pub(crate) raster: RasterMode,
    pub(crate) present: PresentMode,
}

impl GraphicsRecipe {
    pub(crate) const fn new(
        backend: GraphicsApi,
        raster: RasterMode,
        present: PresentMode,
    ) -> Self {
        Self {
            backend,
            raster,
            present,
        }
    }
}

impl std::fmt::Display for GraphicsRecipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "backend={}; raster={}; present={}",
            self.backend, self.raster, self.present
        )
    }
}

/// Compile-time availability of a registry row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendStatus {
    Active,
    /// Placeholder API — skipped during Auto probe with diagnostic.
    Planned,
    /// Feature-off or platform mismatch — skipped during Auto probe.
    Disabled,
}

pub(crate) type GraphicsContextFactory =
    fn(*mut c_void, i32, i32, PendingFailureQueue) -> Result<GraphicsContextCandidate, Error>;

/// One graphics API factory row — API identity plus declared raster × present axes.
///
/// Adapter creation returns a [`GraphicsContextCandidate`]; registry validation consumes
/// its immutable snapshot before renderer assembly receives the validated recipe owner. These fields
/// document the combination this entry is expected to provide
/// ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
pub(crate) struct GraphicsBackendEntry {
    pub(crate) id: GraphicsApi,
    pub(crate) priority: u8,
    pub(crate) status: BackendStatus,
    /// Declared raster axis for this registry row.
    pub(crate) raster: RasterMode,
    /// Declared present axis for this registry row.
    pub(crate) present: PresentMode,
    /// Construction stays inside native factory routing so every context is
    /// recipe-validated and can later receive the shared thread-affinity
    /// binding. Callers must use a recipe-level factory entry instead of
    /// bypassing the lifecycle contract through a raw function pointer.
    pub(crate) create: GraphicsContextFactory,
}

impl GraphicsBackendEntry {
    pub(crate) const fn recipe(&self) -> GraphicsRecipe {
        GraphicsRecipe::new(self.id, self.raster, self.present)
    }

    pub(crate) fn is_probe_candidate(&self) -> bool {
        self.status == BackendStatus::Active
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
use crate::native::factory::registry_linux::PLATFORM_ENTRIES;
#[cfg(target_os = "macos")]
use crate::native::factory::registry_macos::PLATFORM_ENTRIES;
#[cfg(windows)]
use crate::native::factory::registry_windows::PLATFORM_ENTRIES;

#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[];

/// All registry rows for the current platform in declaration order.
pub(crate) fn active_entries() -> &'static [GraphicsBackendEntry] {
    PLATFORM_ENTRIES
}

/// Lookup a registry row by backend id.
pub(crate) fn entry_for(backend: GraphicsApi) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES.iter().find(|entry| entry.id == backend)
}

/// Looks up one exact recipe row.
pub(crate) fn entry_for_recipe(recipe: GraphicsRecipe) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES
        .iter()
        .find(|entry| entry.recipe() == recipe)
}

// 保存 registry 已验证且已经绑定 owner thread 的 context 与唯一静态快照。
struct ValidatedGraphicsContext {
    // 持有仍未离开 native factory 的类型化 recipe context。
    context: GraphicsRecipeContext,
    // 持有 adapter 与 registry row 一致性校验使用的同一 capability 快照。
    caps: GraphicsContextCaps,
}

// 在拒绝已经创建的 context 时统一保留主失败与清理失败的因果链。
fn shutdown_context_with_error<T>(
    // 接收仍由 registry 独占的类型化 context。
    context: &mut GraphicsRecipeContext,
    // 接收触发拒绝的原始 typed failure。
    primary_error: Error,
    // 返回原始失败，或把原始失败链接到更紧迫的清理失败之后。
) -> Result<T, Error> {
    // 检查式关闭已经创建的原生资源。
    match context.try_shutdown() {
        // 清理成功时保持原始拒绝原因。
        Ok(()) => Err(primary_error),
        // 清理失败时以资源生命周期失败为主错误，并保留原始原因。
        Err(cleanup_error) => Err(cleanup_error.with_source(primary_error)),
    }
}

/// Creates a context for one registry row (no probe loop).
// 创建一个已经完成 recipe 校验与线程绑定的内部 context 记录。
fn try_create_context(
    entry: &GraphicsBackendEntry,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
    // 返回 context 与 registry 唯一读取的静态 capability 快照。
) -> Result<ValidatedGraphicsContext, Error> {
    if entry.status == BackendStatus::Planned {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "GraphicsBackend {} is planned but not implemented",
                entry.id
            ),
        ));
    }
    if entry.status == BackendStatus::Disabled {
        return Err(Error::new(
            Errc::PlatformError,
            format!("GraphicsBackend {} is disabled in this build", entry.id),
        ));
    }
    // 让 adapter 创建层先交付同源 context 与静态 capability 候选记录。
    let candidate = (entry.create)(native_surface, width, height, pending_failures)?;
    // 一次解包 candidate，registry 不再通过统一门面查询 recipe 能力。
    let (mut context, caps) = candidate.into_parts();
    // 从唯一静态快照构造 adapter 实际 recipe。
    let actual = GraphicsRecipe::new(caps.backend, caps.raster, caps.present);
    // 读取 registry row 声明的预期 recipe。
    let expected = entry.recipe();
    // adapter 静态快照必须与 registry row 完全一致。
    if actual != expected {
        // 保留完整的预期与实际 recipe 诊断。
        let error = Error::new(
            // adapter 候选记录不一致属于平台实现错误。
            Errc::PlatformError,
            // 保留 registry 与 adapter 的实际差异。
            format!("Graphics recipe {expected}: context reported {actual}"),
        );
        // Creation succeeded, so a context whose live caps do not match the
        // registry row must be shut down before returning the mismatch.
        return shutdown_context_with_error(&mut context, error);
    }
    // 类型化 candidate 分支必须与静态 recipe 轴保持一致。
    let context_matches_recipe = match (&context, expected.raster, expected.present) {
        // GPU owner 只能配对 GPU-native × Swapchain。
        (GraphicsRecipeContext::Gpu(_), RasterMode::GpuNative, PresentMode::Swapchain) => true,
        // PixelUpload owner 只能配对 CPU × PixelUpload。
        (GraphicsRecipeContext::PixelUpload(_), RasterMode::Cpu, PresentMode::PixelUpload) => true,
        // 所有交叉组合都表示 adapter 构造契约破坏。
        _ => false,
    };
    // 在进入 Drawing renderer 前拒绝 context 类型与 recipe 轴不一致。
    if !context_matches_recipe {
        // 构造稳定错误，禁止把类型错配伪装成运行期能力缺失。
        let error = Error::new(
            // adapter 候选记录不一致属于平台实现错误。
            Errc::PlatformError,
            // 保留具体 recipe 供定位错误注册行。
            format!("Graphics recipe {expected}: typed context variant does not match recipe"),
        );
        // 拒绝前检查式释放已经创建的原生 owner，并保留双重失败因果链。
        return shutdown_context_with_error(&mut context, error);
    }
    // 只有 GPU-native × Swapchain recipe 依赖 thin RHI，CPU PixelUpload 不得查询该私有契约。
    if let GraphicsRecipeContext::Gpu(gpu_context) = &mut context {
        // 首帧前强制检查类型化 GPU recipe，避免 registry 接受不完整 RHI。
        let missing_rhi_capability = {
            // 只在这个局部借用内取得必需 RHI 能力快照。
            match gpu_context.rhi_context() {
                // 真实 adapter 需要满足文档冻结的 GPU 原语基线。
                Ok(rhi) => rhi
                    // Registry 只借用组合 owner 的只读 Device 角色。
                    .device_ref()
                    // 只验证资源、pass 与 pipeline 的 Device 能力。
                    .device_capabilities()
                    // Surface 可选能力不得参与 Device 构造门禁。
                    .first_missing_gpu_baseline()
                    .map(str::to_string),
                // 构造期借用失败视为必需 RHI owner 不可用。
                Err(_) => Some("rhi_context".to_string()),
            }
        };
        // 能力缺口必须在返回前关闭已经创建的 native context。
        if let Some(missing) = missing_rhi_capability {
            // 构造缺少 GPU 基线原语的 typed failure。
            let error = Error::new(
                // 未满足生产 GPU 基线属于尚未实现。
                Errc::NotImplemented,
                // 保留缺失原语名称与具体 recipe。
                format!("Graphics recipe {expected} lacks required RHI capability: {missing}"),
            );
            // 检查式关闭类型化 owner，并保留可能的双重失败。
            return shutdown_context_with_error(&mut context, error);
        }
        // native factory 到此只建立薄 RHI 能力事实；Drawing 固定 pipeline 探针由上层 GPU Module 执行。
        tracing::info!("Graphics recipe {expected}: native Device and Surface contract passed");
    }
    // 把 context 绑定到 owner thread；静态快照继续由验证记录独立持有。
    let context = bind_to_current_thread(context);
    // 把 context 与快照作为不可拆分的 factory 内部验证记录返回。
    Ok(ValidatedGraphicsContext { context, caps })
}

/// Ordered probe candidates for a backend request, with one item per recipe
/// row.  An explicit backend request deliberately retains every active recipe
/// for that backend instead of selecting the first matching row.
fn matches_probe_request(entry: &GraphicsBackendEntry, requested: GraphicsSelection) -> bool {
    match requested {
        // 自动策略只接纳当前构建可探测的 Active recipe。
        GraphicsSelection::Automatic => entry.is_probe_candidate(),
        // 显式策略保留同一 API 的条目，让禁用或未实现状态产生 typed 错误。
        GraphicsSelection::Explicit(api) => entry.id == api,
    }
}

pub(crate) fn active_recipes_by_priority(
    entries: &[GraphicsBackendEntry],
    requested: GraphicsSelection,
) -> Vec<GraphicsRecipe> {
    let mut entries = entries
        .iter()
        .filter(|entry| matches_probe_request(entry, requested))
        .collect::<Vec<_>>();
    // GPU-native raster is the primary rendering tier.  A CPU raster recipe
    // may still use a GPU for presentation, but it must remain behind every
    // usable native raster candidate so Auto never selects PixelUpload merely
    // because that API has a higher platform priority. Stable sort preserves
    // declaration order for equal priorities inside the same tier.
    entries.sort_by_key(|entry| {
        (
            entry.raster != RasterMode::GpuNative,
            std::cmp::Reverse(entry.priority),
        )
    });
    entries
        .into_iter()
        .map(GraphicsBackendEntry::recipe)
        .collect()
}

/// Runtime platform label used by graphics bootstrap diagnostics.
pub(crate) fn graphics_runtime_platform() -> &'static str {
    #[cfg(windows)]
    {
        "windows"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
    {
        "unknown"
    }
}

/// Recipe-level probe candidates used by graphics bootstrap.
pub(crate) fn gpu_recipe_candidates(requested: GraphicsSelection) -> Vec<GraphicsRecipe> {
    active_recipes_by_priority(active_entries(), requested)
}

/// Describes why an explicit backend request has no active registry row.
pub(crate) fn describe_backend_availability(requested: GraphicsSelection) -> Option<&'static str> {
    // 自动策略没有单一 API 可供可用性诊断。
    let GraphicsSelection::Explicit(api) = requested else {
        return None;
    };
    // 显式请求只描述同一具体 API 的 registry 状态。
    match entry_for(api) {
        Some(entry) => match entry.status {
            BackendStatus::Active => None,
            BackendStatus::Planned => Some("planned but not implemented on this platform"),
            BackendStatus::Disabled => Some("disabled in this build"),
        },
        None => Some("not registered for this platform"),
    }
}

/// Creates one exact recipe context using the runtime-scoped callback queue.
pub(crate) fn try_create_gpu_recipe_with_queue(
    recipe: GraphicsRecipe,
    native_surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<GraphicsRecipeOwner, Error> {
    let entry = entry_for_recipe(recipe).ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("Graphics factory: no registry entry for recipe {recipe}"),
        )
    })?;
    // Only the native factory bridge unwraps the opaque surface handle before
    // it reaches an API/platform constructor.
    // 先通过精确 registry 行创建并验证 native context 与静态快照。
    let ValidatedGraphicsContext { context, caps } = try_create_context(
        entry,
        native_surface.as_raw(),
        width,
        height,
        pending_failures,
    )?;
    // context 不得跨越 native factory，离开前消费同一快照并收敛为已验证 owner。
    GraphicsRecipeOwner::try_new(context, caps)
}

#[cfg(test)]
mod selection_tests {
    // 复用 registry 的私有候选筛选与排序实现。
    use super::*;
    // 引入 PixelUpload 测试载荷与稳定 surface 快照。
    use crate::core::{PresentDamage, PresentSurface};
    // 引入 CPU recipe 能力、共享生命周期与专用 PixelUpload surface 契约。
    use crate::native::present::{
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
            // 高优先级 CPU-present 条目用于验证 raster tier 优先级。
            id: GraphicsApi::D3d11,
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
            // Vulkan 提供可用的 GPU-native 对照条目。
            id: GraphicsApi::Vulkan,
            // 较低数值优先级用于证明 tier 高于 priority。
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
            // 第二个 D3D11 条目验证同一 API 可以保留多个 recipe。
            id: GraphicsApi::D3d11,
            // 最低数值优先级仍不改变 GPU-native tier 的先行顺序。
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
    // 验证自动策略只探测 Active recipe，并保持 GPU-native 优先。
    fn automatic_selection_filters_status_and_orders_raster_tiers() {
        // 执行私有自动候选策略。
        let recipes = active_recipes_by_priority(TEST_ENTRIES, GraphicsSelection::Automatic);
        // 即使 CPU 条目 priority 更高，GPU-native 仍先于 CPU-present。
        assert_eq!(
            recipes,
            vec![
                GraphicsRecipe::new(
                    GraphicsApi::Vulkan,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain,
                ),
                GraphicsRecipe::new(
                    GraphicsApi::D3d11,
                    RasterMode::GpuNative,
                    PresentMode::Swapchain,
                ),
                GraphicsRecipe::new(
                    GraphicsApi::D3d11,
                    RasterMode::Cpu,
                    PresentMode::PixelUpload,
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
}
