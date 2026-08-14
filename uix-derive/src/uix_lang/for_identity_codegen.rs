// 引入过程宏标识符与卫生跨度。
use proc_macro2::{Ident, Span};

// 引入控制元素、属性值与诊断。
use super::{AttributeValue, Diagnostic, Element};

// 读取组件展开阶段写入的必需 For 内部标识符。
pub(super) fn internal_control_ident(element: &Element, name: &str) -> Result<Ident, Diagnostic> {
    // 委托可选读取入口并要求值存在。
    optional_internal_control_ident(element, name)?.ok_or_else(|| {
        // 返回内部阶段边界诊断。
        Diagnostic::new(
            element.span,
            format!("<For> 缺少内部实例路径 {name}"),
            "通过完整文档组件展开入口生成 UIX View",
        )
    })
}

// 读取组件展开阶段写入的可选 For 内部标识符。
pub(super) fn optional_internal_control_ident(
    // 接收已经展开的 For 元素。
    element: &Element,
    // 接收内部属性名称。
    name: &str,
) -> Result<Option<Ident>, Diagnostic> {
    // 查找指定内部属性。
    let Some(attribute) = element
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
    else {
        // 缺席表示当前可选身份层不存在。
        return Ok(None);
    };
    // 内部属性必须保存卫生名称字面量。
    let AttributeValue::Literal(value) = &attribute.value else {
        // 返回内部形状诊断。
        return Err(Diagnostic::new(
            attribute.span,
            format!("内部实例路径 {name} 形状无效"),
            "重新运行 UIX 文档组件展开阶段",
        ));
    };
    // 使用调用点卫生解析到循环体声明。
    Ok(Some(Ident::new(value, Span::call_site())))
}
