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
    // 保存本窗口循环最后一次成功应用的光标。
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
    // 保存最后一次成功应用的光标。
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
    // 引入光标端口统一结果类型。
    use crate::native::Result;

    // 保存测试期间观察到的窄光标端口调用。
    #[derive(Default)]
    struct RecordingCursor {
        // 按调用顺序记录请求光标。
        calls: Vec<CursorType>,
    }

    // 为定向测试实现完整窄光标端口。
    impl ICursor for RecordingCursor {
        // 记录平台光标请求。
        fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
            // 追加而不覆盖，便于验证调用次数与顺序。
            self.calls.push(cursor);
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
}
