//! Platform raw-surface bridge for the shared wgpu renderer.

use std::ffi::c_void;
#[cfg(windows)]
use std::num::NonZeroIsize;
#[cfg(all(unix, not(target_os = "macos")))]
use std::ptr::NonNull;

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_HINSTANCE},
};

use crate::core::{Errc, Error, Result};

pub(super) struct DrawableExtent {
    pub(super) logical_width: i32,
    pub(super) logical_height: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn drawable_extent(
    native_surface: *mut c_void,
    logical_width: i32,
    logical_height: i32,
) -> DrawableExtent {
    #[cfg(windows)]
    {
        let size = crate::native::presentation::graphics::platform::windows::drawable_size(
            native_surface,
            logical_width,
            logical_height,
        );
        DrawableExtent {
            logical_width: size.logical_width,
            logical_height: size.logical_height,
            width: size.width.max(1) as u32,
            height: size.height.max(1) as u32,
        }
    }
    #[cfg(not(windows))]
    {
        DrawableExtent {
            logical_width: logical_width.max(1),
            logical_height: logical_height.max(1),
            width: logical_width.max(1) as u32,
            height: logical_height.max(1) as u32,
        }
    }
}

pub(super) unsafe fn create_surface(
    instance: &wgpu::Instance,
    native_surface: *mut c_void,
) -> Result<wgpu::Surface<'static>> {
    if native_surface.is_null() {
        return Err(Error::new(
            Errc::PlatformError,
            "wgpu native surface is null",
        ));
    }
    #[cfg(all(target_os = "macos", not(feature = "metal")))]
    {
        let _ = (instance, native_surface);
        return Err(Error::new(
            Errc::PlatformError,
            "macOS wgpu surface requires the `metal` feature",
        ));
    }
    #[cfg(windows)]
    let target = {
        let hwnd = NonZeroIsize::new(native_surface as isize)
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu Win32 HWND is null"))?;
        let hinstance = NonZeroIsize::new(GetWindowLongPtrW(HWND(native_surface), GWLP_HINSTANCE))
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu Win32 HINSTANCE is null"))?;
        let mut window_handle = raw_window_handle::Win32WindowHandle::new(hwnd);
        window_handle.hinstance = Some(hinstance);
        wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Windows(
                raw_window_handle::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: RawWindowHandle::Win32(window_handle),
        }
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let target = {
        let descriptor =
            crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle::from_native(
                native_surface,
            )?;
        let display = NonNull::new(descriptor.display)
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu Wayland display is null"))?;
        let surface = NonNull::new(descriptor.surface)
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu Wayland surface is null"))?;
        wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Wayland(
                raw_window_handle::WaylandDisplayHandle::new(display),
            )),
            raw_window_handle: RawWindowHandle::Wayland(
                raw_window_handle::WaylandWindowHandle::new(surface),
            ),
        }
    };
    #[cfg(all(target_os = "macos", feature = "metal"))]
    let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(native_surface);

    #[cfg(any(not(target_os = "macos"), feature = "metal"))]
    {
        instance.create_surface_unsafe(target).map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("wgpu create_surface failed: {error}"),
            )
        })
    }
}
