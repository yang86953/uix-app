// 引入共享源码跨度。
use super::SourceSpan;

// 表示可直接转换为编译错误的结构化语言诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    // 保存失败位置。
    pub(crate) span: SourceSpan,
    // 保存明确失败原因。
    pub(crate) message: String,
    // 保存可执行的修复建议。
    pub(crate) suggestion: String,
}

// 提供稳定的诊断构造入口。
impl Diagnostic {
    // 构造包含位置、原因和修复建议的诊断。
    pub(crate) fn new(
        // 接收失败跨度。
        span: SourceSpan,
        // 接收失败原因。
        message: impl Into<String>,
        // 接收修复建议。
        suggestion: impl Into<String>,
    ) -> Self {
        // 返回完全初始化的诊断值。
        Self {
            // 保存失败跨度。
            span,
            // 规范化失败原因所有权。
            message: message.into(),
            // 规范化修复建议所有权。
            suggestion: suggestion.into(),
        }
    }
}
