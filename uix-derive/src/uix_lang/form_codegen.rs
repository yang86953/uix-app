// 引入卫生标识符与过程宏令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与按钮生成边界。
use super::codegen::{apply_common_attributes, generate_button_with_group_position};
// 引入 Form 语法树、表达式和值映射契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Expression, ExpressionKind, Node,
    generate_expression, generate_handler_expression, literal_string, rust_identifier,
};

// 生成类型化 Form 与首批 FormInputItem 字段。
pub(crate) fn generate_form(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Form 必须绑定业务模型 State。
    let model_attribute = required_attribute(element, "model")?;
    // 模型必须保留 State<M> 句柄所有权。
    let AttributeValue::Expression(model_expression) = &model_attribute.value else {
        // 返回模型绑定形状诊断。
        return Err(Diagnostic::new(
            model_attribute.span,
            "Form model 必须绑定 State<M> 表达式",
            "使用 model={user_form}",
        ));
    };
    // 生成受限模型状态表达式。
    let model = generate_expression(&model_expression.expression, None)?;
    // Form 首批必须登记类型化提交回调。
    let submit_attribute = required_attribute(element, "@submit")?;
    // 事件解析器应提供受限表达式。
    let AttributeValue::Expression(submit_expression) = &submit_attribute.value else {
        // 返回内部事件形状保护诊断。
        return Err(Diagnostic::new(
            submit_attribute.span,
            "Form @submit 必须是受限处理器表达式",
            "使用 @submit=\"on_submit($event)\"",
        ));
    };

    // 保存字段链和唯一提交按钮。
    let (mut fields, mut submit_button) = (Vec::new(), None);
    // 按源码顺序检查直接子节点。
    for child in &element.children {
        // 忽略排版空白。
        if matches!(child, Node::Text(text) if text.value.trim().is_empty()) {
            // 继续处理下一节点。
            continue;
        }
        // 首批只接受直接元素。
        let Node::Element(child) = child else {
            // 返回子树边界诊断。
            return Err(form_shape_diagnostic(element));
        };
        // 字段必须位于提交按钮之前。
        if child.name == "FormInputItem" {
            // 防止源码顺序被生成器重排。
            if submit_button.is_some() {
                // 返回字段顺序诊断。
                return Err(Diagnostic::new(
                    child.span,
                    "FormInputItem 必须位于 submitForm Button 之前",
                    "先声明全部字段，最后声明提交按钮",
                ));
            }
            // 生成字段链片段。
            fields.push(generate_input_field(child)?);
            // 继续处理下一节点。
            continue;
        }
        // Button 必须是唯一提交入口。
        if child.name == "Button" && submit_button.is_none() {
            // 生成移除内置点击后的按钮。
            submit_button = Some(generate_submit_button(child)?);
            // 继续处理下一节点。
            continue;
        }
        // 拒绝未登记子项或重复按钮。
        return Err(Diagnostic::new(
            child.span,
            format!("<Form> 直接子项 <{}> 尚未进入首批类型化映射", child.name),
            "当前使用 FormInputItem 与一个 submitForm Button",
        ));
    }
    // 至少一个字段才能形成类型化表单。
    if fields.is_empty() {
        // 返回空表单诊断。
        return Err(Diagnostic::new(
            element.span,
            "<Form> 至少需要一个直接 FormInputItem",
            "添加 <FormInputItem field=\"name\" label=\"姓名\" />",
        ));
    }
    // 取出经过验证的提交按钮。
    let submit_button = submit_button.ok_or_else(|| form_shape_diagnostic(element))?;
    // 创建卫生的提交模型、表单与提交句柄变量。
    let submitted = Ident::new("__uix_submitted_model", Span::mixed_site());
    // 创建卫生的表单变量。
    let form = Ident::new("__uix_model_form", Span::mixed_site());
    // 创建卫生的提交克隆变量。
    let submit_form = Ident::new("__uix_submit_form", Span::mixed_site());
    // 生成类型化回调表达式。
    let submit_handler = generate_submit_handler(&submit_expression.expression, &submitted)?;
    // 生成运行时拥有校验与提交生命周期的 View。
    let view = quote! {{
        // 构建类型化字段与回调。
        let #form = ::uix::prelude::Form::model(&(#model))
            #(#fields)*
            .on_submit_typed(move |#submitted| { #submit_handler })
            .build();
        // 克隆同一表单句柄供按钮触发。
        let #submit_form = #form.clone();
        // 组合字段区域和唯一提交按钮。
        ::uix::prelude::column(
            ::std::vec![
                #form.view(),
                ::uix::prelude::View::build(#submit_button).on_click_fn(move || {
                    let _ = #submit_form.submit_typed();
                }),
            ]
        )
    }};
    // 应用 Form 外层公共属性。
    apply_common_attributes(view, &element.attributes, &["model", "@submit"])
}

// 拒绝失去 Form 类型化上下文的字段项。
pub(crate) fn generate_orphan_form_input_item(
    // 接收越界字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        element.span,
        "<FormInputItem> 只能作为 <Form> 的直接子项",
        "把 FormInputItem 放入绑定 model 的 Form 内",
    ))
}

// 生成单个类型化文本字段链。
fn generate_input_field(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormInputItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 标签省略时沿用字段 key。
    let label = find_attribute(element, "label")
        // 显式标签必须是字符串。
        .map(|attribute| literal_string(attribute, "FormInputItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持既有回退行为。
        .unwrap_or_else(|| field.clone());
    // 解析当前登记规则。
    let (required, email) = parse_rules(element)?;
    // 拒绝未知字段属性。
    for attribute in &element.attributes {
        // 当前仅消费三项属性。
        if !matches!(attribute.name.as_str(), "field" | "label" | "rules") {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                attribute.span,
                format!("FormInputItem 属性 {} 尚无已登记映射", attribute.name),
                "当前使用 field、label 与 rules",
            ));
        }
    }
    // 字段项必须是叶节点。
    if element
        .children
        .iter()
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            element.span,
            "<FormInputItem> 不接受子节点",
            "使用自闭合 FormInputItem",
        ));
    }
    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            ::uix::prelude::FormInputItem::new(#field)
                .label(#label)
                .required(#required)
                .email(#email),
        )
    })
}

// 解析逗号分隔的 required/email 规则。
fn parse_rules(element: &Element) -> Result<(bool, bool), Diagnostic> {
    // 没有 rules 时返回空规则集合。
    let Some(attribute) = find_attribute(element, "rules") else {
        // 保持规则默认关闭。
        return Ok((false, false));
    };
    // 规则必须在编译期确定。
    let rules = literal_string(attribute, "FormInputItem rules")?;
    // 保存两项规则开关。
    let (mut required, mut email) = (false, false);
    // 按源码顺序解析关键字。
    for rule in rules.split(',').map(str::trim) {
        // 映射且拒绝重复项。
        match rule {
            "required" if !required => required = true,
            "email" if !email => email = true,
            _ => {
                // 返回精确规则诊断。
                return Err(Diagnostic::new(
                    attribute.span,
                    format!("FormInputItem rules 包含未登记或重复规则 {rule:?}"),
                    "当前使用 required、email，多个规则以逗号分隔",
                ));
            }
        }
    }
    // 返回确定规则集合。
    Ok((required, email))
}

// 生成提交按钮并消费 Form 上下文操作。
fn generate_submit_button(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 提交按钮必须声明 click。
    let click = required_attribute(element, "@click")?;
    // 点击值必须是表达式。
    let AttributeValue::Expression(expression) = &click.value else {
        // 返回事件形状诊断。
        return Err(form_shape_diagnostic(element));
    };
    // 接受裸操作名或零参数调用。
    let is_submit = match &expression.expression.kind {
        ExpressionKind::Identifier(name) => name == "submitForm",
        ExpressionKind::Call { callee, arguments } => {
            arguments.is_empty()
                && matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "submitForm")
        }
        _ => false,
    };
    // 非 submitForm 必须失败。
    if !is_submit {
        // 返回提交入口诊断。
        return Err(form_shape_diagnostic(element));
    }
    // 移除由 Form 上下文消费的点击属性。
    let mut button = element.clone();
    // 保留其余按钮公共映射。
    button
        .attributes
        .retain(|attribute| attribute.name != "@click");
    // 委托既有按钮生成器。
    generate_button_with_group_position(&button, None)
}

// 生成返回 Result<(), String> 的 Form 提交处理器。
fn generate_submit_handler(
    // 接收提交表达式。
    expression: &Expression,
    // 接收类型化模型变量。
    submitted: &Ident,
) -> Result<TokenStream, Diagnostic> {
    // 裸处理器名自动接收模型。
    if let ExpressionKind::Identifier(name) = &expression.kind {
        // 验证 Rust 处理器名。
        let handler = rust_identifier(name, expression.span)?;
        // 返回调用方 Result。
        return Ok(quote! { (#handler)(#submitted) });
    }
    // 显式调用通过 $event 引用模型。
    generate_handler_expression(expression, Some(submitted))
}

// 构造 Form 首批形状诊断。
fn form_shape_diagnostic(element: &Element) -> Diagnostic {
    // 返回统一提交闭环建议。
    Diagnostic::new(
        element.span,
        "<Form> 首批要求直接 FormInputItem 与唯一 submitForm Button",
        "把字段放在前面，并以 <Button @click=\"submitForm\">提交</Button> 结束",
    )
}

// 查找具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
}

// 查找必需属性。
fn required_attribute<'a>(
    // 接收完整元素。
    element: &'a Element,
    // 接收属性名。
    name: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 缺失时返回组件级诊断。
    find_attribute(element, name).ok_or_else(|| {
        Diagnostic::new(
            element.span,
            format!("<{}> 缺少必需的 {name} 属性", element.name),
            "按 Form 类型化映射文档补齐必需属性",
        )
    })
}
