// 引入插槽声明、通用节点与诊断 AST。
use super::{AttributeValue, Diagnostic, Node, WidgetSlot};
// 引入确定性名称去重集合。
use std::collections::BTreeSet;

// 递归收集并验证组件模板中的 Slot 占位。
pub(super) fn parse_widget_slots(nodes: &[Node]) -> Result<Vec<WidgetSlot>, Diagnostic> {
    // 保存源码顺序中的插槽声明。
    let mut slots = Vec::new();
    // 保存默认空键与具名键以拒绝重复占位。
    let mut names = BTreeSet::new();
    // 递归扫描完整组件模板。
    collect_slots(nodes, &mut slots, &mut names)?;
    // 返回已经完成唯一性验证的声明表。
    Ok(slots)
}

// 递归扫描一组模板节点。
fn collect_slots(
    // 接收当前层源码节点。
    nodes: &[Node],
    // 累积源码顺序中的插槽声明。
    slots: &mut Vec<WidgetSlot>,
    // 累积已经出现的默认或具名键。
    names: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    // 按源码顺序检查全部元素节点。
    for node in nodes {
        // 文本与插值不可能声明插槽。
        let Node::Element(element) = node else {
            // 跳过非元素节点。
            continue;
        };
        // 普通元素继续递归扫描其模板子树。
        if element.name != "Slot" {
            // 扫描嵌套布局或组件调用中的占位。
            collect_slots(&element.children, slots, names)?;
            // 当前普通元素处理完成。
            continue;
        }
        // Slot 只能包含可选的唯一 name 属性。
        if element.attributes.len() > 1
            // 单属性也必须名为 name。
            || element
                // 读取可能存在的首个属性。
                .attributes
                // 检查属性名。
                .first()
                // 未知属性视为非法。
                .is_some_and(|attribute| attribute.name != "name")
        {
            // 返回闭合属性集合诊断。
            return Err(Diagnostic::new(
                // 指向完整 Slot 占位。
                element.span,
                // 说明只允许 name。
                "<Slot> 只支持可选的 name 属性",
                // 给出默认与具名规范写法。
                "使用 <Slot /> 或 <Slot name=\"footer\" />",
            ));
        }
        // 解析默认或具名插槽名称。
        let name = match element.attributes.first() {
            // 没有属性表示默认插槽。
            None => None,
            // name 必须是非空字符串字面量。
            Some(attribute) => match &attribute.value {
                // 接收非空具名插槽。
                AttributeValue::Literal(value) if !value.trim().is_empty() => {
                    // 保留作者名称原文。
                    Some(value.clone())
                }
                // 拒绝表达式、内联样式或空名称。
                _ => {
                    // 返回属性形状诊断。
                    return Err(Diagnostic::new(
                        // 指向非法 name 属性。
                        attribute.span,
                        // 说明具名插槽名称约束。
                        "Slot name 必须是非空字符串字面量",
                        // 给出规范具名示例。
                        "使用 <Slot name=\"footer\" />",
                    ));
                }
            },
        };
        // Slot 占位不能携带实际内容。
        if element.children.iter().any(is_renderable_node) {
            // 返回自闭合形状诊断。
            return Err(Diagnostic::new(
                // 指向完整 Slot 元素。
                element.span,
                // 说明占位不拥有模板内容。
                "<Slot> 不能包含子节点",
                // 给出自闭合写法。
                "使用自闭合占位 <Slot />",
            ));
        }
        // 默认插槽使用空字符串参与统一去重。
        let key = name.clone().unwrap_or_default();
        // 每个名称最多声明一个占位。
        if !names.insert(key.clone()) {
            // 返回重复默认或具名插槽诊断。
            return Err(Diagnostic::new(
                // 指向后出现的重复占位。
                element.span,
                // 区分默认与具名错误消息。
                if key.is_empty() {
                    // 说明默认插槽重复。
                    "组件模板重复声明默认 Slot".to_string()
                } else {
                    // 说明具体具名插槽重复。
                    format!("组件模板重复声明 Slot {key}")
                },
                // 给出唯一性修复动作。
                "删除重复占位，或为具名 Slot 使用不同 name",
            ));
        }
        // 保存完成验证的插槽声明。
        slots.push(WidgetSlot {
            // 保存默认或具名事实。
            name,
            // 保存占位源码跨度。
            span: element.span,
        });
    }
    // 报告当前模板子树扫描成功。
    Ok(())
}

// 判断节点是否包含实际可投影内容。
// 默认与具名槽位共用的实际内容判定；元素、插值可渲染，空白文本与成员块不可。
pub(super) fn is_renderable_node(node: &Node) -> bool {
    // 元素与插值始终可渲染。
    match node {
        // 任意元素属于实际内容。
        Node::Element(_) | Node::Interpolation(_) => true,
        // 纯格式化空白文本不属于实际内容。
        Node::Text(text) => !text.value.trim().is_empty(),
        // 成员块是声明载体，不构成插槽内容。
        Node::WidgetMember(_) => false,
    }
}
