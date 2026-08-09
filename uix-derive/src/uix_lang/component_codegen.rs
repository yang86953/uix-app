// 引入确定性组件与字段映射。
use std::collections::BTreeMap;

// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件、文档、表达式与视图 AST。
use super::{
    Attribute, AttributeValue, ComponentDeclaration, ControlBinding, Declaration, Diagnostic,
    Document, Element, Node,
};
// 引入既有核心 View 生成入口。
use super::generate_view;

// 保存组件字段展开后的 Rust 局部绑定。
#[derive(Clone)]
pub(super) struct Binding {
    // 保存读取字段值时使用的卫生名称。
    pub(super) value_name: String,
    // 保存可由 setState 写入的 State 句柄名称。
    pub(super) state_name: Option<String>,
    // 保存值、回调或响应式状态类别。
    pub(super) kind: BindingKind,
}

// 区分不同克隆与传参语义的组件字段。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BindingKind {
    // 表示普通基础值 prop。
    Value,
    // 表示已类型化且可克隆的回调 prop。
    Callback,
    // 表示私有或共享响应式状态。
    State,
}

// 使用有序映射保持生成令牌稳定。
pub(super) type Bindings = BTreeMap<String, Binding>;

// 把完整文档中的自定义组件展开为现有核心 View 代码。
pub(crate) fn generate_document_view(document: &Document) -> Result<TokenStream, Diagnostic> {
    // 创建组件感知展开器。
    let mut expander = ComponentExpander::new(document);
    // 展开根元素与全部组件调用。
    let root = expander.expand_root(&document.root)?;
    // 委托核心映射生成 ViewNode。
    let view = generate_view(&root)?;
    // 取出按依赖顺序生成的局部准备语句。
    let setup = expander.setup;
    // 返回不向调用模块泄漏名称的单一表达式。
    Ok(quote! {{
        // 先创建 props、状态读值、回调适配器与更新器。
        #(#setup)*
        // 最后构建只含核心元素的 View。
        #view
    }})
}

// 保存一次文档展开所需的确定性状态。
pub(super) struct ComponentExpander {
    // 保存名称到组件声明的完整副本。
    pub(super) components: BTreeMap<String, ComponentDeclaration>,
    // 保存最终 View 之前执行的有序准备语句。
    pub(super) setup: Vec<TokenStream>,
    // 保存卫生名称的单调递增编号。
    pub(super) next_id: usize,
    // 保存当前组件展开栈以拒绝递归定义。
    pub(super) stack: Vec<String>,
}

// 实现文档级组件展开与结构校验。
impl ComponentExpander {
    // 从顶层声明构造组件注册表。
    fn new(document: &Document) -> Self {
        // 收集全部已完成名称去重验证的组件声明。
        let components = document
            // 遍历顶层声明。
            .declarations
            // 借用声明迭代器。
            .iter()
            // 只保留组件声明。
            .filter_map(|declaration| match declaration {
                // 复制组件名称与声明。
                Declaration::Component(component) => {
                    // 返回映射条目。
                    Some((component.name.clone(), component.clone()))
                }
                // 其他声明由后续 Gate 处理。
                _ => None,
            })
            // 收集到有序映射。
            .collect();
        // 返回初始展开状态。
        Self {
            // 写入组件注册表。
            components,
            // 初始没有准备语句。
            setup: Vec::new(),
            // 卫生编号从零开始。
            next_id: 0,
            // 初始不在任何组件体内。
            stack: Vec::new(),
        }
    }

    // 展开根元素，并为多根组件体补充 Column。
    fn expand_root(&mut self, root: &Element) -> Result<Element, Diagnostic> {
        // 根作用域没有组件字段。
        let bindings = Bindings::new();
        // 按普通节点规则展开根元素。
        let nodes = self.expand_element(root, &bindings, false)?;
        // 排除只承担排版作用的空白文本。
        let mut renderable = nodes
            // 转成拥有所有权的迭代器。
            .into_iter()
            // 保留可渲染节点。
            .filter(is_renderable_node)
            // 收集展开结果。
            .collect::<Vec<_>>();
        // 单一元素可以直接作为文档根。
        if renderable.len() == 1 {
            // 取出唯一节点。
            let node = renderable.pop().expect("长度已确认恰好为一");
            // 元素节点直接满足根契约。
            if let Node::Element(element) = node {
                // 返回唯一元素根。
                return Ok(element);
            }
            // 文本或插值根需要容器承载。
            renderable.push(node);
        }
        // 空组件体不能形成有效 View。
        if renderable.is_empty() {
            // 返回空视图诊断。
            return Err(Diagnostic::new(
                // 指向根调用位置。
                root.span,
                // 说明没有可渲染内容。
                "根组件展开后没有可渲染节点",
                // 给出最小修复动作。
                "在组件体中加入 Text、Button、Container 或其他 View 元素",
            ));
        }
        // 多节点组件以 Column 保持顺序并满足单根契约。
        Ok(Element {
            // 使用已登记的核心列容器。
            name: "Column".to_string(),
            // 合成容器没有额外属性。
            attributes: Vec::new(),
            // 保存全部展开节点。
            children: renderable,
            // 合成容器不是控制元素。
            control: None,
            // 沿用调用跨度。
            span: root.span,
        })
    }

    // 展开一个元素为一个或多个同层节点。
    pub(super) fn expand_element(
        // 可变借用展开状态。
        &mut self,
        // 接收待展开元素。
        element: &Element,
        // 接收当前组件字段绑定。
        bindings: &Bindings,
        // 标记是否位于 For 动态实例作用域。
        inside_for: bool,
    ) -> Result<Vec<Node>, Diagnostic> {
        // 自定义标签交给组件调用展开。
        if self.components.contains_key(&element.name) {
            // 返回组件展开节点。
            return self.expand_component_call(element, bindings, inside_for);
        }
        // 克隆普通核心或未知元素。
        let mut expanded = element.clone();
        // 逐个改写普通属性与事件表达式。
        for attribute in &mut expanded.attributes {
            // 只有表达式属性需要字段改写。
            if let AttributeValue::Expression(expression) = &mut attribute.value {
                // 事件为字段创建独立克隆。
                let active_bindings = if attribute.name.starts_with('@') {
                    // 生成当前事件专用绑定。
                    self.clone_event_bindings(bindings)
                } else {
                    // 普通表达式直接使用当前绑定。
                    bindings.clone()
                };
                // 只有事件属性允许 setState。
                let allow_set_state = attribute.name.starts_with('@');
                // 改写 props、state 与 setState。
                self.transform_expression(
                    // 可变借用表达式树。
                    &mut expression.expression,
                    // 使用当前属性的字段绑定。
                    &active_bindings,
                    // 传递状态更新作用域。
                    allow_set_state,
                )?;
            }
        }
        // 改写 If 或 For 控制表达式。
        if let Some(control) = &mut expanded.control {
            // 按控制绑定形状改写。
            match control {
                // 改写 If 条件。
                ControlBinding::If(condition) => self.transform_expression(
                    // 可变借用条件表达式。
                    &mut condition.expression,
                    // 使用当前字段绑定。
                    bindings,
                    // 条件不能更新状态。
                    false,
                )?,
                // 改写 For 数据源与可选 key。
                ControlBinding::For { iterable, key, .. } => {
                    // 改写循环数据源。
                    self.transform_expression(
                        // 可变借用数据源表达式。
                        &mut iterable.expression,
                        // 使用当前字段绑定。
                        bindings,
                        // 数据源不能更新状态。
                        false,
                    )?;
                    // 存在 key 时同步改写。
                    if let Some(key) = key {
                        // 改写稳定身份表达式。
                        self.transform_expression(
                            // 可变借用 key 表达式。
                            &mut key.expression,
                            // 使用当前字段绑定。
                            bindings,
                            // key 不能更新状态。
                            false,
                        )?;
                    }
                }
            }
        }
        // For 的直接子树进入动态实例作用域。
        let child_inside_for = inside_for || element.name == "For";
        // 展开全部有序子节点。
        expanded.children = self.expand_nodes(&element.children, bindings, child_inside_for)?;
        // 返回单一普通元素节点。
        Ok(vec![Node::Element(expanded)])
    }

    // 展开有序节点序列并允许组件体在父层级中展开。
    pub(super) fn expand_nodes(
        // 可变借用展开状态。
        &mut self,
        // 接收源码顺序节点。
        nodes: &[Node],
        // 接收当前组件字段绑定。
        bindings: &Bindings,
        // 标记是否位于 For 动态实例作用域。
        inside_for: bool,
    ) -> Result<Vec<Node>, Diagnostic> {
        // 保存展开后的有序节点。
        let mut expanded = Vec::new();
        // 按源码顺序遍历节点。
        for node in nodes {
            // 按节点形状展开或改写。
            match node {
                // 元素可能展开为多个同层节点。
                Node::Element(element) => {
                    // 追加元素展开结果。
                    expanded.extend(self.expand_element(element, bindings, inside_for)?);
                }
                // 普通文本无需名称改写。
                Node::Text(text) => {
                    // 保留文本与跨度。
                    expanded.push(Node::Text(text.clone()));
                }
                // 插值需要改写组件字段读取。
                Node::Interpolation(expression) => {
                    // 克隆插值节点。
                    let mut expression = expression.clone();
                    // 改写插值表达式。
                    self.transform_expression(
                        // 可变借用表达式树。
                        &mut expression.expression,
                        // 使用当前字段绑定。
                        bindings,
                        // 插值不能更新状态。
                        false,
                    )?;
                    // 保存改写后的插值。
                    expanded.push(Node::Interpolation(expression));
                }
            }
        }
        // 返回保持源码顺序的节点列表。
        Ok(expanded)
    }

    // 展开一个已登记自定义组件调用。
    fn expand_component_call(
        // 可变借用展开状态。
        &mut self,
        // 接收组件调用元素。
        element: &Element,
        // 接收调用方字段绑定。
        outer_bindings: &Bindings,
        // 标记调用是否位于 For 作用域。
        inside_for: bool,
    ) -> Result<Vec<Node>, Diagnostic> {
        // 复制声明以允许递归展开。
        let component = self
            // 按标签名称查找组件。
            .components
            // 读取已确认存在的声明。
            .get(&element.name)
            // 复制完整声明。
            .cloned()
            // 注册表检查保证存在。
            .expect("组件存在性已在调用前确认");
        // For 内的状态或 prop 需要运行时逐实例存储。
        if inside_for && (!component.props.is_empty() || !component.states.is_empty()) {
            // 返回明确的动态实例边界诊断。
            return Err(Diagnostic::new(
                // 指向组件调用。
                element.span,
                // 说明缺少逐实例存储。
                format!("For 内的 <{}> 需要逐实例 props/state 存储", element.name),
                // 给出不共享状态的修复建议。
                "把组件移到 For 外，或直接展开无状态无 props 的行内容",
            ));
        }
        // 拒绝直接或间接递归组件。
        if self.stack.contains(&component.name) {
            // 构造包含闭环的调用路径。
            let mut cycle = self.stack.clone();
            // 追加再次进入的组件。
            cycle.push(component.name.clone());
            // 返回递归诊断。
            return Err(Diagnostic::new(
                // 指向形成闭环的调用。
                element.span,
                // 展示调用路径。
                format!("组件递归调用不受支持：{}", cycle.join(" -> ")),
                // 给出拆环建议。
                "移除自调用，或把递归数据改为 For 迭代的有限 View",
            ));
        }
        // 当前组件语法没有默认 slot。
        if element.children.iter().any(is_renderable_node) {
            // 返回未声明 slot 的诊断。
            return Err(Diagnostic::new(
                // 指向完整调用。
                element.span,
                // 说明子节点不会被静默丢弃。
                format!("<{}> 未声明可接收子节点的 slot", element.name),
                // 给出当前合法形状。
                format!("使用自闭合调用 <{} ... />", element.name),
            ));
        }
        // 验证调用属性与 props 一一对应。
        let attributes = validate_component_attributes(element, &component)?;
        // 进入当前组件展开栈。
        self.stack.push(component.name.clone());
        // 在闭包内展开以确保错误路径也弹栈。
        let result = (|| {
            // 组件体只看见自身字段。
            let mut bindings = Bindings::new();
            // 按声明顺序生成 props。
            for prop in &component.props {
                // 读取已经验证存在的属性。
                let attribute = attributes
                    // 按 prop 名称查找。
                    .get(&prop.name)
                    // 复制属性借用。
                    .copied()
                    // 完整性验证保证存在。
                    .expect("必需 prop 已完成存在性验证");
                // 生成类型化 prop 绑定。
                self.emit_prop_binding(prop, attribute, outer_bindings, &mut bindings)?;
            }
            // 按声明顺序生成私有状态。
            for state in &component.states {
                // 生成 State 句柄与读值。
                self.emit_private_state(state, &mut bindings)?;
            }
            // 展开组件体与嵌套组件。
            self.expand_nodes(&component.children, &bindings, false)
        })();
        // 离开当前组件展开栈。
        self.stack.pop();
        // 返回展开结果或诊断。
        result
    }

    // 生成卫生标识符并推进单调编号。
    pub(super) fn fresh_ident(&mut self, role: &str, source_name: &str) -> Ident {
        // 取出当前编号。
        let id = self.next_id;
        // 推进下一编号。
        self.next_id += 1;
        // 规范化可读名称片段。
        let safe_name = source_name
            // 遍历源码字符。
            .chars()
            // 替换非 ASCII 字母数字。
            .map(|character| {
                // 保留安全字符。
                if character.is_ascii_alphanumeric() {
                    // 返回原字符。
                    character
                } else {
                    // 返回下划线。
                    '_'
                }
            })
            // 收集名称片段。
            .collect::<String>();
        // 构造不会与用户名称重叠的标识符。
        let name = format!("__uix_{role}_{id}_{safe_name}");
        // 使用调用点跨度，使表达式 AST 重建的同名标识符可解析到该绑定。
        Ident::new(&name, Span::call_site())
    }
}

// 验证组件调用属性完整、唯一且无未知字段。
fn validate_component_attributes<'a>(
    // 接收组件调用元素。
    element: &'a Element,
    // 接收目标组件声明。
    component: &ComponentDeclaration,
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
                    "组件调用 <{}> 不接受事件属性 {}",
                    element.name, attribute.name
                ),
                // 给出回调 prop 模式。
                "通过组件声明的回调 prop 传入处理器",
            ));
        }
        // 检查同名 prop 声明。
        let known = component
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
                "删除该属性，或把同名字段加入 Component props",
            ));
        }
        // 拒绝重复传入同一 prop。
        if attributes
            .insert(attribute.name.clone(), attribute)
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
    // 检查全部必需 props。
    for prop in &component.props {
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

// 判断节点是否会生成可见或结构 View。
fn is_renderable_node(node: &Node) -> bool {
    // 空白文本只承担源码排版作用。
    !matches!(node, Node::Text(text) if text.value.trim().is_empty())
}
