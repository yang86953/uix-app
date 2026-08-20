//! 平台公开面的呈现与图形装配契约。
//!
//! 本模块把 native factory / present 的 recipe 装配输入与 surface 描述
//! 收口为 platform 公开窄 API，draw/app 启动路径不再直接引用 `crate::native`
//! 的 factory 与 present 内部模块。

// recipe 装配输入与 backend 描述只供 crate 内部启动路径消费。
pub(crate) use crate::native::factory::{
    GraphicsRecipe, describe_backend_availability, gpu_recipe_candidates,
    graphics_runtime_platform, try_create_gpu_recipe_with_queue,
};
// 图形选择面、recipe owner 与 opaque surface 句柄契约。
pub(crate) use crate::native::present::{
    GraphicsApi, GraphicsRecipeOwner, GraphicsSelection, NativeSurfaceHandle,
};

/// 返回逻辑客户区 extent（以物理 DPI 归一后的逻辑尺寸）。
///
/// Windows 以 HWND 实际客户区为准（properties 在样式/DPI 变更窗口期可能
/// 滞后）；其他平台直接返回窗口属性缓存的逻辑尺寸。
/// 仅供 crate 内部启动路径消费，不进入外部公开 API。
pub(crate) fn native_client_logical_extent(
    cached_width: i32,
    cached_height: i32,
    native_window: *mut std::ffi::c_void,
) -> (i32, i32) {
    // Windows 分支经 native 图形边界的共享 drawable 探测取逻辑客户区。
    #[cfg(windows)]
    {
        // 句柄为空时退回缓存值，避免对无效窗口发起 Win32 查询。
        if !native_window.is_null() {
            let drawable = crate::native::presentation::graphics::platform::windows::drawable_size(
                native_window,
                cached_width,
                cached_height,
            );
            return (drawable.logical_width, drawable.logical_height);
        }
    }
    // 非 Windows 目标没有可探测的原生客户区，直接返回缓存值。
    (cached_width, cached_height)
}
