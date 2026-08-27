// 引入组件声明字段解析器。
use super::widget_parser::{parse_computed, parse_props, parse_states, validate_widget_name};
// 引入组件模板插槽声明验证器。
use super::widget_slot_parser::parse_widget_slots;
// 引入组件声明 AST、通用元素、节点联合与诊断。
use super::{
    AttributeValue, Diagnostic, Element, Node, SourceSpan, WidgetDeclaration, WidgetMemberKind,
};
// 引入名称去重集合。
use std::collections::HashSet;

// 表示从模板分拆出的四类成员块源码。
struct MemberBlockSources {
    // props 块。
    props: Option<(String, SourceSpan)>,
    // state 块。
    state: Option<(String, SourceSpan)>,
    // computed 块。
    computed: Option<(String, SourceSpan)>,
    // actions 块。
    actions: Option<(String, SourceSpan)>,
}

// 把模板子节点分拆为成员声明源码与真实视图体；重复同类块在此拒绝。
fn split_member_blocks(
    children: Vec<Node>,
) -> Result<(MemberBlockSources, Vec<Node>), Diagnostic> {
    // 保存按类别的成员块源码。
    let mut sources = MemberBlockSources {
        props: None,
        state: None,
        computed: None,
        actions: None,
    };
    // 保存剔除成员块后的视图体。
    let mut template = Vec::new();
    // 逐节点分派。
    for node in children {
        // 只拦截成员块节点。
        if let Node::WidgetMember(block) = &node {
            // 复制类别与内容后检查重复。
            let slot = match block.member {
                // props 类别。
                WidgetMemberKind::Props => &mut sources.props,
                // state 类别。
                WidgetMemberKind::State => &mut sources.state,
                // computed 类别。
                WidgetMemberKind::Computed => &mut sources.computed,
                // actions 类别。
                WidgetMemberKind::Actions => &mut sources.actions,
            };
            // 同一成员只允许一个声明块。
            if slot.is_some() {
                // 返回重复成员块诊断。
                return Err(Diagnostic::new(
                    // 指向重复块。
                    block.span,
                    // 说明重复类别。
                    format!("组件成员 {} 重复声明块", block.member.as_str()),
                    // 给出合并建议。
                    "把声明合并进同一个 @props/@state/@computed/@actions 块",
                ));
            }
            // 保存源码与跨度。
            *slot = Some((block.body.clone(), block.span));
            // 成员块不进入组件模板体。
            continue;
        }
        // 成员块之外保留原顺序。
        template.push(node);
    }
    // 返回分拆结果。
    Ok((sources, template))
}

// 合流同一成员的字符串属性形式与块级形式；双写返回定向诊断。
fn merge_member_form(
    kind: &str,
    attribute_form: Option<(String, SourceSpan)>,
    block_form: Option<(String, SourceSpan)>,
) -> Result<Option<(String, SourceSpan)>, Diagnostic> {
    // 双写冲突需要显式裁决。
    if let (Some((attribute_value, attribute_span)), Some(_)) =
        (attribute_form.as_ref(), block_form.as_ref())
    {
        // 返回形式冲突诊断；跨度字段是 Copy 值。
        return Err(Diagnostic::new(
            // 指向属性形式。
            *attribute_span,
            // 说明同类别双写。
            format!("组件成员 {kind} 同时使用了属性形式与块级形式"),
            // 给出二选一修复动作。
            format!(
                "删除 {kind}=\"{attribute_value}\" 或删除 @{kind} {{ ... }} 声明块"
            ),
        ));
    }
    // 任一存在即生效。
    Ok(attribute_form.or(block_form))
}

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
    // 保存可选 actions 声明及跨度。
    let mut actions_source = None;
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
                "使用 name、props、state、computed、actions 或 external 的字符串字面量",
            ));
        };
        // 按保留属性名保存源码。
        match attribute.name.as_str() {
            // 保存组件名。
            "name" => name = Some((value.clone(), attribute.span)),
            // 保存 props 声明。
            "props" => props_source = Some((value.clone(), attribute.span)),
            // 保存 state 声明。
            "state" => state_source = Some((value.clone(), attribute.span)),
            // 保存 computed 声明。
            "computed" => computed_source = Some((value.clone(), attribute.span)),
            // 保存同步 action 声明。
            "actions" => actions_source = Some((value.clone(), attribute.span)),
            // 保存 external 声明；external 保持字符串属性形式。
            "external" => external_source = Some((value.clone(), attribute.span)),
            // 其他属性不属于 Widget 元数据。
            _ => {
                // 返回未知属性诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明未知元数据。
                    format!("Widget 不支持属性 {}", attribute.name),
                    // 给出允许集合。
                    "只使用 name、props、state、computed、actions 与 external",
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
    // 把模板子节点分拆为成员声明块与真实视图体。
    let (block_sources, children) = split_member_blocks(element.children)?;
    // 合流字符串属性形式与块级形式的成员源码；同类别双写在此拒绝。
    let props_source = merge_member_form("props", props_source, block_sources.props)?;
    // 合流 state 成员。
    let state_source = merge_member_form("state", state_source, block_sources.state)?;
    // 合流 computed 成员。
    let computed_source = merge_member_form("computed", computed_source, block_sources.computed)?;
    // 合流 actions 成员。
    let actions_source = merge_member_form("actions", actions_source, block_sources.actions)?;
    // 解析合并后的 props 源码。
    let props = match props_source {
        // 解析存在的 props。
        Some((source, span)) => parse_props(&source, span)?,
        // 未声明 props 时使用空列表。
        None => Vec::new(),
    };
    // 解析合并后的私有 state 源码。
    let states = match state_source {
        // 解析存在的 state。
        Some((source, span)) => parse_states(&source, span)?,
        // 未声明 state 时使用空列表。
        None => Vec::new(),
    };
    // 解析合并后的有序派生表达式。
    let computed = match computed_source {
        // 解析存在的 computed。
        Some((source, span)) => parse_computed(&source, span)?,
        // 未声明 computed 时使用空列表。
        None => Vec::new(),
    };
    // 解析合并后的同步 action。
    let mut actions = match actions_source {
        // 解析存在的 actions。
        Some((source, span)) => super::action_parser::parse_actions(&source, span)?,
        // 未声明 actions 时使用空列表。
        None => Vec::new(),
    };
    // 解析可选外部符号白名单；external 只保留字符串属性形式。
    let external = match external_source {
        // 解析存在的 external。
        Some((source, span)) => parse_external(&source, span)?,
        // 未声明 external 时使用空列表。
        None => Vec::new(),
    };
    // 递归收集并验证组件模板中的默认与具名插槽。
    let slots = parse_widget_slots(&children)?;
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
    // action 与全部组件字段及 external 共享静态名称空间。
    for action in &actions {
        // 查找同名 prop、state 或 computed。
        if props.iter().any(|prop| prop.name == action.name)
            || states.iter().any(|state| state.name == action.name)
            || computed.iter().any(|derived| derived.name == action.name)
        {
            // 返回跨类别重复诊断。
            return Err(Diagnostic::new(
                // 指向 actions 声明。
                action.span,
                // 说明 action 名称冲突。
                format!("action {} 与组件字段同名", action.name),
                // 给出明确改名动作。
                "为 action 与 prop、state、computed 使用不同名称",
            ));
        }
        // action 不能遮蔽显式 Rust 外部依赖。
        if external.iter().any(|name| name == &action.name) {
            // 返回本地与外部依赖冲突诊断。
            return Err(Diagnostic::new(
                // 指向 actions 声明。
                action.span,
                // 说明 external 被遮蔽。
                format!("action {} 与 external 外部符号同名", action.name),
                // 给出明确改名动作。
                "重命名 action 或删除同名 external 声明",
            ));
        }
    }
    // 在完整名称空间建立后验证全部 action 的局部作用域与静态调用图。
    super::action_semantic::validate_widget_actions(
        &mut actions,
        &states
            .iter()
            .map(|state| state.name.clone())
            .chain(props.iter().filter_map(|prop| {
                matches!(&prop.kind, super::WidgetPropType::State(_)).then(|| prop.name.clone())
            }))
            .collect::<HashSet<_>>(),
    )?;
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
        // 保存有序同步 action。
        actions,
        // 保存已验证插槽声明。
        slots,
        // 保存外部符号白名单。
        external,
        // 转移剔除成员块后的有序组件体。
        children,
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
