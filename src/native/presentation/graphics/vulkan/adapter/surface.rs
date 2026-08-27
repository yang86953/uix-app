//! Vulkan 平台 surface 创建与 swapchain surface 策略。

use std::ffi::{CStr, c_void};

use ash::{Entry, vk};

use crate::core::Result;
#[cfg(windows)]
use crate::core::{Errc, Error};

#[cfg(all(unix, not(target_os = "macos")))]
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

#[cfg(any(windows, target_os = "macos"))]
use super::context::invalid;
use super::context::vk_err;

pub(super) fn destroy_failed_surface(
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) {
    if surface != vk::SurfaceKHR::null() {
        // SAFETY: surface 由同一 loader 创建，且失败清理路径只销毁一次。
        unsafe {
            surface_loader.destroy_surface(surface, None);
        }
    }
}

pub(super) fn surface_instance_extensions(entry: &Entry) -> (Vec<*const std::ffi::c_char>, bool) {
    let mut extensions = vec![ash::khr::surface::NAME.as_ptr()];
    #[cfg(all(unix, not(target_os = "macos")))]
    extensions.push(ash::khr::wayland_surface::NAME.as_ptr());
    #[cfg(windows)]
    extensions.push(ash::khr::win32_surface::NAME.as_ptr());
    #[cfg(target_os = "macos")]
    {
        extensions.push(ash::ext::metal_surface::NAME.as_ptr());
        extensions.push(ash::khr::portability_enumeration::NAME.as_ptr());
    }
    // swapchain maintenance1 的 device 扩展依赖下面两个 instance 扩展，必须成组启用。
    let surface_maintenance1 = instance_supports_surface_maintenance1(entry);
    if surface_maintenance1 {
        extensions.push(ash::khr::get_surface_capabilities2::NAME.as_ptr());
        extensions.push(ash::ext::surface_maintenance1::NAME.as_ptr());
    }
    (extensions, surface_maintenance1)
}

fn instance_supports_surface_maintenance1(entry: &Entry) -> bool {
    // SAFETY: entry 已加载；None 表示枚举全局 instance extensions。
    let available = match unsafe { entry.enumerate_instance_extension_properties(None) } {
        Ok(available) => available,
        Err(error) => {
            tracing::warn!(
                "VulkanContext: vkEnumerateInstanceExtensionProperties failed; surface maintenance disabled: {error:?}"
            );
            return false;
        }
    };
    let has_extension = |name: &CStr| {
        available.iter().any(|extension| {
            // SAFETY: Vulkan 保证 extension_name 是结构体内以 NUL 结尾的固定数组。
            (unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) }) == name
        })
    };
    has_extension(ash::khr::get_surface_capabilities2::NAME)
        && has_extension(ash::ext::surface_maintenance1::NAME)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) type PlatformSurfaceLoader = ash::khr::wayland_surface::Instance;

#[cfg(windows)]
pub(super) type PlatformSurfaceLoader = ash::khr::win32_surface::Instance;

#[cfg(target_os = "macos")]
pub(super) type PlatformSurfaceLoader = ash::ext::metal_surface::Instance;

pub(super) fn create_platform_surface(
    entry: &Entry,
    instance: &ash::Instance,
    native_surface: *mut c_void,
) -> Result<(vk::SurfaceKHR, PlatformSurfaceLoader)> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // SAFETY: native_surface 必须是平台层传入且在 context 生命周期内有效的 Wayland 句柄包。
        let wayland = unsafe { WaylandSurfaceHandle::from_native(native_surface)? };
        let wayland_surface_loader = ash::khr::wayland_surface::Instance::new(entry, instance);
        let surface_info = vk::WaylandSurfaceCreateInfoKHR::default()
            .display(wayland.display.cast())
            .surface(wayland.surface.cast());
        // SAFETY: display/surface 来自有效 Wayland 连接，instance 已启用对应扩展。
        let surface = unsafe { wayland_surface_loader.create_wayland_surface(&surface_info, None) }
            .map_err(|err| vk_err("vkCreateWaylandSurfaceKHR", err))?;
        Ok((surface, wayland_surface_loader))
    }
    #[cfg(windows)]
    {
        if native_surface.is_null() {
            return Err(invalid(
                "VulkanContext: Win32 HWND native_surface must not be null",
            ));
        }
        // SAFETY: None 请求当前进程模块句柄，不借用外部字符串。
        let hinstance = unsafe { GetModuleHandleW(None) }.map_err(|err| {
            Error::new(
                Errc::PlatformError,
                format!("VulkanContext: GetModuleHandleW failed: {err:?}"),
            )
        })?;
        let win32_surface_loader = ash::khr::win32_surface::Instance::new(entry, instance);
        let surface_info = vk::Win32SurfaceCreateInfoKHR::default()
            .hinstance(hinstance.0 as vk::HINSTANCE)
            .hwnd(native_surface as vk::HWND);
        // SAFETY: native_surface 已验证非空，且由调用方保证为当前进程存活 HWND。
        let surface = unsafe { win32_surface_loader.create_win32_surface(&surface_info, None) }
            .map_err(|err| vk_err("vkCreateWin32SurfaceKHR", err))?;
        Ok((surface, win32_surface_loader))
    }
    #[cfg(target_os = "macos")]
    {
        if native_surface.is_null() {
            return Err(invalid(
                "VulkanContext: CAMetalLayer native_surface must not be null",
            ));
        }
        let metal_surface_loader = ash::ext::metal_surface::Instance::new(entry, instance);
        let surface_info = vk::MetalSurfaceCreateInfoEXT::default()
            .layer(native_surface as *const vk::CAMetalLayer);
        // SAFETY: native_surface 已验证非空，且 AppKit 在 context 生命周期内持有 CAMetalLayer。
        let surface = unsafe { metal_surface_loader.create_metal_surface(&surface_info, None) }
            .map_err(|err| vk_err("vkCreateMetalSurfaceEXT", err))?;
        Ok((surface, metal_surface_loader))
    }
}

pub(crate) fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        };
    }
    formats
        .iter()
        .copied()
        .find(|format| {
            format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .or_else(|| formats.first().copied())
        .unwrap_or(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        })
}

pub(super) fn choose_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    modes
        .iter()
        .copied()
        // MAILBOX 保持无撕裂且不会把交互帧排入 FIFO 队尾，优先降低切页输入延迟。
        .find(|mode| *mode == vk::PresentModeKHR::MAILBOX)
        // Vulkan 保证 FIFO 可用；不支持 MAILBOX 的设备继续使用稳定垂直同步路径。
        .or_else(|| {
            modes
                .iter()
                .copied()
                .find(|mode| *mode == vk::PresentModeKHR::FIFO)
        })
        .or_else(|| modes.first().copied())
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

pub(crate) fn choose_composite_alpha(
    supported: vk::CompositeAlphaFlagsKHR,
) -> Option<vk::CompositeAlphaFlagsKHR> {
    #[cfg(all(unix, not(target_os = "macos")))]
    let candidates = [
        // Wayland 客户端装饰需要把圆角外像素交给 compositor 合成。
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ];
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    let candidates = [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ];
    candidates
        .into_iter()
        .find(|candidate| supported.contains(*candidate))
}

pub(super) fn choose_extent(
    caps: vk::SurfaceCapabilitiesKHR,
    requested: vk::Extent2D,
) -> vk::Extent2D {
    if caps.current_extent.width != u32::MAX {
        return caps.current_extent;
    }
    vk::Extent2D {
        width: requested
            .width
            .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
        height: requested
            .height
            .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
    }
}
