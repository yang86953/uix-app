// capability absence 使用稳定错误码与可重试运行失败区分。
use crate::core::Errc;
// 引入平台能力根契约。
use crate::native::platform::Platform;
// 引入 App System 实际调用的窄光标能力端口。
use crate::native::windowing::input::ICursor;
// 引入平台无关的指针光标枚举。
use crate::platform::windowing::CursorType;
// 引入单线程事件循环保存的当前光标状态。
use std::cell::Cell;

// 仅在请求光标变化时调用平台能力。
pub(super) fn apply_pointer_cursor(
    // 接收 App System 注入的平台能力根。
    platform: &mut dyn Platform,
    // 接收 UI System 解析出的有效光标。
    requested: CursorType,
    // 保存本窗口循环最后一次成功或已确认不支持的光标请求。
    active: &Cell<Option<CursorType>>,
) {
    // 从平台能力根借用唯一光标端口并委托去重逻辑。
    apply_cursor_port(platform.cursor(), requested, active);
}

// 把去重后的请求应用到窄光标能力端口。
fn apply_cursor_port(
    // 接收可记录或真实实现的光标端口。
    cursor: &mut dyn ICursor,
    // 接收 UI System 解析出的有效光标。
    requested: CursorType,
    // 保存最后一次成功或已确认不支持的光标请求。
    active: &Cell<Option<CursorType>>,
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
        // 失败时保留旧状态，让后续循环仍可重试。
        Err(error) => tracing::warn!(
            // 记录结构化失败原因但不终止窗口循环。
            error = %error.short_what(),
            // 标识发生错误的 App System 能力交接。
            "pointer cursor update failed"
        ),
    }
}

// 验证平台调用只在首次请求与光标变化时发生。
#[cfg(test)]
mod tests {
    // 引入被测去重交接函数。
    use super::*;
    // 引入光标位置返回值类型。
    use crate::core::Point;
    // 构造稳定 capability absence 与普通重试失败。
    use crate::core::{Errc, Error};
    // 引入光标端口统一结果类型。
    use crate::native::Result;

    // 保存测试期间观察到的窄光标端口调用。
    #[derive(Default)]
    struct RecordingCursor {
        // 按调用顺序记录请求光标。
        calls: Vec<CursorType>,
        // 可选失败分类用于验证 App 交接策略。
        failure: Option<Errc>,
    }

    // 为定向测试实现完整窄光标端口。
    impl ICursor for RecordingCursor {
        // 记录平台光标请求。
        fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
            // 追加而不覆盖，便于验证调用次数与顺序。
            self.calls.push(cursor);
            // 配置失败时返回稳定 typed error。
            if let Some(code) = self.failure {
                // 诊断只服务测试，不携带平台状态。
                return Err(Error::new(code, "recording cursor failure"));
            }
            // 内存端口始终成功。
            Ok(())
        }

        // 测试不需要改变光标可见性。
        fn show_cursor(&mut self, _visible: bool) -> Result<()> {
            // 保持无副作用成功。
            Ok(())
        }

        // 测试不需要真实光标位置。
        fn cursor_position(&self) -> Result<Point> {
            // 返回稳定零点。
            Ok(Point::zero())
        }

        // 测试不需要移动真实光标。
        fn set_cursor_position(&mut self, _x: i32, _y: i32) -> Result<()> {
            // 保持无副作用成功。
            Ok(())
        }

        // 测试不需要限制真实光标范围。
        fn confine_cursor(&mut self, _confine: bool) -> Result<()> {
            // 保持无副作用成功。
            Ok(())
        }

        // 测试不需要捕获真实鼠标。
        fn capture_mouse(&mut self) -> Result<()> {
            // 保持无副作用成功。
            Ok(())
        }

        // 测试不需要释放真实鼠标。
        fn release_mouse(&mut self) -> Result<()> {
            // 保持无副作用成功。
            Ok(())
        }
    }

    // 相同光标请求不得重复调用平台。
    #[test]
    fn applies_platform_cursor_only_when_requested_value_changes() {
        // 构造记录全部光标调用的内存端口。
        let mut cursor = RecordingCursor::default();
        // 未知初始状态要求第一次请求必须同步到平台。
        let active = Cell::new(None);
        // 首次默认箭头仍需明确应用，避免多窗口继承旧平台状态。
        apply_cursor_port(&mut cursor, CursorType::Arrow, &active);
        // 相同默认箭头请求必须去重。
        apply_cursor_port(&mut cursor, CursorType::Arrow, &active);
        // 光标变化到手形时必须调用一次平台。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 相同手形请求必须继续去重。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 平台历史只应包含首次同步和一次变化。
        assert_eq!(
            // 读取内存平台记录的调用序列。
            cursor.calls,
            // 保持严格的调用顺序。
            vec![CursorType::Arrow, CursorType::Hand]
        );
        // 去重状态必须反映最后一次成功的平台调用。
        assert_eq!(active.get(), Some(CursorType::Hand));
    }

    // NotImplemented 表示稳定能力缺失，相同请求不得形成重试风暴。
    #[test]
    fn deduplicates_stable_cursor_capability_absence() {
        // 构造始终返回 NotImplemented 的记录端口。
        let mut cursor = RecordingCursor {
            // 初始尚未发生平台调用。
            calls: Vec::new(),
            // 配置稳定 capability absence。
            failure: Some(Errc::NotImplemented),
        };
        // 未知初始状态要求首个请求进入平台一次。
        let active = Cell::new(None);
        // 首次手形请求取得 NotImplemented。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 相同手形请求必须由已确认能力状态去重。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 平台端口只观察到一次请求。
        assert_eq!(cursor.calls, vec![CursorType::Hand]);
        // 去重状态保存已确认不支持的请求值。
        assert_eq!(active.get(), Some(CursorType::Hand));
    }

    // 普通运行失败仍需保留旧状态并允许后续重试。
    #[test]
    fn retries_non_capability_cursor_failure() {
        // 构造始终返回运行状态错误的记录端口。
        let mut cursor = RecordingCursor {
            // 初始尚未发生平台调用。
            calls: Vec::new(),
            // InvalidState 不属于稳定 capability absence。
            failure: Some(Errc::InvalidState),
        };
        // 未知初始状态不能被失败请求覆盖。
        let active = Cell::new(None);
        // 首次请求失败。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 相同请求必须继续重试。
        apply_cursor_port(&mut cursor, CursorType::Hand, &active);
        // 平台端口观察到两次可重试调用。
        assert_eq!(cursor.calls, vec![CursorType::Hand, CursorType::Hand]);
        // active 必须保持未成功状态。
        assert_eq!(active.get(), None);
    }
}
