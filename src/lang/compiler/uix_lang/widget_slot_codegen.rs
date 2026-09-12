// 引入确定性插槽投影映射。
use std::collections::BTreeMap;

// 引入组件展开器与字段绑定。
use super::widget_codegen::{Bindings, WidgetExpander};
// 引入组件声明、元素、属性值、节点与诊断 AST。
// 引入槽位归位共享的实际内容判定。
use super::widget_slot_parser::is_renderable_node;
use super::{AttributeValue, Diagnostic, Element, Node, WidgetDeclaration};

// 实现调用方插槽分组与模板占位内联。
impl WidgetExpander {
    // 在调用方作用域中展开并按插槽名分组直接子节点。
    pub(super) fn prepare_slot_projections(
        // 可变借用组件展开状态。
        &mut self,
        // 接收完整自定义组件调用。
        call: &Element,
        // 接收目标组件声明。
        widget: &WidgetDeclaration,
        // 接收调用方字段绑定。
        outer_bindings: &Bindings,
        // 标记调用是否位于 For 动态实例作用域。
        inside_for: bool,
    ) -> Result<BTreeMap<String, Vec<Node>>, Diagnostic> {
        // 为全部已声明插槽建立保持声明顺序无关的空投影。
        let mut projections = widget
            // 遍历声明插槽。
            .slots
            // 借用切片迭代器。
            .iter()
            // 默认插槽映射为空键。
            .map(|slot| (slot.name.clone().unwrap_or_default(), Vec::new()))
            // 收集确定性映射。
            .collect::<BTreeMap<_, _>>();
        // 按调用方源码顺序处理全部实际子节点。
        for child in call.children.iter().filter(|node| is_renderable_node(node)) {
            // 克隆子节点以移除只负责归位的 slot 属性。
            let mut projected = child.clone();
            // 读取直接元素子节点的可选 slot 属性。
            let key = match &mut projected {
                // 元素可以声明具名归位。
                Node::Element(element) => take_slot_attribute(element)?,
                // 文本和插值只能进入默认插槽。
                Node::Text(_) | Node::Interpolation(_) => String::new(),
                // 成员块已在声明解析阶段剥离；防御性归入默认插槽键。
                Node::WidgetMember(_) => String::new(),
            };
            // 未声明目标插槽时拒绝静默丢弃子节点。
            if !projections.contains_key(&key) {
                // 返回不存在的默认或具名插槽诊断。
                return Err(Diagnostic::new(
                    // 指向调用方直接子节点。
                    node_span(child),
                    // 区分默认与具名目标。
                    if key.is_empty() {
                        // 说明组件没有默认插槽。
                        format!("<{}> 未声明可接收子节点的默认 Slot", widget.name)
                    } else {
                        // 说明具体具名插槽不存在。
                        format!("<{}> 未声明名为 {key} 的 Slot", widget.name)
                    },
                    // 给出声明或改名修复动作。
                    "在组件模板中声明对应 <Slot />，或修改子节点的 slot 名称",
                ));
            }
            // 在进入被调用组件作用域前完成调用方内容展开。
            let expanded = self.expand_nodes(
                // 单节点切片保持通用展开顺序。
                std::slice::from_ref(&projected),
                // 使用调用方 props/state 绑定。
                outer_bindings,
                // 沿用调用点 For 约束。
                inside_for,
            )?;
            // 追加到同名插槽的源码顺序尾部。
            projections
                // 目标存在性已在上方确认。
                .get_mut(&key)
                // 防御性断言映射未被并发修改。
                .expect("插槽投影目标已确认存在")
                // 追加可能由组件展开产生的多个同层节点。
                .extend(expanded);
        }
        // 返回全部调用方作用域投影。
        Ok(projections)
    }

    // 把模板 Slot 占位替换为已经展开的调用方节点。
    pub(super) fn expand_slot_placeholder(
        // 只读借用当前展开状态。
        &self,
        // 接收已在声明阶段验证的 Slot 元素。
        slot: &Element,
    ) -> Result<Vec<Node>, Diagnostic> {
        // Slot name 已由声明解析器保证为可选字面量。
        let key = slot
            // 读取可选首属性。
            .attributes
            // 借用属性迭代器。
            .first()
            // 提取具名字符串字面量。
            .and_then(|attribute| match &attribute.value {
                // 返回具名键。
                AttributeValue::Literal(value) => Some(value.clone()),
                // 声明验证保证不会出现其他形状。
                _ => None,
            })
            // 没有 name 时使用默认空键。
            .unwrap_or_default();
        // Slot 只能在正在展开的组件模板中出现。
        let Some(projections) = self.slot_projection_stack.last() else {
            // 返回组件外占位诊断。
            return Err(Diagnostic::new(
                // 指向非法 Slot。
                slot.span,
                // 说明缺少组件调用投影上下文。
                "<Slot> 只能出现在 Widget 模板中",
                // 给出合法结构。
                "把 <Slot /> 移入顶层 <Widget> 声明体",
            ));
        };
        // 返回该占位对应的已展开调用方内容；无内容时为空。
        Ok(projections.get(&key).cloned().unwrap_or_default())
    }
}

// 读取并移除调用方直接子元素的 slot 归位属性。
fn take_slot_attribute(element: &mut Element) -> Result<String, Diagnostic> {
    // 保存已发现属性的索引与值。
    let mut found = None;
    // 扫描直接子元素属性。
    for (index, attribute) in element.attributes.iter().enumerate() {
        // 只处理保留归位属性。
        if attribute.name != "slot" {
            // 其他属性由目标元素正常消费。
            continue;
        }
        // 同一子节点只能声明一次归位目标。
        if found.is_some() {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向后出现的 slot 属性。
                attribute.span,
                // 说明归位目标不唯一。
                "子节点 slot 属性重复声明",
                // 给出唯一属性修复动作。
                "只保留一个 slot=\"名称\" 属性",
            ));
        }
        // slot 只接受非空字符串字面量。
        let AttributeValue::Literal(value) = &attribute.value else {
            // 返回动态名称拒绝诊断。
            return Err(Diagnostic::new(
                // 指向非法 slot 属性。
                attribute.span,
                // 说明归位发生在编译期。
                "子节点 slot 属性必须是字符串字面量",
                // 给出规范写法。
                "使用 slot=\"footer\"",
            ));
        };
        // 空名称不能伪装成默认插槽。
        if value.trim().is_empty() {
            // 返回空名称诊断。
            return Err(Diagnostic::new(
                // 指向空 slot 属性。
                attribute.span,
                // 说明默认插槽不需要属性。
                "子节点 slot 名称不能为空",
                // 给出默认插槽写法。
                "删除 slot 属性以进入默认 Slot",
            ));
        }
        // 保存待移除属性索引与名称。
        found = Some((index, value.clone()));
    }
    // 没有 slot 属性时进入默认插槽。
    let Some((index, key)) = found else {
        // 返回默认空键。
        return Ok(String::new());
    };
    // 删除仅供组件投影使用的属性，避免泄漏到目标元素生成器。
    element.attributes.remove(index);
    // 返回具名投影键。
    Ok(key)
}

// 判断调用方节点是否包含实际可投影内容。

// 取得任意调用方子节点的诊断跨度。
fn node_span(node: &Node) -> super::SourceSpan {
    // 按节点形状返回原始跨度。
    match node {
        // 元素使用完整标签跨度。
        Node::Element(element) => element.span,
        // 文本使用原始文本跨度。
        Node::Text(text) => text.span,
        // 插值使用完整花括号跨度。
        Node::Interpolation(expression) => expression.span,
        // 成员块保留完整声明跨度。
        Node::WidgetMember(block) => block.span,
    }
}
