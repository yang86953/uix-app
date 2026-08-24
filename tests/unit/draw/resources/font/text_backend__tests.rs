// 引入被测模块的私有估算入口。
use super::estimate_text_metrics;

// 验证无限宽度路径保留字符宽度、CRLF 与尾随空白语义。
#[test]
fn unwrapped_metrics_stream_explicit_lines_without_changing_widths() {
    // W+i 为 10，两个中文字符为 20，末行 1+空格为 6.5。
    let metrics = estimate_text_metrics("Wi\r\n中文\n1 ", f32::INFINITY, 10.0);

    assert_eq!(metrics.max_line_width, 20.0);
    assert_eq!(metrics.line_count, 3);
    assert!(!metrics.width_wrapped);
}

// 验证不启用自动折行的边界宽度共享相同流式契约。
#[test]
fn non_positive_and_nan_widths_keep_unwrapped_contract() {
    let expected = estimate_text_metrics("A\nB", f32::INFINITY, 12.0);

    assert_eq!(estimate_text_metrics("A\nB", 0.0, 12.0), expected);
    assert_eq!(estimate_text_metrics("A\nB", -1.0, 12.0), expected);
    assert_eq!(estimate_text_metrics("A\nB", f32::NAN, 12.0), expected);
}
