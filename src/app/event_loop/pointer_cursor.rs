// capability absence 使用稳定错误码与可重试运行失败区分。
use crate::core::Errc;
// 引入平台能力根契约。
use crate::platform::platform::PlatformSystem;
// 引入 App System 实际调用的窄光标能力端口。
use crate::platform::windowing::ICursor;
// 引入平台无关的指针光标枚举。
use crate::platform::windowing::CursorType;
// 引入单线程事件循环保存的当前光标状态。
use std::cell::Cell;

// 仅在请求光标变化时调用平台能力。
pub(super) fn apply_pointer_cursor(
    // 接收 App System 注入的平台能力根。
    platform: &mut dyn PlatformSystem,
    // 接收 UI System 解析出的有效光标。
    requested: CursorType,
    // 保存本窗口循环最后一次成功或已确认不支持的光标请求。
    active: &Cell<Option<CursorType>>,
    // 接收统一诊断 System：瞬态更新失败经冷却去重观察。
    diagnostics: &crate::diagnostics::Diagnostics,
) {
    // 从平台能力根借用唯一光标端口并委托去重逻辑。
    apply_cursor_port(platform.cursor(), requested, active, diagnostics);
}

// 把去重后的请求应用到窄光标能力端口。
fn apply_cursor_port(
    // 接收可记录或真实实现的光标端口。
    cursor: &mut dyn ICursor,
    // 接收 UI System 解析出的有效光标。
    requested: CursorType,
    // 保存最后一次成功或已确认不支持的光标请求。
    active: &Cell<Option<CursorType>>,
    // 接收统一诊断 System：瞬态更新失败经冷却去重观察。
    diagnostics: &crate::diagnostics::Diagnostics,
) {
    // 相同光标不重复进入原生平台调用。
    if active.get() == Some(requested) {
        // 去重路径不产生任何平台副作用。
        return;
    }
    // 通过平台能力端口应用新光标。
    match cursor.set_cursor(requested) {
        // 成功后才更新去重状态。
        Ok(()) => active.set(Some(requested)),
        // 稳定能力缺失不应在每次 pointer 事件重复调用或产生 WARN 风暴。
        Err(error) if error.code() == Errc::NotImplemented => {
            // 缓存已确认不支持的请求值，相同请求保持去重。
            active.set(Some(requested));
            // debug 保留 capability 诊断但不冒充运行故障。
            tracing::debug!(
                // 保留 typed error 的稳定简述。
                error = %error.short_what(),
                // 标识 App System 的预期能力缺失交接。
                "pointer cursor capability is unavailable"
            );
        }
        // 失败时保留旧状态，让后续循环仍可重试；指针移动频率高，经
        // observe_transient 冷却去重避免逐事件刷屏。
        Err(error) => {
            diagnostics.observe_transient(
                "pointer_cursor",
                "pointer cursor update failed; previous cursor retained",
                error.short_what(),
            );
        }
    }
}
