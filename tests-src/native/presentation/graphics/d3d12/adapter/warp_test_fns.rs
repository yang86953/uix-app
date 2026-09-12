// 各项自带原 cfg 门控（test/feature/平台组合），在源文件模块作用域内 include! 展开。

#[cfg(all(test, feature = "d3d12"))]
use crate::platform::presentation::GpuRecipeContext;

#[cfg(all(test, windows, feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
    D3d12Context::new_with_driver(surface, width, height, adapter::D3d12DriverKind::Warp)
        .map(|context| Box::new(context) as _)
}

#[cfg(all(test, windows, feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    true
}



#[cfg(all(test, not(windows), feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "D3D12 WARP tests are only supported on Windows",
    ))
}



#[cfg(all(test, not(windows), feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    false
}
