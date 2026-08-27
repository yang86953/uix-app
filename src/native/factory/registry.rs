//! Backend registry types and table-driven context creation (P6.7 M4–M5 / P6.8).

use std::ffi::c_void;

use crate::core::error::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::thread_bound::bind_to_current_thread;
// 引入离开 native factory 前必须构造的已验证 recipe owner。
use crate::platform::presentation::GraphicsRecipeOwner;
// 引入 registry 的 recipe 事实、类型化 context 与原生 surface 句柄。
use crate::platform::presentation::{
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
    // 平台 registry 的显式优先级先决定参考 API；同优先级才让 GPU-native
    // 先于 PixelUpload。这样三平台可以统一选择 Vulkan，同时保留兼容后端回退。
    entries.sort_by_key(|entry| {
        (
            std::cmp::Reverse(entry.priority),
            entry.raster != RasterMode::GpuNative,
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
