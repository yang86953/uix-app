// Wayland 文件拖放窗口 Adapter 校验能力、生命周期并提交逐窗开关。

// 共享 owner 在窗口对象与 data-device callback 之间同步。
use std::sync::{Arc, Mutex};

// typed errors 区分能力缺失、关闭窗口与状态损坏。
use crate::core::{Errc, Error, Result, WindowId};

// 状态 Component 保存逐窗启用事实与相关在途 owners。
use super::file_drop::WaylandFileDropState;

// 更新一个仍存活窗口的文件拖放能力。
pub(crate) fn set_window_capability(
    // backend 唯一拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 稳定窗口身份。
    window_id: WindowId,
    // surface 注册事实界定窗口生命周期。
    surface_live: bool,
    // seat 绑定后实际 data-device owner 的能力事实。
    available: bool,
    // true 启用，false 禁用。
    enable: bool,
) -> Result<()> {
    // 已注销 surface 不允许重新发布接收资格。
    if !surface_live {
        // 返回稳定生命周期错误。
        return Err(Error::new(
            // 缺失活动 surface 属于 InvalidState。
            Errc::InvalidState,
            // 诊断命名公共窗口入口。
            "os_enable_file_drop: Wayland wl_surface is unavailable",
        ));
    }
    // 可选协议 global 缺失时不得伪造成功。
    if !available {
        // 能力缺失使用 NotImplemented。
        return Err(Error::new(
            // 保持跨平台能力分类。
            Errc::NotImplemented,
            // 精确指出缺失协议 owner。
            "WaylandWindowOps::os_enable_file_drop: wl_data_device is unavailable",
        ));
    }
    // 健康 owner 才允许提交逐窗事实。
    let mut state = state.lock().map_err(|_| {
        // 构造稳定的共享状态错误。
        Error::new(
            // poisoned Component 属于 InvalidState。
            Errc::InvalidState,
            // 诊断区分同步能力更新阶段。
            "Wayland file drop state mutex poisoned during window capability update",
        )
    })?;
    // Component 内部处理幂等启用与禁用清理。
    state.set_window_enabled(window_id, enable);
    // 窗口能力事实已经提交。
    Ok(())
}

// 显式窗口关闭使用检查式禁用端口。
pub(crate) fn disable_window(
    // backend 唯一拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 待关闭稳定窗口身份。
    window_id: WindowId,
) -> Result<()> {
    // 状态损坏时同步暴露 typed failure。
    let mut state = state.lock().map_err(|_| {
        // 构造关闭阶段错误。
        Error::new(
            // poisoned Component 属于 InvalidState。
            Errc::InvalidState,
            // 诊断区分显式窗口关闭。
            "Wayland file drop state mutex poisoned during window close",
        )
    })?;
    // 禁用同步取消属于该窗口的活动与已 Drop transfer。
    state.set_window_enabled(window_id, false);
    // 关闭阶段拖放状态已清理。
    Ok(())
}

// Rust Drop 无同步返回通道，使用独立 teardown Adapter 完成最终释放。
pub(crate) fn force_disable_window(
    // backend 唯一拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 正在销毁的稳定窗口身份。
    window_id: WindowId,
) {
    // teardown 恢复 guard 只用于不可再观察的确定性 owner 释放。
    state
        // 获取最终可变访问。
        .lock()
        // 中毒不应阻止关闭 pipe 与协议 offer。
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        // 禁用窗口并取消属于它的全部 transfer。
        .set_window_enabled(window_id, false);
}
