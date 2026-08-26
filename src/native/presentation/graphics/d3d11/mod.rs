//! Direct3D 11 graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::graphics::GpuAdapterInfo;
use crate::platform::presentation::GraphicsContextCandidate;
// D3D11 WARP 测试入口返回类型化 GPU context trait object。
#[cfg(all(test, feature = "d3d11"))]
use crate::platform::presentation::GpuRecipeContext;

#[path = "adapter/mod.rs"]
pub(crate) mod platform;

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 向 registry 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}

// 原生 factory 只经本模块入口选择 D3D11 adapter，不穿透其内部目录。
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    platform::enumerate_adapters()
}

// 显式 Windows parity feature 只转发生产 D3D11 Adapter 的真实 GPU harness。
#[cfg(all(windows, feature = "d3d11-parity-test"))]
pub(crate) fn run_gpu_parity_test() {
    platform::run_gpu_parity_test();
}

// 测试目标保留 D3D11 WARP 包装入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
    platform::create_warp_test_context(surface, width, height)
}

// 测试目标保留 D3D11 WARP 可用性包装入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn warp_test_context_available() -> bool {
    platform::warp_test_context_available()
}
