// 引入卫生标识符与过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与按钮生成边界。
use super::codegen::{apply_common_attributes, generate_button_with_group_position};
// 引入独立的类型化滑块字段生成边界。
use super::form_slider_codegen::generate_form_slider_field;
// 引入 Form 语法树、表达式和值映射契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Expression, ExpressionKind, Node,
    SOURCE_ID_ATTRIBUTE, boolean_value, generate_event_handler_expression, generate_expression,
    literal_string, rust_identifier, string_value,
};

// 生成类型化 Form 与已登记的字段适配器。
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
        // Form 直接解释字段子项，因此先移除编译器注入的内部源码标记。
        let mut child = child.clone();
        child
            .attributes
            .retain(|attribute| attribute.name != SOURCE_ID_ATTRIBUTE);
        // 已登记字段必须位于提交按钮之前。
        if matches!(
            child.name.as_str(),
            "FormInputItem"
                | "FormSelectItem"
                | "FormCheckboxItem"
                | "FormRadioItem"
                | "FormSwitchItem"
                | "FormSliderItem"
        ) {
            // 防止源码顺序被生成器重排。
            if submit_button.is_some() {
                // 返回字段顺序诊断。
                return Err(Diagnostic::new(
                    child.span,
                    format!("{} 必须位于 submitForm Button 之前", child.name),
                    "先声明全部字段，最后声明提交按钮",
                ));
            }
            // 按字段组件生成对应链片段。
            fields.push(match child.name.as_str() {
                // 文本字段映射到 FormInputItem。
                "FormInputItem" => generate_input_field(&child)?,
                // 选择字段映射到 FormSelectItem。
                "FormSelectItem" => generate_select_field(&child)?,
                // 布尔字段映射到 FormCheckboxItem。
                "FormCheckboxItem" => generate_checkbox_field(&child)?,
                // 单选组字段映射到 FormRadioItem。
                "FormRadioItem" => generate_radio_field(&child)?,
                // 开关字段映射到 FormSwitchItem。
                "FormSwitchItem" => generate_switch_field(&child)?,
                // f64 数值字段映射到 FormSliderItem。
                "FormSliderItem" => generate_form_slider_field(&child)?,
                // 前置匹配已穷尽登记字段。
                _ => unreachable!("已登记 Form 字段分派必须穷尽"),
            });
            // 继续处理下一节点。
            continue;
        }
        // Button 必须是唯一提交入口。
        if child.name == "Button" && submit_button.is_none() {
            // 生成移除内置点击后的按钮。
            submit_button = Some(generate_submit_button(&child)?);
            // 继续处理下一节点。
            continue;
        }
        // 拒绝未登记子项或重复按钮。
        return Err(Diagnostic::new(
            child.span,
            format!("<Form> 直接子项 <{}> 尚未进入首批类型化映射", child.name),
            "当前使用已映射 Form 字段项与一个 submitForm Button",
        ));
    }
    // 至少一个字段才能形成类型化表单。
    if fields.is_empty() {
        // 返回空表单诊断。
        return Err(Diagnostic::new(
            element.span,
            "<Form> 至少需要一个直接类型化字段项",
            "添加已映射的 Form 类型化字段项",
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
    // 生成按声明顺序排列的全模型规则构建链。
    let model_rules = generate_model_rules(element)?;
    // 生成运行时拥有校验与提交生命周期的 View。
    let view = quote! {{
        // 构建类型化字段与回调。
        let #form = ::uix::prelude::Form::model(&(#model))
            #(#fields)*
            #(#model_rules)*
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
    apply_common_attributes(view, &element.attributes, &["model", "rules", "@submit"])
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

// 拒绝失去 Form 类型化上下文的选择字段项。
pub(crate) fn generate_orphan_form_select_item(
    // 接收越界选择字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        element.span,
        "<FormSelectItem> 只能作为 <Form> 的直接子项",
        "把 FormSelectItem 放入绑定 model 的 Form 内",
    ))
}

// 拒绝失去 Form 类型化上下文的复选字段项。
pub(crate) fn generate_orphan_form_checkbox_item(
    // 接收越界复选字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        element.span,
        "<FormCheckboxItem> 只能作为 <Form> 的直接子项",
        "把 FormCheckboxItem 放入绑定 model 的 Form 内",
    ))
}

// 拒绝失去 Form 类型化上下文的单选组字段项。
pub(crate) fn generate_orphan_form_radio_item(
    // 接收越界单选组字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        element.span,
        "<FormRadioItem> 只能作为 <Form> 的直接子项",
        "把 FormRadioItem 放入绑定 model 的 Form 内",
    ))
}

// 拒绝失去 Form 类型化上下文的开关字段项。
pub(crate) fn generate_orphan_form_switch_item(
    // 接收越界开关字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        // 指向完整越界元素。
        element.span,
        // 说明类型化字段必须由 Form 解释。
        "<FormSwitchItem> 只能作为 <Form> 的直接子项",
        // 给出恢复上下文的规范写法。
        "把 FormSwitchItem 放入绑定 model 的 Form 内",
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

// 生成单个类型化选择字段链。
fn generate_select_field(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormSelectItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 标签省略时沿用字段 key。
    let label = find_attribute(element, "label")
        // 显式标签必须是字符串。
        .map(|attribute| literal_string(attribute, "FormSelectItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持字段 key 回退行为。
        .unwrap_or_else(|| field.clone());
    // 选择字段必须显式提供候选集合。
    let options_attribute = required_attribute(element, "options")?;
    // 候选集合必须保留 Rust 侧类型。
    let AttributeValue::Expression(options_expression) = &options_attribute.value else {
        // 返回候选集合形状诊断。
        return Err(Diagnostic::new(
            options_attribute.span,
            "FormSelectItem options 必须绑定可迭代字符串表达式",
            "使用 options={level_options}",
        ));
    };
    // 生成受限候选集合表达式。
    let options = generate_expression(&options_expression.expression, None)?;
    // 选择字段当前只登记 required 规则。
    let required = parse_required_rule(element, "FormSelectItem")?;
    // 拒绝未知字段属性。
    for attribute in &element.attributes {
        // 只消费当前已登记选择字段属性。
        if !matches!(
            attribute.name.as_str(),
            "field" | "label" | "options" | "rules" | "searchable" | "placeholder" | "disabled"
        ) {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                attribute.span,
                format!("FormSelectItem 属性 {} 尚无已登记映射", attribute.name),
                "当前使用 field、label、options、rules、searchable、placeholder 与 disabled",
            ));
        }
    }
    // 选择字段项必须是叶节点。
    if element
        // 遍历全部直接子节点。
        .children
        // 获取只读迭代器。
        .iter()
        // 排版空白以外的节点均不合法。
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            element.span,
            "<FormSelectItem> 不接受子节点",
            "使用自闭合 FormSelectItem",
        ));
    }
    // 创建基础选择字段构建链。
    let mut item = quote! {
        ::uix::prelude::FormSelectItem::new(#field)
            .label(#label)
            .options((#options).clone())
            .required(#required)
    };
    // 可搜索状态接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "searchable") {
        // 生成统一布尔属性令牌。
        let searchable = boolean_value(attribute)?;
        // 运行时启用式构建器通过同类型分支保留 false 默认值。
        item = quote! {{
            // 确保基础字段项只求值一次。
            let __uix_form_select_item = #item;
            // 仅在配置为真时启用搜索能力。
            if #searchable {
                // 调用运行时搜索启用入口。
                __uix_form_select_item.searchable()
            } else {
                // 保留默认非搜索字段项。
                __uix_form_select_item
            }
        }};
    }
    // 占位文本接受字符串字面量或表达式。
    if let Some(attribute) = find_attribute(element, "placeholder") {
        // 生成统一字符串属性令牌。
        let placeholder = string_value(attribute)?;
        // 应用运行时占位文本构建器。
        item = quote! { (#item).placeholder(#placeholder) };
    }
    // 禁用状态接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成统一布尔属性令牌。
        let disabled = boolean_value(attribute)?;
        // 应用运行时禁用构建器。
        item = quote! { (#item).disabled(#disabled) };
    }
    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            #item,
        )
    })
}

// 生成单个类型化布尔复选字段链。
fn generate_checkbox_field(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormCheckboxItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 表单标签省略时沿用字段 key。
    let field_label = find_attribute(element, "label")
        // 显式表单标签必须是字符串。
        .map(|attribute| literal_string(attribute, "FormCheckboxItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持字段 key 回退行为。
        .unwrap_or_else(|| field.clone());
    // 复选字段当前只登记 required 规则。
    let required = parse_required_rule(element, "FormCheckboxItem")?;
    // 拒绝未知字段属性。
    for attribute in &element.attributes {
        // 只消费当前已登记复选字段属性。
        if !matches!(
            attribute.name.as_str(),
            "field" | "label" | "text" | "rules" | "disabled"
        ) {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                attribute.span,
                format!("FormCheckboxItem 属性 {} 尚无已登记映射", attribute.name),
                "当前使用 field、label、text、rules 与 disabled",
            ));
        }
    }
    // 复选字段项必须是叶节点。
    if element
        // 遍历全部直接子节点。
        .children
        // 获取只读迭代器。
        .iter()
        // 排版空白以外的节点均不合法。
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            element.span,
            "<FormCheckboxItem> 不接受子节点",
            "使用自闭合 FormCheckboxItem",
        ));
    }
    // 创建基础复选字段构建链。
    let mut item = quote! {
        ::uix::prelude::FormCheckboxItem::new(#field)
            .field_label(#field_label)
            .required(#required)
    };
    // 复选框自身文字接受字符串字面量或表达式。
    if let Some(attribute) = find_attribute(element, "text") {
        // 生成统一字符串属性令牌。
        let text = string_value(attribute)?;
        // 保留运行时 label 的旧有控件文字语义。
        item = quote! { (#item).label(#text) };
    }
    // 禁用状态接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成统一布尔属性令牌。
        let disabled = boolean_value(attribute)?;
        // 应用运行时禁用构建器。
        item = quote! { (#item).disabled(#disabled) };
    }
    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成 bool 字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            #item,
        )
    })
}

// 生成单个类型化布尔开关字段链。
fn generate_switch_field(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormSwitchItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 表单标签省略时沿用字段 key。
    let label = find_attribute(element, "label")
        // 显式表单标签必须是编译期字符串。
        .map(|attribute| literal_string(attribute, "FormSwitchItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持字段 key 回退行为。
        .unwrap_or_else(|| field.clone());
    // 开关字段当前只登记 required 规则。
    let required = parse_required_rule(element, "FormSwitchItem")?;
    // 拒绝未知字段属性。
    for attribute in &element.attributes {
        // 只消费当前已登记开关字段属性。
        if !matches!(
            // 检查稳定属性名。
            attribute.name.as_str(),
            // 保持静态适配器的最小公开契约。
            "field" | "label" | "rules" | "disabled"
        ) {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                // 指向未知属性。
                attribute.span,
                // 说明未登记的具体属性。
                format!("FormSwitchItem 属性 {} 尚无已登记映射", attribute.name),
                // 列出当前允许的完整属性集合。
                "当前使用 field、label、rules 与 disabled",
            ));
        }
    }
    // 开关字段项必须是叶节点。
    if element
        // 遍历全部直接子节点。
        .children
        // 获取只读迭代器。
        .iter()
        // 排版空白以外的节点均不合法。
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            // 指向完整开关字段项。
            element.span,
            // 说明子节点不属于开关契约。
            "<FormSwitchItem> 不接受子节点",
            // 给出规范自闭合写法。
            "使用自闭合 FormSwitchItem",
        ));
    }
    // 创建基础开关字段构建链。
    let mut item = quote! {
        ::uix::prelude::FormSwitchItem::new(#field)
            .label(#label)
            .required(#required)
    };
    // 禁用状态接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成统一布尔属性令牌。
        let disabled = boolean_value(attribute)?;
        // 应用运行时禁用构建器。
        item = quote! { (#item).disabled(#disabled) };
    }
    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成 bool 字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            #item,
        )
    })
}

// 生成单个类型化单选组字段链。
fn generate_radio_field(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormRadioItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 表单标签省略时沿用字段 key。
    let label = find_attribute(element, "label")
        // 显式标签必须是编译期字符串。
        .map(|attribute| literal_string(attribute, "FormRadioItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持字段 key 回退行为。
        .unwrap_or_else(|| field.clone());
    // 单选组必须显式提供候选集合。
    let options_attribute = required_attribute(element, "options")?;
    // 候选集合必须保留 Rust 侧类型。
    let AttributeValue::Expression(options_expression) = &options_attribute.value else {
        // 返回候选集合形状诊断。
        return Err(Diagnostic::new(
            options_attribute.span,
            "FormRadioItem options 必须绑定可迭代字符串表达式",
            "使用 options={channel_options}",
        ));
    };
    // 生成受限候选集合表达式。
    let options = generate_expression(&options_expression.expression, None)?;
    // 单选组当前只登记 required 规则。
    let required = parse_required_rule(element, "FormRadioItem")?;
    // 拒绝未知字段属性。
    for attribute in &element.attributes {
        // 只消费当前已登记单选组属性。
        if !matches!(
            attribute.name.as_str(),
            "field" | "label" | "options" | "rules" | "groupName" | "disabled" | "vertical"
        ) {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                attribute.span,
                format!("FormRadioItem 属性 {} 尚无已登记映射", attribute.name),
                "当前使用 field、label、options、rules、groupName、disabled 与 vertical",
            ));
        }
    }
    // 单选组字段项必须是叶节点。
    if element
        // 遍历全部直接子节点。
        .children
        // 获取只读迭代器。
        .iter()
        // 排版空白以外的节点均不合法。
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            element.span,
            "<FormRadioItem> 不接受子节点",
            "使用自闭合 FormRadioItem",
        ));
    }
    // 创建基础单选组字段构建链。
    let mut item = quote! {
        ::uix::prelude::FormRadioItem::new(#field)
            .label(#label)
            .options((#options).clone())
            .required(#required)
    };
    // 可选分组名接受字符串字面量或表达式。
    if let Some(attribute) = find_attribute(element, "groupName") {
        // 生成统一字符串属性令牌。
        let group_name = string_value(attribute)?;
        // 应用运行时分组名构建器。
        item = quote! { (#item).group_name(#group_name) };
    }
    // 禁用状态接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成统一布尔属性令牌。
        let disabled = boolean_value(attribute)?;
        // 应用运行时禁用构建器。
        item = quote! { (#item).disabled(#disabled) };
    }
    // 纵向布局接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "vertical") {
        // 生成统一布尔属性令牌。
        let vertical = boolean_value(attribute)?;
        // 启用式构建器通过同类型分支保留 false 默认值。
        item = quote! {{
            // 确保基础字段项只求值一次。
            let __uix_form_radio_item = #item;
            // 仅在配置为真时启用纵向布局。
            if #vertical {
                // 调用运行时纵向布局入口。
                __uix_form_radio_item.vertical()
            } else {
                // 保留默认横向布局。
                __uix_form_radio_item
            }
        }};
    }
    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成 String 字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            #item,
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

// 生成 Form 顶层全模型规则的有序构建链。
fn generate_model_rules(element: &Element) -> Result<Vec<TokenStream>, Diagnostic> {
    // 未声明时保持空规则集合。
    let Some(attribute) = find_attribute(element, "rules") else {
        // 返回无附加链片段。
        return Ok(Vec::new());
    };
    // 顶层规则必须使用表达式数组。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回数组形状诊断。
        return Err(Diagnostic::new(
            attribute.span,
            "Form rules 必须是全模型校验函数标识符数组",
            "使用 rules={[validate_profile, validate_access]}",
        ));
    };
    // 只接受编译期结构明确的数组字面量。
    let ExpressionKind::Array(rules) = &expression.expression.kind else {
        // 动态集合不能保证函数签名与声明顺序。
        return Err(Diagnostic::new(
            attribute.span,
            "Form rules 必须使用数组字面量",
            "使用 rules={[validate_profile]}，函数签名为 fn(&M) -> Result<(), String>",
        ));
    };
    // 保存源码顺序中的构建链片段。
    rules
        // 按数组顺序遍历规则表达式。
        .iter()
        // 把每个函数标识符生成独立 model_rule 调用。
        .map(|rule| {
            // 首版只登记可由 Rust 类型检查签名的函数标识符。
            if !matches!(&rule.kind, ExpressionKind::Identifier(_)) {
                // 拒绝调用、运算与其他无法作为函数值的形状。
                return Err(Diagnostic::new(
                    rule.span,
                    "Form rules 数组项必须是全模型校验函数标识符",
                    "传入函数名，不要在 rules 中调用；签名为 fn(&M) -> Result<(), String>",
                ));
            }
            // 生成卫生的 Rust 函数标识符令牌。
            let validator = generate_expression(rule, None)?;
            // 追加公开运行时规则入口。
            Ok(quote! { .model_rule(#validator) })
        })
        // 收集全部有序链片段或首个诊断。
        .collect()
}

// 解析当前字段组件登记的唯一 required 规则。
fn parse_required_rule(element: &Element, widget: &str) -> Result<bool, Diagnostic> {
    // 没有 rules 时保持非必填默认值。
    let Some(attribute) = find_attribute(element, "rules") else {
        // 返回关闭状态。
        return Ok(false);
    };
    // 规则必须在编译期确定。
    let rules = literal_string(attribute, &format!("{widget} rules"))?;
    // 选择字段当前只允许唯一 required 规则。
    if rules.trim() == "required" {
        // 返回已启用状态。
        return Ok(true);
    }
    // 返回精确规则诊断。
    Err(Diagnostic::new(
        attribute.span,
        format!("{widget} rules 包含未登记或重复规则 {rules:?}"),
        "当前只使用 required",
    ))
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
    generate_event_handler_expression(expression, submitted, "@submit")
}

// 构造 Form 首批形状诊断。
fn form_shape_diagnostic(element: &Element) -> Diagnostic {
    // 返回统一提交闭环建议。
    Diagnostic::new(
        element.span,
        "<Form> 要求直接类型化字段项与唯一 submitForm Button",
        "把字段放在前面，并以 <Button @click=\"submitForm\">提交</Button> 结束",
    )
}

// 查找具名属性。

// 查找必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        // 诊断标签跟随实际元素名。
        &element.name,
        "按 Form 类型化映射文档补齐必需属性",
    )
}
