// 引入共享源码跨度。
use super::SourceSpan;
// 引入 Compiler System 的稳定源码身份。
use crate::lang::compiler::source_graph::SourceId;

// 表示可直接转换为编译错误的结构化语言诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    // 保存生成阶段已经确定的真实源码身份；解析阶段保持为空并由上层绑定。
    pub(crate) source_id: Option<SourceId>,
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
            // 纯语言解析尚未进入具体源码图。
            source_id: None,
            // 保存失败跨度。
            span,
            // 规范化失败原因所有权。
            message: message.into(),
            // 规范化修复建议所有权。
            suggestion: suggestion.into(),
        }
    }

    // 在生成上下文已知时补充真实来源，内层更精确来源优先。
    pub(crate) fn at_source_if_missing(mut self, source_id: Option<SourceId>) -> Self {
        // 只填补尚未归属的诊断，避免外层组件覆盖内层文件。
        if self.source_id.is_none() {
            self.source_id = source_id;
        }
        // 返回保留全部原始诊断字段的值。
        self
    }
}
