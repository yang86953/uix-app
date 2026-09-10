// 引入卫生事件变量所需的标识符与跨度。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Input 属性、表达式、值映射与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_event_handler_expression,
    generate_expression, literal_string, string_value,
};

// 生成保持 State<String> 双向绑定的文本输入节点。
pub(crate) fn generate_input(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Input 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Input 元素。
            element.span,
            // 说明文本输入不接受子节点。
            "<Input> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Input value={name} />",
        ));
    }

    // 按文档缺省值选择单行文本构造器。
    let mut widget = quote! { ::uix_app::prelude::Input::new("") };
    // 输入类型必须在编译期确定，避免运行时近似未知模式。
    if let Some(attribute) = find_attribute(element, "type") {
        // 读取经过验证的类型字面量。
        let kind = literal_string(attribute, "Input type")?;
        // 映射到公开 Input 构造器。
        widget = match kind.as_str() {
            // text 沿用普通单行输入。
            "text" => quote! { ::uix_app::prelude::Input::new("") },
            // textarea 使用公开多行构造器。
            "textarea" => quote! { ::uix_app::prelude::Input::textarea() },
            // password 使用公开密码构造器。
            "password" => quote! { ::uix_app::prelude::Input::password() },
            // 未登记模式必须在编译期拒绝。
            _ => {
                // 返回类型值诊断。
                return Err(Diagnostic::new(
                    // 指向非法 type 属性。
                    attribute.span,
                    // 说明未知输入类型。
                    format!("Input type={kind:?} 尚无公开构造器映射"),
                    // 给出文档登记集合。
                    "使用 text、textarea 或 password",
                ));
            }
        };
    }

    // 可选占位文本支持字符串字面量与受限表达式。
    if let Some(attribute) = find_attribute(element, "placeholder") {
        // 生成公开 String 输入值。
        let placeholder = string_value(attribute)?;
        // 应用公开 placeholder 构建器。
        widget = quote! { (#widget).placeholder(#placeholder) };
    }
    // 可选 value 必须保留 State<String> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Input value 必须绑定 State<String> 表达式",
                // 给出规范绑定写法。
                "使用 value={name}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 Input 双向绑定入口。
        widget = quote! { (#widget).value(&(#state)) };
    }
    // 可选禁用状态支持布尔简写、字面量与表达式。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成布尔状态表达式。
        let disabled = boolean_value(attribute)?;
        // 应用公开禁用构建器。
        widget = quote! { (#widget).disabled(#disabled) };
    }
    // 可选最小可见行数支持 usize 字面量或受限表达式；公开运行时会启用多行模式。
    if let Some(attribute) = find_attribute(element, "rows") {
        // 生成 usize 行数。
        let rows = usize_value(attribute)?;
        // 应用公开行数构建器。
        widget = quote! { (#widget).rows(#rows) };
    }
    // 可选输入长度上限支持 usize 字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "maxLength") {
        // 生成 usize 上限。
        let max_length = usize_value(attribute)?;
        // 应用公开长度上限构建器。
        widget = quote! { (#widget).max_length(#max_length) };
    }

    // 先物化公开叶节点，Change 处理器与样式都由 View 契约拥有。
    let mut view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 可选值变化事件读取统一文本载荷。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Input @change 必须是受限处理器表达式",
                // 给出带载荷的规范写法。
                "使用 @change=\"on_change($event)\"",
            ));
        };
        // 创建卫生的文本载荷变量。
        let value = Ident::new("__uix_change_value", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            // 注册只接收当前文本借用的变更闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Input 专有属性后应用统一尺寸、样式、自动化属性与其他公共事件。
    apply_common_attributes(
        // 传入已经配置输入事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Input 专有属性和 Change 事件被二次映射。
        &["value", "placeholder", "type", "disabled", "rows", "maxLength", "@change"],
    )
}

// 生成 usize 字面量或受限表达式。
fn usize_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成无符号整数。
    match &attribute.value {
        // 静态值必须是 usize 整数。
        AttributeValue::Literal(source) => {
            // 解析无符号平台整数。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回整数类型诊断。
                Diagnostic::new(
                    // 指向非法属性。
                    attribute.span,
                    // 说明公开运行时类型。
                    "Input 的行数与长度上限必须是 usize 整数",
                    // 给出合法示例。
                    "使用 rows=\"2\"、maxLength=\"8192\" 或 usize 表达式",
                )
            })?;
            // 生成类型明确的 usize 字面量。
            Ok(quote! { #value })
        }
        // 动态值保持 Rust usize 类型检查。
        AttributeValue::Expression(expression) => {
            // 生成受限整数表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不可能用于整数。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常属性。
            attribute.span,
            // 说明内部属性形状不匹配。
            "Input 的行数与长度上限不能使用内联样式值",
            // 给出有效整数写法。
            "使用 usize 整数字面量或受限表达式",
        )),
    }
}

// 查找元素上的具名属性。
