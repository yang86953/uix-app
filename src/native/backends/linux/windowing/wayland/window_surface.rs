// Wayland 逐窗 surface descriptor、装饰映射与注册注销辅助 Component。

// Wayland 装饰协议模式用于映射统一标题栏可见性。
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode as XdgDecoMode;

// typed error 保持缺失代理与注册注销失败的稳定分类。
use crate::core::error::{Errc, Error, Result};
// 原生 descriptor 类型用于向 graphics 暴露带 metrics 的窄句柄。
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle;

// 跨注册表 Component 原子撤销 surface 的授权和事件路由事实。
use super::surface_registration::unregister_window_surface;
// 当前窗口 owner 只把窄 surface 操作委托到本 Component。
use super::window_ops::WaylandWindowOps;

impl WaylandWindowOps {
    // 将统一的系统标题栏可见性映射为 Wayland 装饰模式。
    pub(super) fn title_bar_decoration_mode(visible: bool) -> XdgDecoMode {
        // 可见系统标题栏请求 compositor 绘制服装饰。
        if visible {
            // 服务端装饰对应原生标题栏。
            XdgDecoMode::ServerSide
        } else {
            // 客户端装饰把标题栏区域交给 UIX 自己绘制。
            XdgDecoMode::ClientSide
        }
    }

    // 客户端装饰接管时窗口内容圆角的逻辑半径。
    // 与 presentation 侧 WaylandSurfaceSnapshot 的圆角事实保持同一取值。
    pub(super) const CLIENT_CORNER_RADIUS_LOGICAL: i32 = 10;

    // 同一 Wayland 窗口父模块下的私有 Components 共享稳定缺失代理诊断。
    pub(super) fn missing_proxy(operation: &str, proxy: &str) -> Error {
        // 缺失活动协议 owner 是稳定窗口生命周期错误。
        Error::new(
            // 使用 InvalidState 区分能力缺失与已关闭 owner。
            Errc::InvalidState,
            // 保留操作和缺失代理名称。
            format!("{operation}: Wayland {proxy} is unavailable"),
        )
    }

    // 获取窗口生命周期内有效的 wl_surface 原始 C 指针。
    pub(crate) fn surface_c_ptr(&self) -> *mut std::ffi::c_void {
        // 没有活动 surface 时返回空指针，禁止 graphics 接纳 descriptor。
        self.surface
            // 借用当前 surface owner。
            .as_ref()
            // compat Main 直接提供底层 wl_proxy 指针。
            .map_or(std::ptr::null_mut(), |surface| surface.c_ptr())
    }

    // 返回带逐窗 metrics 的 native surface descriptor 指针。
    pub(super) fn native_surface_descriptor_ptr(&self) -> *mut std::ffi::c_void {
        // 只有 display 与 surface 均有效时才能向 graphics 暴露 descriptor。
        if self.native_surface.is_valid() {
            // descriptor 由窗口 owner 持有，调用方只在构造 context 时读取并克隆。
            &self.native_surface as *const WaylandSurfaceHandle as *mut std::ffi::c_void
        } else {
            // 未初始化或已失效的 descriptor 不得伪装成原生 surface。
            std::ptr::null_mut()
        }
    }

    // 检查式注销当前 surface，并只在事务成功后清除可重试身份。
    pub(super) fn unregister_surface(&mut self) -> Result<()> {
        // 重复注销保持幂等，不触碰任何共享注册表。
        let Some(surface_id) = self.surface_id else {
            // 当前窗口已经没有活动 surface 注册事实。
            return Ok(());
        };
        // 私有 Component 在任一修改前取得两份健康注册表 guard。
        unregister_window_surface(
            // 传入 raw pointer activation 的唯一共享 owner。
            &self.pointer_activations,
            // 传入 surface 到窗口身份的唯一共享 owner。
            &self.surface_windows,
            // 注销当前协议 surface 编号。
            surface_id,
            // 限定只能删除仍属于当前窗口的授权注册。
            self.window_id,
        )?;
        // 两份注册表均成功注销后才消费窗口持有的可重试身份。
        self.surface_id = None;
        // 显式报告注销事务成功。
        Ok(())
    }
}
