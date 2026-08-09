//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::thread_bound::bind_to_current_thread;
// 引入离开 native factory 前必须构造的已验证 recipe owner。
use crate::native::present::GraphicsRecipeOwner;
// 引入 registry 的 recipe 事实、兼容 context 与原生 surface 句柄。
use crate::native::present::{
    GraphicsApi, GraphicsSelection, IGraphicsContext, NativeSurfaceHandle, PresentMode, RasterMode,
};

/// One probeable graphics configuration.
///
/// A backend identity is diagnostic data only: one API may expose multiple
/// legal raster × present combinations, each of which must be probed and
/// validated independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphicsRecipe {
    pub backend: GraphicsApi,
    pub raster: RasterMode,
    pub present: PresentMode,
}

impl GraphicsRecipe {
    pub const fn new(backend: GraphicsApi, raster: RasterMode, present: PresentMode) -> Self {
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
pub enum BackendStatus {
    Active,
    /// Placeholder API — skipped during Auto probe with diagnostic.
    Planned,
    /// Feature-off or platform mismatch — skipped during Auto probe.
    Disabled,
}

pub(crate) type GraphicsContextFactory =
    fn(*mut c_void, i32, i32, PendingFailureQueue) -> Result<Box<dyn IGraphicsContext>, Error>;

/// One graphics API factory row — API identity plus declared raster × present axes.
///
/// Native factory validation reads live [`IGraphicsContext::caps`]; renderer assembly
/// receives the immutable snapshot carried by the validated recipe owner. These fields
/// document the combination this entry is expected to provide
/// ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
pub struct GraphicsBackendEntry {
    pub id: GraphicsApi,
    pub priority: u8,
    pub status: BackendStatus,
    /// Declared raster axis for this registry row.
    pub raster: RasterMode,
    /// Declared present axis for this registry row.
    pub present: PresentMode,
    /// Construction stays inside native factory routing so every context is
    /// recipe-validated and can later receive the shared thread-affinity
    /// binding. Callers must use a recipe-level factory entry instead of
    /// bypassing the lifecycle contract through a raw function pointer.
    pub(crate) create: GraphicsContextFactory,
}

impl GraphicsBackendEntry {
    pub const fn recipe(&self) -> GraphicsRecipe {
        GraphicsRecipe::new(self.id, self.raster, self.present)
    }

    pub fn is_probe_candidate(&self) -> bool {
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
pub fn active_entries() -> &'static [GraphicsBackendEntry] {
    PLATFORM_ENTRIES
}

/// Lookup a registry row by backend id.
pub fn entry_for(backend: GraphicsApi) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES.iter().find(|entry| entry.id == backend)
}

/// Looks up one exact recipe row.
pub fn entry_for_recipe(recipe: GraphicsRecipe) -> Option<&'static GraphicsBackendEntry> {
    PLATFORM_ENTRIES
        .iter()
        .find(|entry| entry.recipe() == recipe)
}

/// Creates a context for one registry row (no probe loop).
pub(crate) fn try_create_context(
    entry: &GraphicsBackendEntry,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
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
    let mut ctx = (entry.create)(native_surface, width, height, pending_failures)?;
    let caps = ctx.caps();
    let actual = GraphicsRecipe::new(caps.backend, caps.raster, caps.present);
    let expected = entry.recipe();
    if actual != expected {
        let msg = format!("Graphics recipe {expected}: context reported {actual}");
        // Creation succeeded, so a context whose live caps do not match the
        // registry row must be shut down before returning the mismatch.
        let mut ctx = ctx;
        ctx.try_shutdown()?;
        return Err(Error::new(Errc::PlatformError, msg));
    }
    // 首帧前强制检查迁移期薄 RHI，避免 registry 把只有 legacy draw
    // 接口的 context 宣称为产品 GPU recipe。
    let missing_rhi_capability = {
        // 只在这个局部借用内读取组合 RHI 能力快照。
        match ctx.rhi_context() {
            // 真实 adapter 需要满足文档冻结的 GPU 原语基线。
            Some(rhi) => rhi
                .capabilities()
                .first_missing_gpu_baseline()
                .map(str::to_string),
            // 没有薄 RHI 的 legacy context 不能进入生产 recipe。
            None => Some("thin_rhi".to_string()),
        }
    };
    // 能力缺口必须在返回前关闭已经创建的 native context。
    if let Some(missing) = missing_rhi_capability {
        ctx.try_shutdown()?;
        return Err(Error::new(
            Errc::NotImplemented,
            format!("Graphics recipe {expected} lacks required RHI capability: {missing}"),
        ));
    }
    // 首帧前执行真实资源与固定 pipeline probe，拒绝只声明 capability 的 context。
    if let Some(rhi) = ctx.rhi_context() {
        // probe 失败时保持原始 typed error，并先释放已经创建的 owner 资源。
        if let Err(error) = rhi.probe() {
            ctx.try_shutdown()?;
            return Err(error);
        }
        // 记录首帧前已经通过真实资源与固定 pipeline 编译的 adapter。
        tracing::info!("Graphics recipe {expected}: thin RHI probe passed");
    } else {
        // capability gate 已处理 None；这里保留显式分支防止未来逻辑回归。
        ctx.try_shutdown()?;
        return Err(Error::new(
            Errc::NotImplemented,
            format!("Graphics recipe {expected} lacks thin_rhi probe"),
        ));
    }
    Ok(bind_to_current_thread(ctx))
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
pub fn graphics_runtime_platform() -> &'static str {
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
pub fn gpu_recipe_candidates(requested: GraphicsSelection) -> Vec<GraphicsRecipe> {
    active_recipes_by_priority(active_entries(), requested)
}

/// Describes why an explicit backend request has no active registry row.
pub fn describe_backend_availability(requested: GraphicsSelection) -> Option<&'static str> {
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
    // 先通过精确 registry 行创建并 probe 迁移期 context。
    let context = try_create_context(
        entry,
        native_surface.as_raw(),
        width,
        height,
        pending_failures,
    )?;
    // context 不得跨越 native factory，离开前收敛为已验证 recipe owner。
    GraphicsRecipeOwner::try_new(context)
}

#[cfg(test)]
mod selection_tests {
    // 复用 registry 的私有候选筛选与排序实现。
    use super::*;

    // 测试 factory 永远不应被候选排序测试实际调用。
    fn unused_factory(
        _surface: *mut c_void,
        _width: i32,
        _height: i32,
        _pending: PendingFailureQueue,
    ) -> Result<Box<dyn IGraphicsContext>, Error> {
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
}
