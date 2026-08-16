//! Application 窗口创建 Adapter 统一启用产品所需的原生能力。

// typed error 区分稳定能力缺失与真实创建失败。
use crate::core::{Errc, Result};
// 原生窗口管理器只提供机制，Application 在本 Adapter 注入 FileDrop 策略。
use crate::native::windowing::window::{IWindowManager, PlatformWindow};

// 创建一个默认启用 FileDrop 的应用窗口。
pub(crate) fn create_app_window(
    // 调用方提供当前平台的原生窗口工厂。
    window_manager: &mut dyn IWindowManager,
    // 标题原样交给平台。
    title: &str,
    // 宽度保持 logical 客户区语义。
    width: i32,
    // 高度保持 logical 客户区语义。
    height: i32,
    // 返回已配置应用能力的唯一窗口 owner。
) -> Result<Box<dyn PlatformWindow>> {
    // 先创建真实平台窗口，尚不向调用方发布 owner。
    let mut window = window_manager.create_window(title, width, height)?;
    // Application 默认请求 Upload drag 所需的 FileDrop 能力。
    match classify_file_drop_enable(window.properties_mut().enable_file_drop(true)) {
        // 能力已启用或平台稳定不支持时均可继续创建。
        Ok(_) => Ok(window),
        // 真实启用失败必须在线性化创建事务内关闭窗口。
        Err(primary_error) => {
            // 清理成功保留原始启用失败，清理失败保留完整原因链。
            Err(match window.close() {
                // 原生窗口已释放，传播根因。
                Ok(()) => primary_error,
                // 清理错误成为外层失败，并保留启用错误作为 source。
                Err(cleanup_error) => cleanup_error.with_source(primary_error),
            })
        }
    }
}

// 把 FileDrop 能力结果分类为启用、稳定缺失或真实失败。
fn classify_file_drop_enable(result: Result<()>) -> Result<bool> {
    // 分类保持 typed error，不依赖日志文本。
    match result {
        // 平台确认启用后返回 true。
        Ok(()) => Ok(true),
        // macOS 或缺少 data-device 的 Wayland 可继续创建无该能力窗口。
        Err(error) if error.code() == Errc::NotImplemented => Ok(false),
        // 状态、I/O 与平台执行失败必须阻止发布半配置窗口。
        Err(error) => Err(error),
    }
}

// 纯分类测试不创建真实原生窗口。
#[cfg(test)]
mod tests {
    // 引入当前私有分类 Component。
    use super::classify_file_drop_enable;
    // 构造 typed 结果所需的错误码与错误值。
    use crate::core::{Errc, Error};

    // 成功结果必须报告真实启用。
    #[test]
    fn reports_enabled_capability() {
        // Ok 只能映射为 true。
        assert_eq!(classify_file_drop_enable(Ok(())).unwrap(), true);
    }

    // 稳定 NotImplemented 不得阻止应用窗口创建。
    #[test]
    fn tolerates_only_not_implemented() {
        // 构造平台稳定能力缺失。
        let unsupported = Error::new(Errc::NotImplemented, "file drop unavailable");
        // 能力缺失映射为 false 而非错误。
        assert_eq!(
            // 调用纯分类 Component。
            classify_file_drop_enable(Err(unsupported)).unwrap(),
            // false 表示窗口可用但 FileDrop 未启用。
            false
        );
    }

    // 真实执行失败必须保留原始错误码。
    #[test]
    fn propagates_real_enable_failure() {
        // 构造不可吞掉的状态错误。
        let failure = Error::new(Errc::InvalidState, "file drop state poisoned");
        // 分类器必须返回错误。
        let returned = classify_file_drop_enable(Err(failure)).unwrap_err();
        // typed 错误码不得被重写。
        assert_eq!(returned.code(), Errc::InvalidState);
    }
}
