//! One emitter for all declared Rust components. The declaration supplies API
//! mappings; this module owns syntax validation and expression generation.

use super::{Attribute, AttributeValue, Diagnostic, Element, Node, SourceSpan};
use crate::lang::compiler::components::{ChildrenDeclaration, ComponentDeclaration, PropertyDeclaration, PropertyKind};
use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::BTreeMap;

pub(super) fn generate(element: &Element, declaration: &ComponentDeclaration) -> Result<TokenStream, Diagnostic> {
    generate_with_inputs(element, declaration, BTreeMap::new())
}

fn generate_with_inputs(element: &Element, declaration: &ComponentDeclaration, mut inputs: BTreeMap<String, TokenStream>) -> Result<TokenStream, Diagnostic> {
    crate::lang::compiler::components::use_unit(&declaration.unit);
    let span = element.span;
    if declaration.reject_events && element.attributes.iter().any(|a| a.name.starts_with('@')) { return Err(error(span, "this component does not accept events")); }
    for group in &declaration.required_any {
        if !group.iter().any(|name| element.attributes.iter().any(|a| &a.name == name)) { return Err(error(span, format!("requires one of {}", group.join(", ")))); }
    }
    for group in &declaration.exclusive_properties {
        if group.iter().filter(|name| element.attributes.iter().any(|a| &a.name == *name)).count() > 1 {
            return Err(error(span, format!("properties {} are mutually exclusive", group.join(", "))));
        }
    }

    for (name, property) in &declaration.properties {
        let attribute = element.attributes.iter().find(|attribute| &attribute.name == name);
        let present = attribute.is_some();
        inputs.insert(format!("has_{name}"), quote! { #present });
        let value = match attribute {
            Some(attribute) => Some(property_value(attribute, property)?),
            None if property.required => return Err(error(span, format!("<{}> requires property {name}", element.name))),
            None => property.default.as_ref().map(|value| template(value, &inputs, span)).transpose()?,
        };
        if let Some(value) = value { inputs.insert(name.clone(), value); }
    }
    let mut default_children = Vec::new();
    let mut slots = BTreeMap::new();
    let mut trailing_slot = false;
    for child in &element.children {
        let Node::Element(child_element) = child else { default_children.push(child.clone()); continue; };
        let attribute = child_element.attributes.iter().find(|attribute| attribute.name == "slot");
        let name = if let Some(attribute) = attribute { super::literal_string(attribute, "slot")? }
            else if let Some((name, _)) = declaration.slots.iter().find(|(_, slot)| slot.tag.as_ref() == Some(&child_element.name)) { name.clone() }
            else {
                if trailing_slot { return Err(error(child_element.span, "ordinary children must precede a trailing slot")); }
                default_children.push(child.clone()); continue;
            };
        let slot_span = attribute.map_or(child_element.span, |a| a.span);
        if !declaration.slots.contains_key(&name) { return Err(error(slot_span, format!("unknown slot {name}"))); }
        let slot = &declaration.slots[&name];
        if slot.static_element && (child_element.control.is_some() || matches!(child_element.name.as_str(), "If" | "For" | "Else" | "ElseIf" | "KernelChildren")) {
            return Err(error(slot_span, "this slot requires a static element root"));
        }
        if slot.exclusive_properties.iter().any(|name| element.attributes.iter().any(|a| &a.name == name)) {
            return Err(error(slot_span, "slot conflicts with a declared property"));
        }
        if child_element.attributes.iter().filter(|a| a.name == "slot").count() > 1 { return Err(error(slot_span, "duplicate slot attribute")); }
        if slots.contains_key(&name) { return Err(error(slot_span, format!("duplicate slot {name}"))); }
        let mut child_element = child_element.clone();
        child_element.attributes.retain(|attribute| attribute.name != "slot");
        trailing_slot |= slot.trailing;
        for (event, action) in &slot.bound_events {
            let attribute = child_element.attributes.iter().find(|attribute| &attribute.name == event).ok_or_else(|| error(slot_span, format!("slot requires event {event}")))?;
            let AttributeValue::Expression(expression) = &attribute.value else { return Err(error(attribute.span, "expected a bound action")); };
            let valid = match &expression.expression.kind {
                super::ExpressionKind::Identifier(name) => name == action,
                super::ExpressionKind::Call { callee, arguments } => arguments.is_empty() && matches!(&callee.kind, super::ExpressionKind::Identifier(name) if name == action),
                _ => false,
            };
            if !valid { return Err(error(attribute.span, format!("slot requires action {action}"))); }
            child_element.attributes.retain(|attribute| &attribute.name != event);
        }
        slots.insert(name, super::codegen::generate_node_view(&Node::Element(child_element))?);
    }
    let children_nodes = &default_children;
    let children = match &declaration.children {
        ChildrenDeclaration::None => {
            if children_nodes.iter().any(super::codegen::is_renderable_node) {
                return Err(error(span, format!("<{}> does not accept children", element.name)));
            }
            quote! { () }
        }
        ChildrenDeclaration::Text => super::codegen::generate_text_content(children_nodes, span)?,
        ChildrenDeclaration::TextFunction => super::dynamic_text_codegen::generate_text_function(children_nodes, span)?,
        ChildrenDeclaration::Views { minimum, maximum, tags, item_finish } => {
            let children = children_nodes.iter().filter(|node| super::codegen::is_renderable_node(node)).collect::<Vec<_>>();
            if !tags.is_empty() && children.iter().any(|node| !matches!(node, Node::Element(child) if tags.contains(&child.name) && child.control.is_none())) {
                return Err(error(span, format!("expected static children from {}", tags.join(", "))));
            }
            if minimum.is_some_and(|min| children.len() < min) || maximum.is_some_and(|max| children.len() > max) {
                return Err(error(span, format!("<{}> has an invalid number of children", element.name)));
            }
            if (minimum.is_some() || maximum.is_some()) && children.iter().any(|node| matches!(node, Node::Element(element) if matches!(element.name.as_str(), "If" | "ElseIf" | "Else" | "For" | "KernelChildren"))) {
                return Err(error(span, "a fixed-cardinality slot requires static child roots"));
            }
            if let Some(finish) = item_finish {
                let count = children.len();
                let values = children.iter().enumerate().map(|(index, node)| {
                    let Node::Element(child) = node else { return Err(error(span, "a decorated child must be an element")); };
                    let mut declaration = crate::lang::compiler::components::declaration(&child.name).ok_or_else(|| error(child.span, "child needs a component declaration"))?;
                    declaration.finish = Some(finish.clone());
                    generate_with_inputs(child, &declaration, BTreeMap::from([("index".into(), quote! { #index }), ("count".into(), quote! { #count })]))
                }).collect::<Result<Vec<_>, _>>()?;
                quote! { ::std::vec![#(#values),*] }
            } else { super::codegen::generate_children(children_nodes)? }
        },
        ChildrenDeclaration::Single { optional, static_element } => {
            let children = children_nodes.iter().filter(|node| super::codegen::is_renderable_node(node)).collect::<Vec<_>>();
            match children.as_slice() {
                [] if *optional => quote! { None::<::uix_app::prelude::ViewNode> },
                [child] => {
                    if *static_element && !matches!(child, Node::Element(child) if child.control.is_none() && !matches!(child.name.as_str(), "If" | "For" | "Else" | "ElseIf" | "KernelChildren")) {
                        return Err(error(span, "this slot requires a static element root"));
                    }
                    let child = super::codegen::generate_node_view(child)?;
                    if *optional { quote! { Some(#child) } } else { child }
                }
                _ => return Err(error(span, format!("<{}> requires {} child", element.name, if *optional { "at most one" } else { "exactly one" }))),
            }
        }
        ChildrenDeclaration::Fold { initial, tags, minimum } => {
            let mut parent = template(initial, &inputs, span)?;
            let children = children_nodes.iter().filter(|node| super::codegen::is_renderable_node(node)).collect::<Vec<_>>();
            if minimum.is_some_and(|min| children.len() < min) { return Err(error(span, "too few record children")); }
            for node in children {
                let Node::Element(child) = node else { return Err(error(span, "a record child must be an element")); };
                if child.control.is_some() || !tags.contains(&child.name) { return Err(error(child.span, format!("expected static children from {}", tags.join(", ")))); }
                let declaration = crate::lang::compiler::components::declaration(&child.name).ok_or_else(|| error(child.span, "child needs a component declaration"))?;
                if !declaration.record { return Err(error(child.span, "a fold child must be a record")); }
                parent = generate_with_inputs(child, &declaration, BTreeMap::from([("parent".into(), parent)]))?;
            }
            parent
        }
        ChildrenDeclaration::Lazy { data_property, item_property, render, render_keyed } => {
            super::lazy_children_codegen::generate(element, data_property, item_property.as_deref(), render, render_keyed, &inputs)?
        }
        ChildrenDeclaration::Records { tags, minimum } => {
            let mut values = Vec::new();
            for child in children_nodes.iter().filter(|node| super::codegen::is_renderable_node(node)) {
                let Node::Element(child) = child else { return Err(error(span, "record children must be elements")); };
                if !tags.contains(&child.name) { return Err(error(child.span, format!("expected one of {}", tags.join(", ")))); }
                let child_declaration = crate::lang::compiler::components::declaration(&child.name)
                    .ok_or_else(|| error(child.span, format!("undeclared record {}", child.name)))?;
                if !child_declaration.record { return Err(error(child.span, "a record slot requires a record declaration")); }
                values.push(generate(child, &child_declaration)?);
            }
            if minimum.is_some_and(|min| values.len() < min) { return Err(error(span, "too few record children")); }
            quote! { ::std::vec![#(#values),*] }
        }
    };
    inputs.insert("children".into(), children);
    let has_children = children_nodes.iter().any(super::codegen::is_renderable_node);
    inputs.insert("has_children".into(), quote! { #has_children });
    let mut widget = template(&declaration.constructor, &inputs, span)?;
    let mut consumed = Vec::new();
    let mut order = declaration.property_order.clone();
    order.extend(declaration.properties.keys().filter(|name| !declaration.property_order.contains(name)).cloned());
    for name in order {
        let property = declaration.properties.get(&name).ok_or_else(|| error(span, format!("unknown property {name} in declaration order")))?;
        consumed.push(name.clone());
        let Some(attribute) = element.attributes.iter().find(|attribute| attribute.name == name) else { continue; };
        let value = inputs.get(&name).ok_or_else(|| error(span, format!("missing property value {name}")))?.clone();
        let rust = if matches!(&attribute.value, AttributeValue::Expression(_)) { property.expression_rust.as_ref().or(property.rust.as_ref()) } else { property.rust.as_ref() };
        if let Some(rust) = rust {
            let mut values = inputs.clone(); values.insert("widget".into(), widget); values.insert("value".into(), value);
            widget = template(rust, &values, span)?;
        } else if let Some(method) = &property.method {
            let method: Ident = syn::parse_str(method).map_err(|_| error(span, format!("invalid Rust method {method}")))?;
            widget = quote! { (#widget).#method(#value) };
        }
    }
    for (name, declaration) in &declaration.slots {
        let Some(slot) = slots.remove(name) else {
            if declaration.required { return Err(error(span, format!("missing required slot {name}"))); }
            continue;
        };
        inputs.insert(format!("slot_{name}"), slot.clone());
        if !declaration.rust.is_empty() {
            let mut values = inputs.clone(); values.insert("widget".into(), widget); values.insert("slot".into(), slot);
            widget = template(&declaration.rust, &values, span)?;
        }
    }
    for before_finish in [true, false] {
      if !before_finish {
        if let Some(finish) = &declaration.finish {
            inputs.insert("widget".into(), widget);
            widget = template(finish, &inputs, span)?;
        }
      }
      for (name, event) in &declaration.events {
        if event.before_finish != before_finish { continue; }
        consumed.push(name.clone());
        let Some(attribute) = element.attributes.iter().find(|attribute| &attribute.name == name) else {
            if event.required { return Err(error(span, format!("requires event {name}"))); }
            continue;
        };
        let AttributeValue::Expression(expression) = &attribute.value else { return Err(error(attribute.span, "an event handler must be an expression")); };
        let event_ident = Ident::new("__uix_component_event", Span::mixed_site());
        let fields = event.fields.iter().map(String::as_str).collect::<Vec<_>>();
        super::event_payload_codegen::validate_event_payload_fields(&expression.expression, name, &fields)?;
        if event.no_payload && super::expression_uses_event(&expression.expression) { return Err(error(attribute.span, "this event has no payload")); }
        let handler = if event.bare_handler_payload && matches!(expression.expression.kind, super::ExpressionKind::Identifier(_)) {
            let function = super::generate_expression(&expression.expression, None)?;
            quote! { (#function)(#event_ident) }
        } else { super::generate_handler_expression(&expression.expression, if event.no_payload { None } else { Some(&event_ident) })? };
        let mut values = inputs.clone();
        values.insert("widget".into(), widget);
        values.insert("event".into(), quote! { #event_ident });
        values.insert("handler".into(), handler);
        widget = template(&event.rust, &values, attribute.span)?;
      }
    }
    if declaration.record {
        if let Some(map) = &declaration.view_map {
            let view = Ident::new("__uix_record_view", Span::mixed_site());
            let consumed = consumed.iter().map(String::as_str).collect::<Vec<_>>();
            let decorated = super::codegen::apply_common_attributes(quote! { #view }, &element.attributes, &consumed)?;
            return template(map, &BTreeMap::from([("record".into(), widget), ("view".into(), quote! { #view }), ("decorated".into(), decorated)]), span);
        }
        for attribute in &element.attributes {
            if attribute.name != super::SOURCE_ID_ATTRIBUTE && !consumed.contains(&attribute.name) {
                return Err(error(attribute.span, format!("unknown record property {}", attribute.name)));
            }
        }
        Ok(widget)
    } else {
        let consumed = consumed.iter().map(String::as_str).collect::<Vec<_>>();
        super::codegen::apply_common_attributes(widget, &element.attributes, &consumed)
    }
}

fn property_value(attribute: &Attribute, property: &PropertyDeclaration) -> Result<TokenStream, Diagnostic> {
    property_value_with_event(attribute, property, None)
}

pub(super) fn expression_value(expression: &super::Expression, property: &PropertyDeclaration, event: Option<&Ident>) -> Result<TokenStream, Diagnostic> {
    property_value_with_event(&expression_attribute("value", expression, property.kind), property, event)
}

fn property_value_with_event(attribute: &Attribute, property: &PropertyDeclaration, event: Option<&Ident>) -> Result<TokenStream, Diagnostic> {
    let property_value = |attribute: &Attribute, property: &PropertyDeclaration| property_value_with_event(attribute, property, event);
    if property.call_only && !matches!(&attribute.value, AttributeValue::Expression(super::ExpressionNode { expression: super::Expression { kind: super::ExpressionKind::Call { .. }, .. }, .. })) { return Err(error(attribute.span, "expected a constructor call")); }

    if let (Some(pattern), AttributeValue::Literal(value)) = (&property.literal_pattern, &attribute.value) {
        let pattern = regex::Regex::new(pattern).map_err(|e| error(attribute.span, format!("invalid declaration pattern: {e}")))?;
        if !pattern.is_match(value) { return Err(error(attribute.span, format!("{} does not match its declared value pattern", attribute.name))); }
    }
    if property.nonempty && matches!(&attribute.value, AttributeValue::Literal(value) if value.trim().is_empty()) { return Err(error(attribute.span, "expected a nonempty value")); }
    if property.literal_only && !matches!(&attribute.value, AttributeValue::Literal(_) | AttributeValue::Expression(super::ExpressionNode { expression: super::Expression { kind: super::ExpressionKind::Array(_) | super::ExpressionKind::Object(_), .. }, .. })) { return Err(error(attribute.span, "expected a literal value")); }
    let numeric = match &attribute.value {
        AttributeValue::Literal(value) => value.strip_suffix("px").unwrap_or(value).parse::<f64>().ok(),
        AttributeValue::Expression(expression) => match &expression.expression.kind {
            super::ExpressionKind::Number(value) => value.parse::<f64>().ok(),
            super::ExpressionKind::Unary { operator: super::UnaryOperator::Negate, operand } => match &operand.kind {
                super::ExpressionKind::Number(value) => value.parse::<f64>().ok().map(|v| -v), _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    if matches!(property.kind, PropertyKind::Number | PropertyKind::Integer) && numeric.is_some_and(|value|
        !value.is_finite() || property.minimum.is_some_and(|min| value < min)
        || property.maximum.is_some_and(|max| value > max)
        || property.exclusive_minimum.is_some_and(|min| value <= min)
        || (matches!(property.kind, PropertyKind::Integer) && value.fract() != 0.0)) {
        return Err(error(attribute.span, format!("{} is outside its declared numeric range", attribute.name)));
    }
    match property.kind {
        PropertyKind::Identifier => {
            let name = match &attribute.value {
                AttributeValue::Literal(value) => value.as_str(),
                AttributeValue::Expression(expression) => match &expression.expression.kind { super::ExpressionKind::Identifier(name) => name.as_str(), _ => return Err(error(attribute.span, "expected an identifier")) },
                _ => return Err(error(attribute.span, "expected an identifier")),
            };
            let ident = super::rust_identifier(name, attribute.span)?;
            Ok(quote! { #ident })
        }
        PropertyKind::String => {
            let value = match &attribute.value {
                AttributeValue::Expression(expression) => super::generate_expression(&expression.expression, event)?,
                _ => super::string_value(attribute)?,
            };
            if property.owned { Ok(quote! { ::std::string::ToString::to_string(&(#value)) }) } else { Ok(value) }
        },
        PropertyKind::Number => match &attribute.value {
            AttributeValue::Literal(value) => {
                let value = value.strip_suffix("px").unwrap_or(value).parse::<f64>().map_err(|_| error(attribute.span, "expected a number"))?;
                if !value.is_finite()
                    || property.minimum.is_some_and(|min| value < min)
                    || property.maximum.is_some_and(|max| value > max)
                    || property.exclusive_minimum.is_some_and(|min| value <= min) {
                    return Err(error(attribute.span, format!("{} is outside its declared numeric range", attribute.name)));
                }
                let value = proc_macro2::Literal::f64_unsuffixed(value);
                Ok(quote! { #value })
            }
            AttributeValue::Expression(expression) => {
                if let Some(value) = numeric { let value = proc_macro2::Literal::f64_unsuffixed(value); Ok(quote! { #value }) }
                else { super::generate_expression(&expression.expression, event) }
            },
            _ => Err(error(attribute.span, "expected a number or expression")),
        },
        PropertyKind::Integer => match &attribute.value {
            AttributeValue::Literal(value) => {
                value.parse::<i128>().map_err(|_| error(attribute.span, "expected an integer literal"))?;
                value.parse().map_err(|_| error(attribute.span, "invalid integer literal"))
            }
            AttributeValue::Expression(expression) => {
                if let super::ExpressionKind::Number(value) = &expression.expression.kind {
                    let value = value.parse::<f64>().map_err(|_| error(attribute.span, "invalid integer"))?;
                    if !value.is_finite() || value.fract() != 0.0 { return Err(error(attribute.span, "expected an integral value")); }
                    format!("{value:.0}").parse().map_err(|_| error(attribute.span, "invalid integer literal"))
                } else { super::generate_expression(&expression.expression, event) }
            }
            _ => Err(error(attribute.span, "expected an integer or expression")),
        },
        PropertyKind::Boolean => match &attribute.value {
            AttributeValue::Expression(expression) => super::generate_expression(&expression.expression, event),
            _ => super::boolean_value(attribute),
        },
        PropertyKind::GridTracks => {
            let source = super::literal_string(attribute, &attribute.name)?;
            let tracks = super::style_value_codegen::parse_grid_tracks(&source, &|message, hint| Diagnostic::new(attribute.span, message, hint))?;
            Ok(quote! { ::std::vec![#(#tracks),*] })
        }
        PropertyKind::Color => {
            let color = match &attribute.value {
                AttributeValue::Literal(value) => {
                    if let Some(rust) = property.values.get(value) { return template(rust, &BTreeMap::new(), attribute.span); }
                    let style = super::StyleProperty { name: attribute.name.clone(), value: super::StyleValue { source: value.clone(), hashes: Vec::new(), span: attribute.span }, span: attribute.span, media: None };
                    let (r, g, b, a) = super::style_value_codegen::parse_color(value, &style)?;
                    quote! { ::uix_app::prelude::Color::rgba(#r, #g, #b, #a) }
                }
                AttributeValue::Expression(expression) => super::generate_expression(&expression.expression, event)?,
                _ => return Err(error(attribute.span, "expected a color or expression")),
            };
            if let Some(value) = &property.value { template(value, &BTreeMap::from([("value".into(), color)]), attribute.span) }
            else { Ok(color) }
        },
        PropertyKind::Enumeration => {
            if property.allow_expression {
                if let AttributeValue::Expression(expression) = &attribute.value { return super::generate_expression(&expression.expression, event); }
            }
            let value = super::literal_string(attribute, &attribute.name)?;
            let rust = property.values.get(&value).ok_or_else(|| error(attribute.span, format!("{} accepts: {}", attribute.name, property.values.keys().cloned().collect::<Vec<_>>().join(", "))))?;
            template(rust, &BTreeMap::new(), attribute.span)
        }
        PropertyKind::Expression | PropertyKind::State => {
            let AttributeValue::Expression(expression) = &attribute.value else {
                let expected = if matches!(property.kind, PropertyKind::State) {
                    format!("a {} expression", property.state_handle.as_deref().unwrap_or("State<_>"))
                } else {
                    "an expression".to_owned()
                };
                return Err(error(attribute.span, format!("{} requires {expected}", attribute.name)));
            };
            let value = super::generate_expression(&expression.expression, event)?;
            if matches!(property.kind, PropertyKind::State) { Ok(quote! { &(#value) }) } else { Ok(value) }
        }
        PropertyKind::Object => {
            let AttributeValue::Expression(expression) = &attribute.value else { return Err(error(attribute.span, "expected an object expression")); };
            let super::ExpressionKind::Object(fields) = &expression.expression.kind else {
                return super::generate_expression(&expression.expression, event);
            };
            let mut values = BTreeMap::new();
            for field in fields {
                let declaration = property.fields.get(&field.name).ok_or_else(|| error(field.span, format!("unknown field {}", field.name)))?;
                values.insert(field.name.clone(), property_value(&expression_attribute(&field.name, &field.value, declaration.kind), declaration)?);
            }
            for (name, declaration) in &property.fields {
                if values.contains_key(name) { continue; }
                if let Some(default) = &declaration.default { values.insert(name.clone(), template(default, &values, attribute.span)?); }
                else if declaration.required { return Err(error(attribute.span, format!("missing field {name}"))); }
            }
            template(property.value.as_ref().ok_or_else(|| error(attribute.span, "object declaration needs a Rust value expression"))?, &values, attribute.span)
        }
        PropertyKind::Array => {
            if let (Some(separator), AttributeValue::Literal(source)) = (&property.separator, &attribute.value) {
                let item = property.item.as_ref().ok_or_else(|| error(attribute.span, "array declaration needs an item type"))?;
                let items = source.split(separator).map(|value| property_value(&Attribute { name: attribute.name.clone(), value: AttributeValue::Literal(value.trim().into()), span: attribute.span }, item)).collect::<Result<Vec<_>, _>>()?;
                if property.unique && items.iter().map(ToString::to_string).collect::<std::collections::BTreeSet<_>>().len() != items.len() { return Err(error(attribute.span, "duplicate array item")); }
                let items = quote! { ::std::vec![#(#items),*] };
                return if let Some(value) = &property.value { template(value, &BTreeMap::from([("items".into(), items)]), attribute.span) } else { Ok(items) };
            }
            let AttributeValue::Expression(expression) = &attribute.value else { return Err(error(attribute.span, "expected an array expression")); };
            let super::ExpressionKind::Array(items) = &expression.expression.kind else {
                return super::generate_expression(&expression.expression, event);
            };
            if property.maximum_items.is_some_and(|max| items.len() > max) { return Err(error(attribute.span, "too many array items")); }
            let item = property.item.as_ref().ok_or_else(|| error(attribute.span, "array declaration needs an item type"))?;
            let items = items.iter().map(|value| property_value(&expression_attribute("item", value, item.kind), item)).collect::<Result<Vec<_>, _>>()?;
            if property.unique && items.iter().map(ToString::to_string).collect::<std::collections::BTreeSet<_>>().len() != items.len() { return Err(error(attribute.span, "duplicate array item")); }
            let items = quote! { ::std::vec![#(#items),*] };
            if let Some(value) = &property.value { template(value, &BTreeMap::from([("items".into(), items)]), attribute.span) }
            else { Ok(items) }
        }
    }
}

fn expression_attribute(name: &str, expression: &super::Expression, kind: PropertyKind) -> Attribute {
    let literal = matches!(kind, PropertyKind::String | PropertyKind::Enumeration | PropertyKind::Number | PropertyKind::Integer | PropertyKind::Boolean);
    let value = match &expression.kind {
        super::ExpressionKind::String(value) | super::ExpressionKind::Number(value) if literal => AttributeValue::Literal(value.clone()),
        super::ExpressionKind::Boolean(value) if literal => AttributeValue::Literal(value.to_string()),
        _ => AttributeValue::Expression(super::ExpressionNode { source: String::new(), expression: expression.clone(), span: expression.span }),
    };
    Attribute { name: name.into(), value, span: expression.span }
}

fn error(span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(span, message, "Check the component's declared properties, events and slots")
}

pub(super) fn template(source: &str, inputs: &BTreeMap<String, TokenStream>, span: SourceSpan) -> Result<TokenStream, Diagnostic> {
    fn substitute(tokens: TokenStream, inputs: &BTreeMap<String, TokenStream>, span: SourceSpan) -> Result<TokenStream, Diagnostic> {
        let mut output = TokenStream::new();
        let mut tokens = tokens.into_iter();
        while let Some(token) = tokens.next() {
            match token {
                TokenTree::Punct(ref punct) if punct.as_char() == '$' => {
                    let Some(TokenTree::Ident(name)) = tokens.next() else { return Err(error(span, "a declaration placeholder must have a name")); };
                    output.extend(inputs.get(&name.to_string()).ok_or_else(|| error(span, format!("missing declaration input ${name}")))?.clone());
                }
                TokenTree::Group(group) => {
                    let mut replaced = Group::new(group.delimiter(), substitute(group.stream(), inputs, span)?);
                    replaced.set_span(group.span()); output.extend([TokenTree::Group(replaced)]);
                }
                token => output.extend([token]),
            }
        }
        Ok(output)
    }
    let tokens = source.parse().map_err(|e| error(span, format!("invalid component Rust expression: {e}")))?;
    let output = substitute(tokens, inputs, span)?;
    fn syntax_tokens(tokens: TokenStream) -> TokenStream {
        tokens.into_iter().filter_map(|token| match token {
            TokenTree::Ident(ref name) if name.to_string().starts_with("__uix_source_marker_") => None,
            TokenTree::Group(group) => Some(TokenTree::Group(Group::new(group.delimiter(), syntax_tokens(group.stream())))),
            token => Some(token),
        }).collect()
    }
    syn::parse2::<syn::Expr>(syntax_tokens(output.clone())).map_err(|e| error(span, format!("invalid component Rust expression {source:?}: {e}")))?;
    Ok(output)
}
