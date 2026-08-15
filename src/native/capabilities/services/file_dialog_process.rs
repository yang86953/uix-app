// 引入 platform System 的 typed error 契约。
use crate::core::{Errc, Error, Result};

// 限制进入错误诊断的外部进程标准错误字符数。
const MAX_STDERR_CHARS: usize = 512;

// 外部文件对话框进程可以建立的非失败结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalDialogOutcome {
    // 对话框已确认，调用方应解析标准输出。
    Confirmed,
    // 用户取消对话框，公开契约应返回成功空值。
    Cancelled,
}

// 把外部文件对话框退出状态分类为确认、取消或 typed failure。
pub(crate) fn classify_external_dialog_exit(
    // Provider 名称只用于可操作诊断。
    provider: &str,
    // 缺失退出码表示进程被信号或平台机制终止。
    exit_code: Option<i32>,
    // 标准错误只进入有界诊断，不参与成功语义。
    stderr: &[u8],
) -> Result<ExternalDialogOutcome> {
    // 只把两个明确退出码解释为非失败结果。
    match exit_code {
        // 零表示用户确认并产生可解析结果。
        Some(0) => Ok(ExternalDialogOutcome::Confirmed),
        // Zenity 与 KDialog 的普通取消路径使用退出码一。
        Some(1) => Ok(ExternalDialogOutcome::Cancelled),
        // 其他退出码或无退出码必须保持可观察失败。
        unexpected => Err(external_dialog_failure(provider, unexpected, stderr)),
    }
}

// 构造包含有界 Provider 诊断的稳定平台错误。
fn external_dialog_failure(provider: &str, exit_code: Option<i32>, stderr: &[u8]) -> Error {
    // 将非 UTF-8 标准错误安全替换为可显示文本。
    let stderr = String::from_utf8_lossy(stderr);
    // 去除子进程输出外围空白。
    let stderr = stderr.trim();
    // 只复制诊断上限内的字符。
    let mut stderr_chars = stderr.chars();
    // 保存有限长度的标准错误前缀。
    let mut stderr_summary = stderr_chars
        .by_ref()
        .take(MAX_STDERR_CHARS)
        .collect::<String>();
    // 超过上限时明确标记诊断已截断。
    if stderr_chars.next().is_some() {
        // 使用单个省略号而不是保留无界输出。
        stderr_summary.push('…');
    }
    // 将退出状态转换为稳定人类可读文本。
    let status = match exit_code {
        // 保留原始非成功退出码。
        Some(code) => format!("exit code {code}"),
        // 无退出码通常表示信号或外部终止。
        None => "no exit code".to_owned(),
    };
    // 仅在 Provider 给出标准错误时附加诊断。
    let stderr_detail = if stderr_summary.is_empty() {
        // 空标准错误不添加多余分隔符。
        String::new()
    } else {
        // 标准错误前缀帮助定位桌面环境故障。
        format!("; stderr: {stderr_summary}")
    };
    // 返回公开门面可传播的 typed 平台错误。
    Error::new(
        // 已启动 Provider 的异常退出属于平台调用失败。
        Errc::PlatformError,
        // 同时保留 Provider、退出状态与有界诊断。
        format!("LinuxFileDialog: {provider} failed with {status}{stderr_detail}"),
    )
}

// 纯逻辑测试不启动真实桌面进程。
#[cfg(test)]
mod tests {
    // 引入退出分类器、结果枚举与错误码。
    use super::{Errc, ExternalDialogOutcome, classify_external_dialog_exit};

    // 确认与取消必须保持公开文件对话框契约。
    #[test]
    fn classifies_confirmed_and_cancelled_dialogs() {
        // 零退出码必须要求调用方解析路径结果。
        assert_eq!(
            classify_external_dialog_exit("zenity", Some(0), b"")
                // 测试夹具提供受支持退出码。
                .expect("zero exit code should be confirmed"),
            ExternalDialogOutcome::Confirmed,
        );
        // 一退出码必须保持成功取消而不是平台错误。
        assert_eq!(
            classify_external_dialog_exit("kdialog", Some(1), b"")
                // 测试夹具提供受支持取消码。
                .expect("one exit code should be cancelled"),
            ExternalDialogOutcome::Cancelled,
        );
    }

    // 意外退出必须返回有界且可诊断的 typed failure。
    #[test]
    fn rejects_unexpected_dialog_exit_with_bounded_stderr() {
        // 构造明显超过诊断上限的外部标准错误。
        let stderr = vec![b'x'; 1_024];
        // 非取消退出码必须形成失败。
        let error = classify_external_dialog_exit("zenity", Some(5), &stderr)
            // 成功会掩盖 Zenity timeout 等真实故障。
            .expect_err("unexpected exit code must fail");
        // 错误分类必须保持平台故障。
        assert_eq!(error.code(), Errc::PlatformError);
        // 诊断必须保留 Provider 与原始退出码。
        assert!(error.message().contains("zenity failed with exit code 5"));
        // 截断标记证明无界标准错误没有完整进入错误对象。
        assert!(error.message().ends_with('…'));
        // 无退出码同样不能伪装成用户取消。
        assert!(classify_external_dialog_exit("kdialog", None, b"").is_err());
    }
}
