// 各项自带原 cfg 门控（test/feature/平台组合），在源文件模块作用域内 include! 展开。

#[cfg(all(test, feature = "d3d11"))]
use crate::platform::presentation::GpuRecipeContext;

// Windows 测试目标保留 D3D11 WARP 平台入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, windows, feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
    D3d11Context::new_warp_test_context(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

// Windows 测试目标保留 D3D11 WARP 可用性探测，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, windows, feature = "d3d11"))]
pub(crate) const fn warp_test_context_available() -> bool {
    true
}

// 非 Windows 测试目标保留明确的 D3D11 WARP 不支持入口。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "D3D11 WARP tests are only supported on Windows",
    ))
}

// 非 Windows 测试目标保留明确的 D3D11 WARP 不可用性探测。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(crate) const fn warp_test_context_available() -> bool {
    false
}
