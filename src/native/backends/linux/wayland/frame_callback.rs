// Wayland frame callback Component：把 compositor Done 转换为逐窗 one-shot 事件。

// 使用 FIFO 保存目标窗口事件。
use std::collections::VecDeque;
// 共享 request owner 与窗口事件队列。
use std::sync::{Arc, Mutex};
// 保留 callback 到达的单调时间。
use std::time::Instant;

// 引入稳定错误分类、窗口身份与 typed Result。
use crate::core::{Errc, Error, Result, WindowId};
// 引入 frame event 与 one-shot request 契约。
use crate::native::windowing::event::UiEvent;
// 引入当前原生 frame 请求值。
use crate::native::windowing::window::NativeFrameRequest;

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
