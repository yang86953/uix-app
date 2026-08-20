// 引入组件声明字段解析器。
use super::widget_parser::{parse_computed, parse_props, parse_states, validate_widget_name};
// 引入组件模板插槽声明验证器。
use super::widget_slot_parser::parse_widget_slots;
// 引入组件声明 AST、通用元素与诊断。
use super::{AttributeValue, Diagnostic, Element, SourceSpan, WidgetDeclaration};
// 引入名称去重集合。
use std::collections::HashSet;

// 把通用顶层 Widget 元素验证为结构化组件声明。
pub(crate) fn parse_widget_declaration(
    // 接收已经完成标签级解析的 Widget。
    element: Element,
) -> Result<WidgetDeclaration, Diagnostic> {
    // 防御性检查调用方只传入保留标签。
    if element.name != "Widget" {
        // 返回内部路由诊断。
        return Err(Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明元素类型不匹配。
            "组件声明解析器只接受 <Widget>",
            // 给出正确路由。
            "把普通元素交给 View 解析器",
        ));
    }
    // 保存已出现的 Widget 属性名。
    let mut attribute_names = HashSet::new();
    // 保存必需组件名。
    let mut name = None;
    // 保存可选 props 声明及跨度。
    let mut props_source = None;
    // 保存可选 state 声明及跨度。
    let mut state_source = None;
    // 保存可选 computed 声明及跨度。
    let mut computed_source = None;
    // 保存可选 external 声明及跨度。
    let mut external_source = None;
    // 验证 Widget 只包含已登记声明属性。
    for attribute in &element.attributes {
        // 拒绝重复属性。
        if !attribute_names.insert(attribute.name.as_str()) {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向重复属性。
                attribute.span,
                // 说明重复名称。
                format!("Widget 属性 {} 重复声明", attribute.name),
                // 给出修复动作。
                "合并重复属性并只保留一次",
            ));
        }
        // Widget 元数据必须使用字符串字面量。
        let AttributeValue::Literal(value) = &attribute.value else {
            // 返回元数据值形状诊断。
            return Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明不接受运行期表达式。
                format!("Widget {} 必须使用字符串字面量", attribute.name),
                // 给出规范形式。
                "使用 name、props、state、computed 或 external 的字符串字面量",
            ));
        };
        // 按保留属性名保存源码。
        match attribute.name.as_str() {
            // 保存组件名。
            "name" => name = Some((value.clone(), attribute.span)),
            // 保存 props 声明。
            "props" => props_source = Some((value.as_str(), attribute.span)),
            // 保存 state 声明。
            "state" => state_source = Some((value.as_str(), attribute.span)),
            // 保存 computed 声明。
            "computed" => computed_source = Some((value.as_str(), attribute.span)),
            // 保存 external 声明。
            "external" => external_source = Some((value.as_str(), attribute.span)),
            // 其他属性不属于 Widget 元数据。
            _ => {
                // 返回未知属性诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明未知元数据。
                    format!("Widget 不支持属性 {}", attribute.name),
                    // 给出允许集合。
                    "只使用 name、props、state、computed 与 external",
                ));
            }
        }
    }
    // name 是组件声明的必需属性。
    let Some((name, name_span)) = name else {
        // 返回缺失名称诊断。
        return Err(Diagnostic::new(
            // 指向完整组件。
            element.span,
            // 说明缺少必需名称。
            "<Widget> 缺少必需的 name 属性",
            // 给出规范示例。
            "使用 <Widget name=\"Counter\">...</Widget>",
        ));
    };
    // 组件名必须可映射为 PascalCase Rust 标识符。
    validate_widget_name(&name, name_span)?;
    // 解析可选 props 字符串。
    let props = match props_source {
        // 解析存在的 props。
        Some((source, span)) => parse_props(source, span)?,
        // 未声明 props 时使用空列表。
        None => Vec::new(),
    };
    // 解析可选私有 state 字符串。
    let states = match state_source {
        // 解析存在的 state。
        Some((source, span)) => parse_states(source, span)?,
        // 未声明 state 时使用空列表。
        None => Vec::new(),
    };
    // 解析可选有序派生表达式。
    let computed = match computed_source {
        // 解析存在的 computed。
        Some((source, span)) => parse_computed(source, span)?,
        // 未声明 computed 时使用空列表。
        None => Vec::new(),
    };
    // 解析可选外部符号白名单。
    let external = match external_source {
        // 解析存在的 external。
        Some((source, span)) => parse_external(source, span)?,
        // 未声明 external 时使用空列表。
        None => Vec::new(),
    };
    // 递归收集并验证组件模板中的默认与具名插槽。
    let slots = parse_widget_slots(&element.children)?;
    // props 与 state 共享组件体标识符命名空间。
    for state in &states {
        // 查找同名 prop。
        if props.iter().any(|prop| prop.name == state.name) {
            // 返回跨类别重复诊断。
            return Err(Diagnostic::new(
                // 指向 state 声明。
                state.span,
                // 说明名称冲突。
                format!("组件字段 {} 同时声明为 prop 与 state", state.name),
                // 给出改名建议。
                "为 prop 与私有 state 使用不同名称",
            ));
        }
    }
    // computed 与 props/state 共享组件体标识符命名空间。
    for derived in &computed {
        // 查找同名 prop 或私有 state。
        if props.iter().any(|prop| prop.name == derived.name)
            || states.iter().any(|state| state.name == derived.name)
        {
            // 返回跨类别重复诊断。
            return Err(Diagnostic::new(
                // 指向 computed 声明。
                derived.span,
                // 说明名称冲突。
                format!("组件字段 {} 与 computed 派生值同名", derived.name),
                // 给出改名建议。
                "为 prop、state 与 computed 使用不同名称",
            ));
        }
        // computed 不能遮蔽显式 Rust 外部依赖。
        if external.iter().any(|name| name == &derived.name) {
            // 返回本地与外部依赖冲突诊断。
            return Err(Diagnostic::new(
                // 指向 computed 声明。
                derived.span,
                // 说明 external 被遮蔽。
                format!("computed {} 与 external 外部符号同名", derived.name),
                // 给出明确改名动作。
                "重命名 computed 或删除同名 external 声明",
            ));
        }
    }
    // 返回结构化组件声明。
    Ok(WidgetDeclaration {
        // 保存组件名。
        name,
        // 保存 props。
        props,
        // 保存 states。
        states,
        // 保存有序 computed 派生值。
        computed,
        // 保存已验证插槽声明。
        slots,
        // 保存外部符号白名单。
        external,
        // 转移有序组件体。
        children: element.children,
        // 保存完整声明跨度。
        span: element.span,
    })
}

// 解析逗号分隔的外部符号白名单。
fn parse_external(source: &str, span: SourceSpan) -> Result<Vec<String>, Diagnostic> {
    // 空字符串表示组件不依赖 Rust 外部符号。
    if source.trim().is_empty() {
        // 返回空白名单。
        return Ok(Vec::new());
    }
    // 保存源码顺序中的外部符号。
    let mut external = Vec::new();
    // 保存已出现名称以拒绝重复声明。
    let mut names = HashSet::new();
    // 按逗号切分独立 Rust 标识符。
    for entry in source.split(',') {
        // 去除名称周围空白。
        let name = entry.trim();
        // 外部名称必须是普通非关键字 Rust 标识符。
        if name.is_empty() || syn::parse_str::<syn::Ident>(name).is_err() {
            // 返回精确属性诊断。
            return Err(Diagnostic::new(
                // 指向 external 属性。
                span,
                // 说明非法名称。
                format!("external 名称 {name:?} 不是合法 Rust 标识符"),
                // 给出规范形式。
                "使用逗号分隔的 Rust 标识符，例如 external=\"debounce, format\"",
            ));
        }
        // 相同外部依赖只允许声明一次。
        if !names.insert(name) {
            // 返回重复声明诊断。
            return Err(Diagnostic::new(
                // 指向 external 属性。
                span,
                // 说明重复名称。
                format!("external 符号 {name} 重复声明"),
                // 给出修复动作。
                "删除重复名称并只保留一次",
            ));
        }
        // 保存已验证名称。
        external.push(name.to_string());
    }
    // 返回确定的源码顺序白名单。
    Ok(external)
}
