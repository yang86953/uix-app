// 引入过程宏令牌流。
use proc_macro2::TokenStream;

// 引入父级语言模块拥有的诊断与源码跨度。
use super::super::{Diagnostic, SourceSpan};

// 恢复由组件展开器保存的 For 逐迭代准备语句。
pub(super) fn parse_for_iteration_setup(
    // 接收按依赖顺序保存的内部令牌文本。
    statements: &[String],
    // 接收所属 For 的源码跨度供内部诊断定位。
    span: SourceSpan,
) -> Result<Vec<TokenStream>, Diagnostic> {
    // 保持组件依赖顺序遍历全部语句。
    statements
        // 借用内部令牌文本迭代器。
        .iter()
        // 只解析代码生成器自己保存的内部令牌，不接收用户文本。
        .map(|statement| {
            // 恢复可由 quote 插入循环体的令牌流。
            statement.parse::<TokenStream>().map_err(|_| {
                // 内部序列化失败必须在当前 For 位置明确暴露。
                Diagnostic::new(
                    // 指向所属 For 控制元素。
                    span,
                    // 说明内部逐迭代准备语句损坏。
                    "For 组件准备语句无法恢复",
                    // 该错误不能由 UIX 作者修改源码规避。
                    "报告 UIX 编译器内部错误并附上当前 Widget 与 For 声明",
                )
            })
        })
        // 在生成循环前传播内部恢复失败并收集全部令牌流。
        .collect()
}

// 恢复由 reactive 组件展开器收归到子树作用域的准备语句。
pub(super) fn parse_reactive_setup(
    // 接收按依赖顺序保存的内部令牌文本。
    statements: &[String],
    // 接收所属组件调用的源码跨度供内部诊断定位。
    span: SourceSpan,
) -> Result<Vec<TokenStream>, Diagnostic> {
    // 保持组件依赖顺序遍历全部语句。
    statements
        // 借用内部令牌文本迭代器。
        .iter()
        // 只解析代码生成器自己保存的内部令牌，不接收用户文本。
        .map(|statement| {
            // 恢复可由 quote 插入 scoped 闭包的令牌流。
            statement.parse::<TokenStream>().map_err(|_| {
                // 内部序列化失败必须在当前调用位置明确暴露。
                Diagnostic::new(
                    // 指向所属组件调用。
                    span,
                    // 说明内部响应式准备语句损坏。
                    "reactive 组件准备语句无法恢复",
                    // 该错误不能由 UIX 作者修改源码规避。
                    "报告 UIX 编译器内部错误并附上当前 reactive Widget 声明",
                )
            })
        })
        // 在生成闭包前传播内部恢复失败并收集全部令牌流。
        .collect()
}
