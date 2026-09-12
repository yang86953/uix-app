//! `src/native/presentation/graphics/d3d11/adapter/context/methods.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl D3d11Context） ——

impl D3d11Context {
    /// Deterministic WARP-only constructor for crate tests.
    ///
    /// Production construction deliberately keeps the hardware-then-WARP
    /// policy in [`Self::new`]. Tests that compare backend pixels must not
    /// inherit a machine-specific hardware adapter instead.
    // 测试目标保留确定性 WARP 构造器，供显式后端像素测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn new_warp_test_context(
        native_window: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let drawable = win_surface::drawable_size(native_window, width, height);
        create_with_drawable(
            native_window,
            drawable,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
    }
}
