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

// 解析外部文件对话框已经确认的单路径输出。
pub(crate) fn parse_confirmed_dialog_path(provider: &str, stdout: &[u8]) -> Result<String> {
    // Provider 协议允许在路径后附加 CR/LF 行终止符。
    let path = trim_terminal_line_endings(stdout);
    // 确认成功必须同时产生一条非空路径。
    if path.is_empty() {
        // 空确认不能伪装成有效 owned 路径。
        return Err(invalid_confirmed_output(
            provider,
            "confirmed without a selected path",
        ));
    }
    // 内部 IFileDialog 契约只能承载有效 UTF-8 字符串。
    let path = std::str::from_utf8(path).map_err(|_| {
        // 无法无损表示的 Provider 输出必须保持可观察失败。
        invalid_confirmed_output(provider, "confirmed with a non-UTF-8 path")
    })?;
    // 保留合法路径首尾空格，只复制协议载荷本身。
    Ok(path.to_owned())
}

// 解析外部文件对话框已经确认的逐行多路径输出。
pub(crate) fn parse_confirmed_dialog_paths(provider: &str, stdout: &[u8]) -> Result<Vec<String>> {
    // 先移除整个 Provider 输出末尾的协议行终止符。
    let paths = trim_terminal_line_endings(stdout);
    // 多选确认至少必须包含一条路径。
    if paths.is_empty() {
        // 空列表与用户取消具有不同语义。
        return Err(invalid_confirmed_output(
            provider,
            "confirmed without selected paths",
        ));
    }
    // 在拆分前拒绝会被 lossy 转换改写的路径字节。
    let paths = std::str::from_utf8(paths).map_err(|_| {
        // 多选中任一不可表示字节都会使完整结果失败。
        invalid_confirmed_output(provider, "confirmed with non-UTF-8 paths")
    })?;
    // 为每条换行分隔的路径分配独立 owned 字符串。
    paths
        // Zenity 与 KDialog 的多选协议都使用 LF 分隔。
        .split('\n')
        // 保留路径空格并只移除 CRLF 中属于协议的 CR。
        .map(|path| path.strip_suffix('\r').unwrap_or(path))
        // 空选择项表示 Provider 输出损坏而不是合法路径。
        .map(|path| {
            // 每个元素都必须单独满足非空不变量。
            if path.is_empty() {
                // 不允许静默过滤损坏的列表项。
                Err(invalid_confirmed_output(
                    provider,
                    "confirmed with an empty selected path",
                ))
            } else {
                // 路径保持 Provider 顺序并保留首尾空格。
                Ok(path.to_owned())
            }
        })
        // 任一损坏元素使整个原子选择结果失败。
        .collect()
}

// 只移除外部对话框协议附加在整个输出末尾的 CR/LF。
fn trim_terminal_line_endings(mut output: &[u8]) -> &[u8] {
    // 连续移除末尾行终止字节，直到遇到路径内容。
    while matches!(output.last(), Some(b'\r' | b'\n')) {
        // 切片缩短一字节，不复制外部进程输出。
        output = &output[..output.len() - 1];
    }
    // 返回仍然保留普通空格与其他路径字节的视图。
    output
}

// 构造不含敏感路径载荷的确认输出错误。
fn invalid_confirmed_output(provider: &str, reason: &str) -> Error {
    // Provider 协议损坏属于平台调用失败。
    Error::new(
        // 与异常退出使用相同的稳定 typed code。
        Errc::PlatformError,
        // 只记录 Provider 与失败原因，不泄漏用户路径。
        format!("LinuxFileDialog: {provider} {reason}"),
    )
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
    // 引入退出分类器、结果解析器、结果枚举与错误码。
    use super::{
        Errc, ExternalDialogOutcome, classify_external_dialog_exit, parse_confirmed_dialog_path,
        parse_confirmed_dialog_paths,
    };

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

    // 单路径解析只移除协议终止符并保留合法空格。
    #[test]
    fn parses_confirmed_single_path_without_trimming_spaces() {
        // 路径两端空格都属于文件名内容。
        let path = parse_confirmed_dialog_path("zenity", b" /tmp/report .txt \r\n")
            // 测试输入包含一条有效 UTF-8 路径。
            .expect("confirmed path should parse");
        // 解析不能复用宽泛 trim 改写路径。
        assert_eq!(path, " /tmp/report .txt ");
    }

    // 多路径解析保持 Provider 顺序与每条路径内容。
    #[test]
    fn parses_confirmed_path_lines_in_order() {
        // 混合 CRLF 与 LF 模拟两个 Provider 的逐行输出。
        let paths = parse_confirmed_dialog_paths("kdialog", b"/tmp/one file\r\n /tmp/two \n")
            // 两条路径都非空且为有效 UTF-8。
            .expect("confirmed paths should parse");
        // 返回顺序和第二条路径两端空格都必须保留。
        assert_eq!(paths, vec!["/tmp/one file", " /tmp/two "]);
    }

    // 确认状态不得把损坏输出转换为成功路径。
    #[test]
    fn rejects_invalid_confirmed_path_output() {
        // 单路径空输出不是用户取消。
        let empty = parse_confirmed_dialog_path("zenity", b"\r\n")
            // 空确认必须形成 typed failure。
            .expect_err("empty confirmed path must fail");
        // 输出协议故障保持平台错误分类。
        assert_eq!(empty.code(), Errc::PlatformError);
        // 多路径中间的空项不能被静默过滤。
        assert!(parse_confirmed_dialog_paths("kdialog", b"/tmp/a\n\n/tmp/b\n").is_err());
        // 无效 UTF-8 不能被替换字符悄悄改写。
        assert!(parse_confirmed_dialog_path("zenity", &[0xff, b'\n']).is_err());
    }
}
