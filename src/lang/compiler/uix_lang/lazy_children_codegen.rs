//! Generic lazy For-slot lowering. Rendering APIs are declared by the owning library.

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use super::codegen::{generate_node_view, is_renderable_node,
                      parse_for_iteration_setup};
use super::for_identity_codegen::optional_internal_control_ident;
use super::{
    Attribute, AttributeValue, ControlBinding, Diagnostic, Element, Expression, ExpressionKind,
    ExpressionNode, Node, generate_expression, generate_expression_without_source_marker,
    rust_identifier,
};

pub(super) fn generate(
    element: &Element, data_property: &str, item_property: Option<&str>,
    render_template: &str, keyed_template: &str,
    inputs: &std::collections::BTreeMap<String, TokenStream>,
) -> Result<TokenStream, Diagnostic> {
    let data_attribute = required_attribute(element, data_property)?;
    let data = expression_attribute(data_attribute, "惰性子节点 data")?;
    let template = lazy_template(element)?;
    let Some(ControlBinding::For {
        binding,
        binding_span,
        index_binding,
        index_span,
        iterable,
        key,
    }) = template.control.as_ref()
    else {
        return Err(Diagnostic::new(
            template.span,
            "惰性子节点 的 <For> 缺少循环绑定",
            "使用 <For {item} in {data}>...</For>",
        ));
    };
    let data_tokens = generate_expression(&data.expression, None)?;
    let data_comparison = generate_expression_without_source_marker(&data.expression, None)?;
    let iterable_tokens = generate_expression_without_source_marker(&iterable.expression, None)?;
    if data_comparison.to_string() != iterable_tokens.to_string() {
        return Err(Diagnostic::new(
            iterable.span,
            "惰性子节点 data 与直接 <For> 的 in 数据源不一致",
            "让 data={items} 与 <For ... in {items}> 使用同一表达式",
        ));
    }
    let data = data_tokens;
    if let Some(name) = item_property { validate_item_attribute(element, name, binding)?; }
    let row = single_row_view(template)?;
    let row = generate_node_view(row)?;
    let key = key
        .as_ref()
        .map(|value| {
            validate_lazy_key_expression(
                &value.expression,
                binding,
                index_binding.as_deref(),
            )?;
            generate_expression(&value.expression, None)
        })
        .transpose()?;
    let binding = rust_identifier(binding, *binding_span)?;
    let index_binding = index_binding
        .as_deref()
        .map(|name| rust_identifier(name, index_span.unwrap_or(*binding_span)))
        .transpose()?;
    let data_snapshot = Ident::new("__uix_lazy_data", Span::mixed_site());
    let item_count = Ident::new("__uix_lazy_count", Span::mixed_site());
    let item_index = Ident::new("__uix_lazy_index", Span::mixed_site());
    let index_statement = index_binding.map(|index_binding| {
        quote! { let #index_binding = #item_index; }
    });
    let key_data_snapshot = Ident::new("__uix_lazy_key_data", Span::mixed_site());
    let row_path = optional_internal_control_ident(template, "__uix_for_path")?;
    let row_setup = parse_for_iteration_setup(&template.for_iteration_setup, element.span)?;
    let row_clones = template
        .for_iteration_clones
        .iter()
        .map(|name| Ident::new(name, Span::call_site()))
        .collect::<Vec<_>>();
    let row_outer_captures = template
        .for_iteration_outer_captures
        .iter()
        .map(|name| Ident::new(name, Span::call_site()))
        .collect::<Vec<_>>();
    let row_path_decl = row_path.map(|path| {
        quote! {
            let #path = ::std::format!("lazy-row|{}", #item_index);
        }
    });
    let row_scope = quote! {
        #row_path_decl
        #(#row_setup)*
        #(let #row_clones = (#row_clones).clone();)*
    };
    let renderer = quote! {
        move |#item_index| {
            let #binding = (#data_snapshot)[#item_index].clone();
            #index_statement
            #row_scope
            #row
        }
    };
    let mut inputs = inputs.clone();
    inputs.insert("count".into(), quote! { #item_count });
    inputs.insert("render".into(), renderer);
    let (key_snapshot_setup, template) = if let Some(key) = key {
        inputs.insert("key".into(), quote! {
            move |#item_index| {
                let #binding = (#key_data_snapshot)[#item_index].clone();
                #index_statement
                #key
            }
        });
        (quote! { let #key_data_snapshot = ::std::sync::Arc::clone(&#data_snapshot); }, keyed_template)
    } else { (quote! {}, render_template) };
    let rendered = super::component_codegen::template(template, &inputs, element.span)?;
    let base = quote! {{
        let #data_snapshot = ::std::sync::Arc::new((#data).clone());
        let #item_count = (#data_snapshot).len();
        #(
            let #row_outer_captures = ::std::clone::Clone::clone(&#row_outer_captures);
        )*
        #key_snapshot_setup
        #rendered
    }};
    Ok(base)
}

fn validate_lazy_key_expression(
    expression: &Expression,
    binding: &str,
    index_binding: Option<&str>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ExpressionKind::LoweredAction(_) => Err(Diagnostic::new(
            expression.span,
            "惰性子节点 的 For key 不能调用 action",
            "使用 key={item.id} 或 item/index 的纯成员与算术表达式",
        )),
        ExpressionKind::Identifier(name)
            if name == binding || index_binding.is_some_and(|index| index == name) =>
        {
            Ok(())
        }
        ExpressionKind::Identifier(_) => Err(Diagnostic::new(
            expression.span,
            "惰性子节点 的 For key 只能依赖当前 item 或 index",
            "使用 key={item.id}，不要在 key 中引用外部状态",
        )),
        ExpressionKind::Number(_) | ExpressionKind::Boolean(_) => {
            Ok(())
        }
        ExpressionKind::String(_) => Err(Diagnostic::new(
            expression.span,
            "惰性子节点 的 For key 不支持字符串字面量",
            "使用 item 的字符串字段（如 item.id）参与 key",
        )),
        ExpressionKind::Unary { operand, .. } => {
            validate_lazy_key_expression(operand, binding, index_binding)
        }
        ExpressionKind::Binary { left, right, .. } => {
            validate_lazy_key_expression(left, binding, index_binding)?;
            validate_lazy_key_expression(right, binding, index_binding)
        }
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_lazy_key_expression(condition, binding, index_binding)?;
            validate_lazy_key_expression(then_branch, binding, index_binding)?;
            validate_lazy_key_expression(else_branch, binding, index_binding)
        }
        ExpressionKind::Member { object, .. } => {
            validate_lazy_key_expression(object, binding, index_binding)
        }
        ExpressionKind::Index { object, index } => {
            validate_lazy_key_expression(object, binding, index_binding)?;
            validate_lazy_key_expression(index, binding, index_binding)
        }
        ExpressionKind::Call { .. }
        | ExpressionKind::Object(_)
        | ExpressionKind::Array(_)
        | ExpressionKind::Closure { .. } => {
            Err(Diagnostic::new(
                expression.span,
                "惰性子节点 的 For key 不能包含调用、数组、对象字面量或闭包",
                "使用 key={item.id} 或 item/index 的纯成员与算术表达式",
            ))
        }
    }
}

fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    super::required_attribute(
        element,
        name,
        "惰性子节点",
        "使用 <惰性子节点 data={items} rowHeight=\"32px\"><For {item} in {items}>...</For></惰性子节点>",
    )
}

fn expression_attribute<'a>(
    attribute: &'a Attribute,
    label: &str,
) -> Result<&'a ExpressionNode, Diagnostic> {
    let AttributeValue::Expression(expression) = &attribute.value else {
        return Err(Diagnostic::new(
            attribute.span,
            format!("{label} 必须使用花括号表达式"),
            "使用 data={items}",
        ));
    };
    Ok(expression)
}

fn lazy_template(element: &Element) -> Result<&Element, Diagnostic> {
    let children = element
        .children
        .iter()
        .filter(|child| is_renderable_node(child))
        .collect::<Vec<_>>();
    if children.len() != 1 {
        return Err(Diagnostic::new(
            element.span,
            "<惰性子节点> 必须恰好包含一个直接 <For> 行模板",
            "用一个 <For {item} in {items}>...</For> 包裹行内容",
        ));
    }
    let Node::Element(template) = children[0] else {
        return Err(Diagnostic::new(
            element.span,
            "惰性子节点 的直接子节点必须是 <For>",
            "使用 <For {item} in {items}>...</For>",
        ));
    };
    if template.name != "For" {
        return Err(Diagnostic::new(
            template.span,
            format!("惰性子节点 的直接子节点不能是 <{}>", template.name),
            "改用直接 <For> 声明惰性行模板",
        ));
    }
    Ok(template)
}

fn validate_item_attribute(element: &Element, name: &str, binding: &str) -> Result<(), Diagnostic> {
    let Some(attribute) = element
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
    else {
        return Ok(());
    };
    let item = match &attribute.value {
        AttributeValue::Literal(value) => value.as_str(),
        AttributeValue::Expression(expression) => {
            let ExpressionKind::Identifier(value) = &expression.expression.kind else {
                return Err(Diagnostic::new(
                    attribute.span,
                    "惰性子节点 item 只能是行变量标识符",
                    "使用 item={row}，并让 <For {row} ...> 使用同名绑定",
                ));
            };
            value.as_str()
        }
        AttributeValue::InlineStyle(_) => {
            return Err(Diagnostic::new(
                attribute.span,
                "惰性子节点 item 不能使用内联样式",
                "使用 item={row}",
            ));
        }
    };
    if item != binding {
        return Err(Diagnostic::new(
            attribute.span,
            format!("惰性子节点 item={item:?} 与 <For> 行变量 {binding:?} 不一致"),
            "删除 item 属性，或让 item 与 <For> 使用同一行变量名",
        ));
    }
    Ok(())
}

fn single_row_view(template: &Element) -> Result<&Node, Diagnostic> {
    let rows = template
        .children
        .iter()
        .filter(|child| is_renderable_node(child))
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(Diagnostic::new(
            template.span,
            "惰性子节点 的 <For> 必须恰好生成一个直接行根 View",
            "用 Container、Column、Row 或 Grid 包裹多个行内节点",
        ));
    }
    if matches!(rows[0], Node::Element(element) if matches!(element.name.as_str(), "If" | "For")) {
        return Err(Diagnostic::new(
            template.span,
            "惰性子节点 行根不能是 If 或嵌套 For",
            "用 Container 包裹行内控制内容",
        ));
    }
    Ok(rows[0])
}
