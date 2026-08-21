//! 平台公开面的呈现与图形装配契约。
//!
//! 本模块把 native factory / present 的 recipe 装配输入与 surface 描述
//! 收口为 platform 公开窄 API，draw/app 启动路径不再直接引用 `crate::native`
//! 的 factory 与 present 内部模块。

// platform System 唯一拥有跨图形 API 复用的资源、命令、提交与 Surface 契约。
// Drawing 只依赖本模块；Vulkan 等 Adapter 只把这些事实机械映射到原生 API。
pub(crate) mod rhi;

// recipe 装配输入与 backend 描述只供 crate 内部启动路径消费。
pub(crate) use crate::native::factory::{
    GraphicsRecipe, describe_backend_availability, gpu_recipe_candidates,
    graphics_runtime_platform, try_create_gpu_recipe_with_queue,
};
// 图形选择面、recipe owner 与 opaque surface 句柄契约。
pub(crate) use crate::native::present::{
    GpuRecipeOwner, GraphicsApi, GraphicsRecipeOwner, GraphicsSelection, NativeSurfaceHandle,
    PixelUploadRecipeOwner, PresentTestResult,
};
