//! `src/native/presentation/graphics/d3d12/adapter/context/methods.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl D3d12Context） ——

impl D3d12Context {
    #[cfg(test)]
    pub(crate) fn new_with_driver(
        native_window: *mut c_void,
        width: i32,
        height: i32,
        driver: D3d12DriverKind,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(platform_error("D3d12Context: native window handle is null"));
        }
        // SAFETY: 无指针输入参数，返回的工厂对象由 windows crate 类型接管，失败走 HRESULT 返回。
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| d3d12_error("CreateDXGIFactory2", error))?;
        Self::create_with_factory(native_window, width, height, factory, driver)
    }
}
