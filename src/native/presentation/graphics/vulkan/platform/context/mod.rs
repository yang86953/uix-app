//! Vulkan graphics context for Linux Wayland, Windows Win32, and macOS MoltenVK
//! (PixelUpload via shared swapchain stage/present).

use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::native::present::{GraphicsApi, GraphicsContextCaps, IGraphicsContext, PresentDamage};

#[cfg(target_os = "macos")]
use crate::native::backends::macos::platform as macos_surface;

use super::adapter::{select_queue, VulkanAdapterInfo};
pub(crate) use super::device::staging_size;
use super::device::{VulkanDevice, VulkanRuntime};
use super::drawable::drawable_size;
use super::surface::{
    choose_composite_alpha, choose_extent, choose_present_mode, choose_surface_format,
    create_platform_surface, destroy_failed_surface,
};

mod graphics;
mod methods;
mod swapchain;
mod transfer;

pub(crate) use swapchain::allocate_image_layouts;
#[cfg(test)]
pub(crate) use swapchain::PresentCompletion;
#[cfg(test)]
pub(crate) use transfer::allocate_cpu_shadow;

use swapchain::{
    allocate_presented_images, create_render_finished_semaphores, destroy_semaphores,
    PresentFenceSet, PresentLifetime,
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

pub struct VulkanContext {
    runtime: Option<Rc<VulkanRuntime>>,
    device_lease: Option<Rc<VulkanDevice>>,
    surface_loader: ash::khr::surface::Instance,
    #[cfg(all(unix, not(target_os = "macos")))]
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
    queue: vk::Queue,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    image_layouts: Vec<vk::ImageLayout>,
    swapchain_format: vk::Format,
    extent: vk::Extent2D,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    upload: UploadBuffer,
    image_available: vk::Semaphore,
    render_finished: Vec<vk::Semaphore>,
    present_fences: PresentFenceSet,
    present_lifetime: PresentLifetime,
    frame_fence: vk::Fence,
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
