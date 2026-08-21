// Wayland frame callback Component：把 compositor Done 转换为逐窗 one-shot 事件。

// 使用 FIFO 保存目标窗口事件。
use std::collections::VecDeque;
// 共享 request owner 与窗口事件队列。
use std::sync::{Arc, Mutex};
// 保留 callback 到达的单调时间。
use std::time::Instant;

// 引入 wl_callback 协议 handle，由逐窗 Component 显式持有。
use wayland_client::protocol::wl_callback;

// 引入稳定错误分类、窗口身份与 typed Result。
use crate::core::{Errc, Error, Result, WindowId};
// 引入 frame event 与 one-shot request 契约。
use crate::platform::windowing::event::{FrameRequestToken, UiEvent};
// 引入当前原生 frame 请求值。
use crate::platform::windowing::window::NativeFrameRequest;

// 引入兼容层协议 handle，使 owner 可主动注销 callback registry 项。
use super::compat::Main;

// 统一持有单窗口 active request 与尚未完成的协议 callback。
pub(super) struct FrameCallbackOwner {
    // active request 由 compositor callback 闭包共享并检查式消费。
    active: Arc<Mutex<Option<NativeFrameRequest>>>,
    // 当前未被替换、取消或 teardown 的 wl_callback handle。
    callback: Option<Main<wl_callback::WlCallback>>,
}

// FrameCallbackOwner 独占逐窗 frame callback 生命周期转换。
impl FrameCallbackOwner {
    // 构造没有 active request 或协议 callback 的 owner。
    pub(super) fn new() -> Self {
        // 同时初始化业务 request 与协议 owner 两份状态。
        Self {
            // callback 闭包稍后共享同一 active request owner。
            active: Arc::new(Mutex::new(None)),
            // 新窗口尚未请求 compositor frame opportunity。
            callback: None,
        }
    }

    // 登记新 request，并在不同 request 替换时注销旧 callback。
    pub(super) fn prepare_request(&mut self, request: NativeFrameRequest) -> Result<bool> {
        // request owner 损坏必须在任何 callback 生命周期修改前同步返回。
        let mut active = self.active.lock().map_err(|_| {
            // 构造稳定的 request 登记阶段错误。
            Error::new(
                // active request owner 已无法安全访问。
                Errc::InvalidState,
                // 保留原生 frame 请求登记阶段。
                "Wayland frame request mutex poisoned during registration",
            )
        })?;
        // 相同 request 已有在途 callback 时保持严格去重。
        if active.as_ref() == Some(&request) {
            // false 表示调用方不得创建第二个协议 callback。
            return Ok(false);
        }
        // 健康 owner 先发布新 request，使迟到旧 callback 只能被安全忽略。
        *active = Some(request);
        // callback registry 操作不得持有 active request mutex。
        drop(active);
        // 不同 request 建立前消费旧 callback owner，避免等待可能永不到达的 Done。
        self.clear_callback();
        // true 表示调用方需要创建并接线新的 wl_callback。
        Ok(true)
    }

    // 克隆 callback 闭包需要的 active request owner。
    pub(super) fn active_source(&self) -> Arc<Mutex<Option<NativeFrameRequest>>> {
        // Arc clone 不复制 request 状态或建立第二 authority。
        Arc::clone(&self.active)
    }

    // 接管已经接线的新 wl_callback handle。
    pub(super) fn attach_callback(&mut self, callback: Main<wl_callback::WlCallback>) {
        // prepare_request 已保证旧 callback owner 被消费。
        self.callback = Some(callback);
    }

    // 仅在 token 匹配当前 request 时取消业务状态与协议 callback owner。
    pub(super) fn cancel_checked(&mut self, token: FrameRequestToken) -> Result<()> {
        // poisoned owner 必须在 callback 注销前同步返回。
        let mut active = self.active.lock().map_err(|_| {
            // 构造稳定的 request 取消阶段错误。
            Error::new(
                // active request owner 已无法安全访问。
                Errc::InvalidState,
                // 保留原生 frame 请求取消阶段。
                "Wayland frame request mutex poisoned during cancellation",
            )
        })?;
        // 只允许精确 token 消费当前 request。
        let matched = active
            // 借用当前 request 而不移动业务值。
            .as_ref()
            // 对存在的 request 比较公开 token。
            .is_some_and(|request| request.token == token);
        // token 不匹配时必须保留 active 与 callback 两份状态。
        if !matched {
            // 返回成功表示取消目标已不存在或已被替换。
            return Ok(());
        }
        // 匹配 request 先清除业务状态，迟到 Done 将被忽略。
        *active = None;
        // callback registry 操作不得持有 active request mutex。
        drop(active);
        // 主动注销匹配 request 的协议 callback owner。
        self.clear_callback();
        // 两份 owner 均已消费。
        Ok(())
    }

    // 显式窗口关闭在健康 request owner 上执行 checked 清理。
    pub(super) fn close_checked(&mut self) -> Result<()> {
        // poisoned owner 必须保留 callback 与窗口协议对象供上层处理。
        let mut active = self.active.lock().map_err(|_| {
            // 构造稳定的窗口关闭阶段错误。
            Error::new(
                // 共享请求状态已无法安全访问。
                Errc::InvalidState,
                // 保留 frame request 与窗口关闭阶段。
                "Wayland frame request mutex poisoned during window close",
            )
        })?;
        // 健康 owner 在 callback 注销前先失效业务 request。
        *active = None;
        // callback registry 操作不得持有 active request mutex。
        drop(active);
        // 显式关闭主动注销尚未 Done 的 callback。
        self.clear_callback();
        // checked 清理完成。
        Ok(())
    }

    // 无同步错误接收方的 fatal/Drop teardown 确定性消费两份 owner。
    pub(super) fn shutdown(&mut self) {
        // teardown 恢复中毒 guard 只用于清空失效 request，不读取业务值。
        let mut active = self
            // 访问单窗口 active request owner。
            .active
            // teardown 仍等待唯一可变访问。
            .lock()
            // 无同步接收方时恢复 guard 以完成资源清理。
            .unwrap_or_else(|error| error.into_inner());
        // 失效任何尚未由 compositor 消费的 request。
        *active = None;
        // callback registry 操作不得持有 active request mutex。
        drop(active);
        // 只注销本地 callback，不发送 Wayland 协议请求。
        self.clear_callback();
    }

    // 消费当前协议 callback owner并从 compat registry 注销闭包。
    fn clear_callback(&mut self) {
        // take 先清空 owner 槽，保证清理幂等。
        if let Some(callback) = self.callback.take() {
            // clear_callback 只移除本地 registry owner。
            callback.clear_callback();
        }
    }
}

// 消费一个已经由 compositor 完成的匹配 frame request。
pub(crate) fn deliver_frame_opportunity(
    // 借用单窗口唯一 active request owner。
    active: &Arc<Mutex<Option<NativeFrameRequest>>>,
    // 借用 callback 与 App owner thread 之间的唯一事件队列。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 保留本次底层 callback 对应的精确 request token。
    request: NativeFrameRequest,
    // 标记 frame opportunity 所属窗口。
    window_id: WindowId,
    // 记录 callback 实际到达的单调时间。
    callback_at: Instant,
) -> Result<()> {
    // active owner 损坏时不得恢复访问或伪装为迟到 callback。
    let mut active = active.lock().map_err(|_| {
        // 构造可进入 backend pending failure source 的稳定错误。
        Error::new(
            // request owner 损坏属于稳定状态失败。
            Errc::InvalidState,
            // 保留 frame callback 消费阶段。
            "Wayland frame callback request mutex poisoned",
        )
    })?;
    // 旧 token 或已经取消的 callback 继续作为正常迟到事件忽略。
    if active.as_ref() != Some(&request) {
        // 不触碰新 request，也不生成窗口事件。
        return Ok(());
    }
    // compositor 已消费本次 one-shot；先清除 active，禁止留下幽灵 request。
    *active = None;
    // 在获取事件队列前释放 request owner，保持锁作用域最小。
    drop(active);
    // 事件队列损坏必须成为 typed failure，不能恢复写入。
    let mut events = events.lock().map_err(|_| {
        // 构造可进入 backend pending failure source 的稳定错误。
        Error::new(
            // callback 无法投递逐窗事实属于稳定状态失败。
            Errc::InvalidState,
            // 保留 frame callback 事件投递阶段。
            "Wayland frame callback event queue mutex poisoned",
        )
    })?;
    // 健康队列接收本次 token 的唯一 frame opportunity。
    events.push_back(
        // 保留原 token、callback 时间与无预测时间语义。
        UiEvent::frame_opportunity(request.token, callback_at, None)
            // 把事件绑定到稳定窗口身份。
            .for_window(window_id),
    );
    // 确认 one-shot request 已消费且事件已投递。
    Ok(())
}
