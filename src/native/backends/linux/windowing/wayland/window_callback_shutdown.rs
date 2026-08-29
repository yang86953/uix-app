// Wayland window callback shutdown Component 确定性注销逐窗 xdg-shell 回调。

// 窗口 owner 类型继续持有逐窗协议 handles 与生命周期编排权。
use super::window_ops::WaylandWindowOps;

// 为逐窗 owner 提供不发送协议请求的幂等 callback teardown 端口。
impl WaylandWindowOps {
    // 注销并释放当前窗口的 frame 与 xdg-shell callback owners。
    pub(super) fn shutdown_window_callbacks(&mut self) {
        // surface 关闭前先消费仍等待 Done 的 frame request 与 callback owner。
        self.frame_callback.shutdown();
        // xdg-shell owners 中 pending activation token 必须先于目标 surface 注销。
        if let Some(activation_token) = self.activation_token.take() {
            // 窗口关闭后不得再处理迟到 Done 或激活旧 surface。
            activation_token.clear_callback();
        }
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
        // layer surface 与 xdg_surface 互斥；桌面窗口关闭时注销其 configure/closed callback。
        if let Some(layer_surface) = self.layer_surface.take() {
            // 协议 destructor 先通知 compositor 释放桌面 role。
            layer_surface.destroy();
            layer_surface.clear_callback();
        }
    }
}
