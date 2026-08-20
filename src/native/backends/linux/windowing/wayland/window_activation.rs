// Wayland window activation Component 异步取得 token 后提交逐窗激活请求。

// 引入稳定错误分类与同步请求建立结果。
use crate::core::error::{Errc, Error, Result};
// 引入 activation-token Done 事件。
use wayland_protocols::xdg::activation::v1::client::xdg_activation_token_v1;

// 窗口 owner 类型继续持有 activation global、surface 与 pending token。
use super::window_ops::WaylandWindowOps;

// 为 WaylandWindowOps 提供单一异步 activation request 端口。
impl WaylandWindowOps {
    // 建立一个等待 compositor token 的非阻塞窗口激活请求。
    pub(super) fn request_activation(&mut self) -> Result<()> {
        // 克隆 activation global 供 Done callback 提交最终请求。
        let activation = self
            // 借用当前窗口绑定的可选协议能力。
            .xdg_activation
            // 缺少扩展时返回稳定能力缺失，不伪造成功。
            .as_ref()
            // 生成调用方可分类的 typed error。
            .ok_or_else(|| {
                // 明确缺少的是 Wayland activation 扩展。
                Error::new(Errc::NotImplemented, "xdg_activation_v1 is unavailable")
                // 结束能力缺失错误构造。
            })?
            // callback 与窗口 owner 共享同一协议连接身份。
            .clone();
        // 克隆目标 surface，确保 Done callback 执行期间身份仍存活。
        let surface = self
            // 借用当前窗口的活动原生 surface。
            .surface
            // 已关闭窗口必须返回稳定状态错误。
            .as_ref()
            // 复用窗口操作统一的缺失代理诊断。
            .ok_or_else(|| Self::missing_proxy("os_raise", "wl_surface"))?
            // callback 只持有协议 handle，不持有应用窗口对象。
            .clone();
        // 新请求替换旧请求前先注销其 callback owner。
        if let Some(previous) = self.activation_token.take() {
            // 迟到旧 token 不得再激活当前或已关闭 surface。
            previous.clear_callback();
        }
        // 从 backend 注入的 activation global 创建一次性 token owner。
        let token = activation.get_activation_token();
        // token 与目标 surface 绑定，限制激活请求作用域。
        token.set_surface(&surface);
        // app-id 与 xdg_toplevel 初始化值保持一致。
        token.set_app_id("uix-app".to_string());
        // callback 只捕获协议 handles，不访问应用状态或事件队列。
        let callback_activation = activation.clone();
        // callback 保持目标 surface 存活直到 token Done。
        let callback_surface = surface.clone();
        // 注册 callback 必须先于 commit，避免丢失快速返回的 Done。
        token.quick_assign(move |_, event, _| {
            // activation token 只有 Done 能交付 compositor 生成的真实 token。
            if let xdg_activation_token_v1::Event::Done { token } = event {
                // 使用 compositor 返回值提交最终异步激活请求，禁止空 token。
                callback_activation.activate(token, &callback_surface);
            }
        });
        // callback 就绪后才向 compositor 提交 token 请求。
        token.commit();
        // 窗口 owner 保存 pending handle，供新 raise 或 teardown 主动注销。
        self.activation_token = Some(token);
        // 成功仅表示异步请求已建立，不表示 compositor 已授予焦点。
        Ok(())
    }
}
