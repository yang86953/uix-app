// Wayland window callback shutdown Component 确定性注销逐窗 xdg-shell 回调。

// 窗口 owner 类型继续持有逐窗协议 handles 与生命周期编排权。
use super::window_ops::WaylandWindowOps;

// 为逐窗 owner 提供不发送协议请求的幂等 callback teardown 端口。
impl WaylandWindowOps {
    // 注销并释放当前窗口的 xdg_toplevel 与 xdg_surface callback owners。
    pub(super) fn shutdown_window_callbacks(&mut self) {
        // 先取出 toplevel handle，阻止后续入口重新观察旧 owner。
        if let Some(toplevel) = self.toplevel.take() {
            // handle 仍存活时从兼容注册表注销精确 callback。
            toplevel.clear_callback();
        }
        // 再取出依赖更底层 surface role 的 xdg_surface handle。
        if let Some(xdg_surface) = self.xdg_surface.take() {
            // 在释放 handle 前注销 configure callback 与其捕获状态。
            xdg_surface.clear_callback();
        }
    }
}
