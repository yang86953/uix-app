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
