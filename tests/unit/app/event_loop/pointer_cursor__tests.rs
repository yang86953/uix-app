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
