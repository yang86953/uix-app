// 引入确定性属性映射。
use std::collections::BTreeMap;

// 引入组件声明、属性、诊断与调用元素 AST。
use super::{Attribute, Diagnostic, Element, WidgetDeclaration};

// 验证组件调用属性完整、唯一且无未知字段。
pub(super) fn validate_widget_attributes<'a>(
    // 接收组件调用元素。
    element: &'a Element,
    // 接收目标组件声明。
    widget: &WidgetDeclaration,
) -> Result<BTreeMap<String, &'a Attribute>, Diagnostic> {
    // 保存调用属性映射。
    let mut attributes = BTreeMap::new();
    // 按源码顺序登记属性。
    for attribute in &element.attributes {
        // 组件调用事件应通过回调 prop 传入。
        if attribute.name.starts_with('@') {
            // 返回未知事件属性诊断。
            return Err(Diagnostic::new(
                // 指向事件属性。
                attribute.span,
                // 说明事件不属于声明 props。
                format!(
                    // 拼接组件与事件名。
                    "组件调用 <{}> 不接受事件属性 {}",
                    // 写入组件名。
                    element.name,
                    // 写入事件属性名。
                    attribute.name
                ),
                // 给出回调 prop 模式。
                "通过组件声明的回调 prop 传入处理器",
            ));
        }
        // 检查同名 prop 声明。
        let known = widget
            // 遍历 props。
            .props
            // 借用迭代器。
            .iter()
            // 判断名称是否匹配。
            .any(|prop| prop.name == attribute.name);
        // 未声明属性不能静默透传。
        if !known {
            // 返回未知 prop 诊断。
            return Err(Diagnostic::new(
                // 指向未知属性。
                attribute.span,
                // 说明组件未声明输入。
                format!("<{}> 未声明 prop {}", element.name, attribute.name),
                // 给出删除或声明建议。
                "删除该属性，或把同名字段加入 Widget props",
            ));
        }
        // 拒绝重复传入同一 prop。
        if attributes
            // 按属性名写入映射。
            .insert(attribute.name.clone(), attribute)
            // 已存在时表示重复。
            .is_some()
        {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向后出现的属性。
                attribute.span,
                // 说明 prop 重复传值。
                format!("<{}> 重复传入 prop {}", element.name, attribute.name),
                // 给出唯一传值要求。
                "每个 prop 在一次组件调用中只传入一次",
            ));
        }
    }
    // 检查全部无默认值的必需 props。
    for prop in &widget.props {
        // 有默认值的 prop 可以在调用处省略。
        if prop.default.is_some() {
            // 继续检查下一 prop。
            continue;
        }
        // 缺少同名属性时返回诊断。
        if !attributes.contains_key(&prop.name) {
            // 返回缺失 prop 诊断。
            return Err(Diagnostic::new(
                // 指向完整组件调用。
                element.span,
                // 说明缺少必需输入。
                format!("<{}> 缺少必需 prop {}", element.name, prop.name),
                // 给出修复建议。
                format!("在调用处加入 {}=...", prop.name),
            ));
        }
    }
    // 返回完成验证的属性映射。
    Ok(attributes)
}
