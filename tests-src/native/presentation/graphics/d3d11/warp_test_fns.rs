// 各项自带原 cfg 门控（test/feature/平台组合），在源文件模块作用域内 include! 展开。

#[cfg(all(test, feature = "d3d11"))]
use crate::platform::presentation::GpuRecipeContext;

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
