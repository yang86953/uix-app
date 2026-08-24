// 解析顶层 <Visual> 静态视觉记录声明。

use std::collections::HashSet;

use proc_macro2::Ident;

use super::{AttributeValue, Diagnostic, Element, VisualDeclaration, VisualField, VisualValue};

// 把通用顶层元素验证为由 UIX 唯一拥有的静态视觉记录。
pub(crate) fn parse_visual_declaration(element: Element) -> Result<VisualDeclaration, Diagnostic> {
    if element.name != "Visual" {
        return Err(Diagnostic::new(
            element.span,
            "视觉声明解析器只接受 <Visual>",
            "把普通元素交给 View 解析器",
        ));
    }
    if !element.children.is_empty() {
        return Err(Diagnostic::new(
            element.span,
            "<Visual> 不接受子节点",
            "把具名静态值写为 Visual 属性并使用自闭合标签",
        ));
    }

    let mut attribute_names = HashSet::new();
    let mut rust_field_names = HashSet::new();
    let mut name = None;
    let mut rust_type = None;
    let mut fields = Vec::new();

    for attribute in element.attributes {
        if !attribute_names.insert(attribute.name.clone()) {
            return Err(Diagnostic::new(
                attribute.span,
                format!("Visual 属性 {} 重复声明", attribute.name),
                "合并重复属性并只保留一次",
            ));
        }
        match attribute.name.as_str() {
            "name" => {
                let AttributeValue::Literal(value) = attribute.value else {
                    return Err(Diagnostic::new(
                        attribute.span,
                        "Visual name 必须使用字符串字面量",
                        "使用 name=\"ICON_VISUAL\"",
                    ));
                };
                validate_visual_name(&value, attribute.span)?;
                name = Some(value);
            }
            "type" => {
                let AttributeValue::Literal(value) = attribute.value else {
                    return Err(Diagnostic::new(
                        attribute.span,
                        "Visual type 必须使用字符串字面量",
                        "使用 type=\"IconVisual\"",
                    ));
                };
                validate_visual_type(&value, attribute.span)?;
                rust_type = Some(value);
            }
            field_name => {
                validate_visual_field_name(field_name, attribute.span)?;
                let rust_name = visual_field_rust_name(field_name);
                if !rust_field_names.insert(rust_name.clone()) {
                    return Err(Diagnostic::new(
                        attribute.span,
                        format!("Visual 字段 {field_name} 映射到重复 Rust 字段 {rust_name}"),
                        "合并同义 camelCase/snake_case 字段或使用不同名称",
                    ));
                }
                let value = match attribute.value {
                    AttributeValue::Literal(value) => VisualValue::Literal(value),
                    AttributeValue::Expression(value) => VisualValue::Expression(value.expression),
                    AttributeValue::InlineStyle(_) => {
                        return Err(Diagnostic::new(
                            attribute.span,
                            format!("Visual 字段 {field_name} 不接受内联 style"),
                            "使用字符串字面量或花括号常量表达式",
                        ));
                    }
                };
                fields.push(VisualField {
                    source_name: field_name.to_string(),
                    rust_name,
                    value,
                    span: attribute.span,
                });
            }
        }
    }

    let name = name.ok_or_else(|| {
        Diagnostic::new(
            element.span,
            "<Visual> 缺少必需的 name 属性",
            "使用 name=\"ICON_VISUAL\" 声明模块级常量名",
        )
    })?;
    let rust_type = rust_type.ok_or_else(|| {
        Diagnostic::new(
            element.span,
            "<Visual> 缺少必需的 type 属性",
            "使用 type=\"IconVisual\" 绑定同模块 Rust 视觉结构",
        )
    })?;
    if fields.is_empty() {
        return Err(Diagnostic::new(
            element.span,
            "<Visual> 至少需要一个具名视觉字段",
            "添加 defaultSize={24} 等静态视觉属性",
        ));
    }

    Ok(VisualDeclaration {
        name,
        rust_type,
        fields,
        span: element.span,
    })
}

// Visual 常量使用稳定的 SCREAMING_SNAKE_CASE Rust 标识符。
fn validate_visual_name(name: &str, span: super::SourceSpan) -> Result<(), Diagnostic> {
    let valid_shape = !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|value| value.is_ascii_uppercase() || value == '_')
        && name
            .chars()
            .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit() || value == '_')
        && syn::parse_str::<Ident>(name).is_ok();
    if valid_shape {
        Ok(())
    } else {
        Err(Diagnostic::new(
            span,
            format!("Visual 名称 {name:?} 不是合法的 SCREAMING_SNAKE_CASE 标识符"),
            "使用 ICON_VISUAL 或 ALERT_LAYOUT 等常量名称",
        ))
    }
}

// Visual 类型限定为同模块 PascalCase Rust 结构名称，避免隐藏路径依赖。
fn validate_visual_type(name: &str, span: super::SourceSpan) -> Result<(), Diagnostic> {
    let valid_shape = name
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_uppercase())
        && name
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '_')
        && syn::parse_str::<Ident>(name).is_ok();
    if valid_shape {
        Ok(())
    } else {
        Err(Diagnostic::new(
            span,
            format!("Visual type {name:?} 必须是同模块 PascalCase Rust 类型"),
            "使用 IconVisual 或 AlertLayoutVisual 等结构名称",
        ))
    }
}

// 字段遵循 UIX camelCase 或 Rust snake_case，并在生成前统一归一化。
fn validate_visual_field_name(name: &str, span: super::SourceSpan) -> Result<(), Diagnostic> {
    let valid_shape = name
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_lowercase() || value == '_')
        && name
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '_');
    if valid_shape {
        Ok(())
    } else {
        Err(Diagnostic::new(
            span,
            format!("Visual 字段 {name:?} 必须使用 camelCase 或 snake_case"),
            "使用 defaultSize 或 default_size 等具名字段",
        ))
    }
}

// 把 UIX camelCase 属性确定性映射为 Rust snake_case 字段。
fn visual_field_rust_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len() + 4);
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            if !output.is_empty() && !output.ends_with('_') {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}
