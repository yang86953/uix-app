// 各项自带原 cfg 门控（test/feature/平台组合），在源文件模块作用域内 include! 展开。

// 为测试专用 WARP context 建立线程亲和 wrapper。
#[cfg(all(test, any(feature = "d3d11", feature = "d3d12")))]
fn bind_test_context_to_current_thread(
    // 接收测试 adapter 新创建的类型化 GPU context。
    context: Box<dyn crate::platform::presentation::GpuRecipeContext>,
) -> Box<dyn crate::platform::presentation::GpuRecipeContext> {
    // WARP 测试只验证原生资源与生命周期，不再反向探测静态 capability。
    let bound = thread_bound::bind_to_current_thread(
        // 固化 WARP 测试入口的 GPU recipe 类型。
        crate::platform::presentation::GraphicsRecipeContext::Gpu(context),
    );
    // 取回线程绑定后的同一 GPU recipe 分支。
    match bound {
        // 返回不可选的 GPU owner。
        crate::platform::presentation::GraphicsRecipeContext::Gpu(context) => context,
        // 构造输入固定为 GPU，因此该分支不可达。
        crate::platform::presentation::GraphicsRecipeContext::PixelUpload(_) => {
            // 防止未来 bind 实现破坏 recipe 保持契约。
            unreachable!("GPU test context binding changed recipe variant")
        }
    }
}

// 测试目标保留 D3D11 WARP 工厂入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_d3d11_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::platform::presentation::GpuRecipeContext>> {
    crate::native::presentation::graphics::d3d11::create_warp_test_context(surface, width, height)
        // 在测试 factory 边界建立线程门禁。
        .map(bind_test_context_to_current_thread)
}

// 测试目标保留 D3D11 WARP 可用性探测，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn d3d11_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d11::warp_test_context_available()
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::platform::presentation::GpuRecipeContext>> {
    crate::native::presentation::graphics::d3d12::create_warp_test_context(surface, width, height)
        // 在测试 factory 边界建立线程门禁。
        .map(bind_test_context_to_current_thread)
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn d3d12_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d12::warp_test_context_available()
}
