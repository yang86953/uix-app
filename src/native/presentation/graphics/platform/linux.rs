//! Linux native GPU surface descriptors.

#![cfg(all(unix, not(target_os = "macos")))]
// 在 Rust 2024 下禁止 unsafe 函数体隐式扩大底层操作范围。
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WaylandSurfaceHandle {
    pub(crate) display: *mut c_void,
    pub(crate) surface: *mut c_void,
}

impl WaylandSurfaceHandle {
    pub(crate) fn new(display: *mut c_void, surface: *mut c_void) -> Self {
        Self { display, surface }
    }

    pub(crate) fn is_valid(self) -> bool {
        !self.display.is_null() && !self.surface.is_null()
    }

    pub(crate) unsafe fn from_native(native_surface: *mut c_void) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "WaylandSurfaceHandle: native surface descriptor is null",
            ));
        }
        // SAFETY：native_surface 由调用方保证来自受信来源，且非空（上文已校验）；
        // 解引用得到的是按值复制的句柄描述，不长期持有原生指针。
        let handle = unsafe { *(native_surface as *const WaylandSurfaceHandle) };
        if !handle.is_valid() {
            return Err(Error::new(
                Errc::PlatformError,
                "WaylandSurfaceHandle: display or surface pointer is null",
            ));
        }
        Ok(handle)
    }
}
