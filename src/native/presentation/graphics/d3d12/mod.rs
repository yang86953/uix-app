//! Direct3D 12 graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::graphics::GpuAdapterInfo;
use crate::platform::presentation::GraphicsContextCandidate;
// D3D12 WARP 测试入口返回类型化 GPU context trait object。

#[path = "adapter/mod.rs"]
pub(crate) mod platform;

// 原生 factory 只经本模块入口选择 D3D12 adapter，不穿透其内部目录。
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    platform::enumerate_adapters()
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}

// 仅测试构建的 WARP/绘制辅助位于 tests-src，经模块级 include! 保持原作用域。
#[cfg(test)]
include!("../../../../../tests-src/native/presentation/graphics/d3d12/warp_test_fns.rs");
