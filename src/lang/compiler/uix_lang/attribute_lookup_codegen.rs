// 引入结构化诊断与共享元素、属性 AST。
use super::{Attribute, Diagnostic, Element};

// 返回元素上第一个名称匹配的属性；全部生成器共享同一查找实现。
pub(crate) fn find_attribute<'a>(
    // 接收完整元素。
    element: &'a Element,
    // 接收要查找的属性名。
    name: &str,
) -> Option<&'a Attribute> {
    // 遍历源码顺序中的属性。
    element
        // 只读属性切片。
        .attributes
        // 按名称匹配。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
}

// 要求元素携带具名属性，缺失时生成组件级诊断；全部生成器共享同一必需属性实现。
pub(crate) fn required_attribute<'a>(
    // 接收完整元素。
    element: &'a Element,
    // 接收必需属性名。
    name: &str,
    // 接收诊断点名的组件标签，不含尖括号。
    owner: &str,
    // 接收缺失时的修复建议文案。
    repair_hint: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享查找并在缺失时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整组件元素。
            element.span,
            // 点名缺失属性所属组件。
            format!("<{owner}> 缺少必需的 {name} 属性"),
            // 透传各组件登记的修复建议。
            repair_hint,
        )
    })
}
