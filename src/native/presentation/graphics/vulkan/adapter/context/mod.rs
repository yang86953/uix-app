//! Vulkan graphics context for Linux Wayland, Windows Win32, and macOS MoltenVK
//! (PixelUpload via shared swapchain stage/present).

use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::{GraphicsContextLifecycle, PixelUploadSurface, PresentDamage};

#[cfg(target_os = "macos")]
use crate::native::backends::macos::platform as macos_surface;

use super::adapter::{VulkanAdapterInfo, select_queue};
pub(crate) use super::device::staging_size;
use super::device::{VulkanDevice, VulkanRuntime};
use super::drawable::drawable_size;
use super::surface::{
    choose_composite_alpha, choose_extent, choose_present_mode, choose_surface_format,
    create_platform_surface, destroy_failed_surface,
};

// 构造 module 唯一持有正式 context 交付前的 Vulkan native 资源。
mod construction;
mod graphics;
// 显式 Vulkan parity 使用无窗口生产 Device 角色执行真实 FramePlan。
#[cfg(feature = "vulkan-parity-test")]
mod headless_parity;
mod methods;
mod rhi_device;
mod rhi_frame;
mod rhi_pipeline;
mod rhi_surface;
mod rhi_surface_readback;
mod rhi_texture;
mod swapchain;
mod transfer;

// 构造函数只通过私有 guard 完成失败回滚与成功句柄移交。
use construction::PendingVulkanContext;
use rhi_device::VulkanRhiDevice;
use rhi_surface_readback::VulkanSurfaceReadbackBuffer;

// 测试 harness 仍复用私有 RHI Device，不扩大生产 Adapter 接口。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_gpu_parity_test() {
    rhi_device::run_gpu_parity_test();
}

// 显式测试 feature 复用 Vulkan 每图像 present 完成状态机。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_present_completion_contract_test() {
    swapchain::run_present_completion_contract_test();
}

#[cfg(test)]
pub(crate) use swapchain::PresentCompletion;
pub(crate) use swapchain::allocate_image_layouts;
#[cfg(test)]
pub(crate) use transfer::allocate_cpu_shadow;

use swapchain::{
    PresentFenceSet, PresentLifetime, allocate_presented_images, create_render_finished_semaphores,
    create_swapchain_image_views, destroy_image_views, destroy_semaphores,
};

pub(crate) fn vk_err(operation: &str, err: vk::Result) -> Error {
    let code = match err {
        vk::Result::ERROR_OUT_OF_DATE_KHR
        | vk::Result::SUBOPTIMAL_KHR
        | vk::Result::ERROR_SURFACE_LOST_KHR => Errc::GraphicsSurfaceLost,
        vk::Result::ERROR_DEVICE_LOST => Errc::GraphicsDeviceLost,
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
            Errc::GraphicsOutOfMemory
        }
        _ => Errc::PlatformError,
    };
    Error::new(code, format!("VulkanContext: {operation} failed: {err:?}"))
}

pub(crate) fn failed_submit_error(submit_status: vk::Result, fence_recovery: Result<()>) -> Error {
    let submit_failure = vk_err("vkQueueSubmit", submit_status);
    match fence_recovery {
        Ok(()) => submit_failure,
        Err(recovery_failure) => submit_failure.with_appended_source(recovery_failure),
    }
}

pub(crate) fn merge_surface_recreate_failure(
    surface_failure: Error,
    recreate_failure: Error,
) -> Error {
    match recreate_failure.code() {
        Errc::GraphicsDeviceLost | Errc::GraphicsOutOfMemory => {
            recreate_failure.with_appended_source(surface_failure)
        }
        _ => surface_failure.with_appended_source(recreate_failure),
    }
}

/// 校验 teardown 前的 device idle 结果。
///
/// Vulkan 把 `ERROR_DEVICE_LOST` 视为 pending 资源不再 in-use，但 child object
/// 仍须显式销毁；因此该状态允许继续按子到父的顺序回收。其余 wait 失败保留 typed 根因。
pub(crate) fn accept_device_wait_for_shutdown(
    result: std::result::Result<(), vk::Result>,
) -> Result<()> {
    match result {
        Ok(()) | Err(vk::Result::ERROR_DEVICE_LOST) => Ok(()),
        Err(err) => Err(vk_err("vkDeviceWaitIdle during shutdown", err)),
    }
}

/// 架构守卫的拆分后源边界：`surface.rs` 持有 `create_win32_surface`、
/// `create_wayland_surface`、`create_metal_surface` 与 `portability_enumeration`；
/// `adapter.rs` 持有 `portability_subset`。返回源码仅供测试核对真实所有权。
#[cfg(test)]
pub(crate) const fn platform_contract_sources() -> (&'static str, &'static str) {
    (include_str!("../surface.rs"), include_str!("../adapter.rs"))
}

#[cfg(test)]
pub(crate) const fn drawable_contract_source() -> &'static str {
    include_str!("../drawable.rs")
}

pub(super) fn loader_err(operation: &str, err: impl std::fmt::Debug) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("VulkanContext: {operation} failed: {err:?}"),
    )
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(Errc::InvalidArgument, message.into())
}

pub(crate) fn validate_swapchain_support(
    formats: &[vk::SurfaceFormatKHR],
    present_modes: &[vk::PresentModeKHR],
) -> Result<()> {
    if formats.is_empty() {
        return Err(Error::new(
            Errc::PlatformError,
            "VulkanContext: surface reported no swapchain formats",
        ));
    }
    if present_modes.is_empty() {
        return Err(Error::new(
            Errc::PlatformError,
            "VulkanContext: surface reported no present modes",
        ));
    }
    Ok(())
}

pub(crate) fn validate_swapchain_images(images: &[vk::Image]) -> Result<()> {
    if images.is_empty() {
        return Err(Error::new(
            Errc::PlatformError,
            "VulkanContext: swapchain reported no images",
        ));
    }
    Ok(())
}

struct UploadBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
}

// 保存 acquire 后尚未由 Device submit 消费的 swapchain image 同步事实。
#[derive(Clone, Copy)]
struct VulkanAcquiredFrame {
    image_index: u32,
    acquire_suboptimal: bool,
    render_finished: vk::Semaphore,
    present_fence: Option<vk::Fence>,
    release_count: usize,
}

// 保存 queue submit 后只允许 Surface present 消费的不可拆原生事实。
struct VulkanSubmittedFrame {
    image_index: u32,
    acquire_suboptimal: bool,
    render_finished: vk::Semaphore,
    present_fence: Option<vk::Fence>,
    submission: crate::platform::presentation::rhi::SubmissionHandle,
}

// 显式 parity feature 可安排的一次性 Vulkan Surface 原生结果。
#[cfg(feature = "vulkan-parity-test")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VulkanSurfaceFaultForParity {
    // 在调用 vkAcquireNextImageKHR 前模拟旧 swapchain 已失效。
    AcquireOutOfDate,
    // 保留真实 acquire image，只把其返回状态提升为 SUBOPTIMAL。
    AcquireSuboptimal,
    // 在调用 vkQueuePresentKHR 前模拟当前 swapchain 已失效。
    PresentOutOfDate,
    // 保留真实 queue present，只把其返回状态提升为 SUBOPTIMAL。
    PresentSuboptimal,
}

pub struct VulkanContext {
    runtime: Option<Rc<VulkanRuntime>>,
    device_lease: Option<Rc<VulkanDevice>>,
    surface_loader: ash::khr::surface::Instance,
    #[cfg(target_os = "linux")]
    _wayland_surface_loader: ash::khr::wayland_surface::Instance,
    #[cfg(windows)]
    _win32_surface_loader: ash::khr::win32_surface::Instance,
    #[cfg(target_os = "macos")]
    _metal_surface_loader: ash::ext::metal_surface::Instance,
    /// AppKit-owned `CAMetalLayer` (macOS MoltenVK WSI); used to sync drawableSize.
    #[cfg(target_os = "macos")]
    metal_layer: *mut c_void,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    pub(crate) adapter_info: VulkanAdapterInfo,
    device: ash::Device,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    image_layouts: Vec<vk::ImageLayout>,
    swapchain_format: vk::Format,
    // 保存最近一次成功创建 swapchain 时查询到的真实 Surface image usage 集合。
    surface_supported_usage_flags: vk::ImageUsageFlags,
    extent: vk::Extent2D,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    // 持有由 platform 类型化资源表签发身份的 Vulkan RHI Device 状态。
    rhi_device: VulkanRhiDevice,
    // Surface adapter 独占的 HOST_VISIBLE transfer destination，不进入通用资源表。
    surface_readback: VulkanSurfaceReadbackBuffer,
    upload: UploadBuffer,
    image_available: vk::Semaphore,
    render_finished: Vec<vk::Semaphore>,
    present_fences: PresentFenceSet,
    present_lifetime: PresentLifetime,
    frame_fence: vk::Fence,
    acquired_frame: Option<VulkanAcquiredFrame>,
    submitted_frame: Option<VulkanSubmittedFrame>,
    // 一次只允许 parity 组合根安排一个 Surface 原生结果，不进入生产 feature。
    #[cfg(feature = "vulkan-parity-test")]
    surface_fault_for_parity: Option<VulkanSurfaceFaultForParity>,
    // 跳过 present 后禁止复用仍为 signaled 的旧代 render-finished semaphore。
    #[cfg(feature = "vulkan-parity-test")]
    replace_present_sync_for_parity: bool,
    // API 无关状态机唯一拥有 Surface generation、extent 与重建事务顺序。
    surface_lifecycle: crate::platform::presentation::rhi::RhiSurfaceLifecycle,
    native_surface: *mut c_void,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    /// Last successfully staged PixelUpload frame (CPU shadow of staging buffer).
    /// Enables destination-dependent IR via readback → apply → replace upload without
    /// claiming GPU image readback.
    cpu_shadow: Vec<u32>,
    shutdown: bool,
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            // 泄漏事实经 tracing::error 观察：adapter 层不持有 Diagnostics 句柄
            // （诊断属于 app 组装根），且 Drop 期报告存储随进程关闭同步消亡。
            tracing::error!(
                "VulkanContext: undrained Drop retained Vulkan parents: {}",
                error.short_what()
            );
            if let Some(device) = self.device_lease.take() {
                std::mem::forget(device);
            }
            if let Some(runtime) = self.runtime.take() {
                std::mem::forget(runtime);
            }
        }
    }
}

pub(crate) fn crop_cpu_shadow(
    shadow: &[u32],
    surface_width: i32,
    surface_height: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<Vec<u32>> {
    if width <= 0 || height <= 0 {
        return Ok(Vec::new());
    }
    if surface_width <= 0 || surface_height <= 0 {
        return Err(Error::new(
            Errc::InvalidState,
            format!(
                "VulkanContext: CPU shadow has invalid surface extent {surface_width}x{surface_height}"
            ),
        ));
    }
    if x < 0
        || y < 0
        || x.saturating_add(width) > surface_width
        || y.saturating_add(height) > surface_height
    {
        return Err(invalid(format!(
            "VulkanContext: readback rect ({x},{y},{width}x{height}) outside {surface_width}x{surface_height}"
        )));
    }
    let stride = surface_width as usize;
    let expected = stride
        .checked_mul(surface_height as usize)
        .ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: CPU shadow extent {surface_width}x{surface_height} exceeds host address space"
                ),
            )
        })?;
    if shadow.len() != expected {
        return Err(Error::new(
            Errc::InvalidState,
            format!(
                "VulkanContext: CPU shadow length {} does not match {surface_width}x{surface_height} ({expected} pixels)",
                shadow.len()
            ),
        ));
    }
    let output_pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!(
                    "VulkanContext: CPU readback extent {width}x{height} exceeds host address space"
                ),
            )
        })?;
    let mut out = allocate_readback_output(output_pixels)?;
    for row in 0..height as usize {
        let start = (y as usize + row)
            .checked_mul(stride)
            .and_then(|offset| offset.checked_add(x as usize))
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "VulkanContext: CPU shadow row offset overflowed",
                )
            })?;
        let end = start.checked_add(width as usize).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: CPU shadow row end overflowed",
            )
        })?;
        let source = shadow.get(start..end).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!("VulkanContext: CPU shadow row {row} is incomplete"),
            )
        })?;
        out.extend_from_slice(source);
    }
    Ok(out)
}

pub(crate) fn allocate_readback_output(pixel_count: usize) -> Result<Vec<u32>> {
    let mut output = Vec::new();
    output.try_reserve_exact(pixel_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: CPU readback output allocation for {pixel_count} pixels failed: {error}"
            ),
        )
    })?;
    Ok(output)
}
