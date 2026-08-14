// 引入确定性组件与字段映射。
use std::collections::BTreeMap;

// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件、文档、表达式与视图 AST。
use super::{
    Attribute, AttributeValue, ComponentDeclaration, ComponentScopeMarker, ControlBinding,
    Declaration, Diagnostic, Document, Element, Node, RecordDeclaration, StyleClassResolver,
};
// 引入既有核心 View 生成入口。
use super::generate_view;
// 引入组件动态样式使用检测。
use super::dynamic_style_lower::nodes_use_set_style;

// 保存组件字段展开后的 Rust 局部绑定。
#[derive(Clone)]
pub(super) struct Binding {
    // 保存读取字段值时使用的卫生名称。
    pub(super) value_name: String,
    // 保存可由 setState 写入的 State 句柄名称。
    pub(super) state_name: Option<String>,
    // 保存值、回调或响应式状态类别。
    pub(super) kind: BindingKind,
    // 类型化整数/单精度状态在 setState 值中保留作者数字形状。
    pub(super) authored_numbers: bool,
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
    // uix! 永久保持 ViewNode 契约，不接受应用生命周期根。
    if document.root.name == "App" {
        // 返回定向迁移诊断。
        return Err(Diagnostic::new(
            // 指向完整 App 根。
            document.root.span,
            // 说明入口契约冲突。
            "uix! 只生成 ViewNode，不能生成 <App> 应用入口",
            // 指向独立的 App builder 宏。
            "把该调用改为 uix_app!(...)，或移除 <App> 并保留单个 View 根",
        ));
    }
    // 创建组件感知展开器。
    let mut expander = ComponentExpander::new(document)?;
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
    // 保存名称到 record 声明的完整副本。
    pub(super) records: BTreeMap<String, RecordDeclaration>,
    // 保存最终 View 之前执行的有序准备语句。
    pub(super) setup: Vec<TokenStream>,
    // 保存卫生名称的单调递增编号。
    pub(super) next_id: usize,
    // 保存当前组件展开栈以拒绝递归定义。
    pub(super) stack: Vec<String>,
    // 保存已经展开继承的样式类注册表。
    pub(super) styles: StyleClassResolver,
    // 保存最近 UIX Component 的运行时作用域局部变量。
    pub(super) component_scope_stack: Vec<Ident>,
    // 保存嵌套 For 当前实际实例路径的局部变量。
    pub(super) for_path_stack: Vec<Ident>,
}

// 实现文档级组件展开与结构校验。
impl ComponentExpander {
    // 从顶层声明构造组件注册表。
    fn new(document: &Document) -> Result<Self, Diagnostic> {
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
        // 收集全部 record 声明供 state 类型与结构体字面量使用。
        let records = document
            // 遍历顶层声明。
            .declarations
            // 借用声明迭代器。
            .iter()
            // 只保留 record 声明。
            .filter_map(|declaration| match declaration {
                // 复制 record 名称与声明。
                Declaration::Record(record) => {
                    // 返回映射条目。
                    Some((record.name.clone(), record.clone()))
                }
                // 其他声明不占用 record 命名空间。
                _ => None,
            })
            // 收集到有序映射。
            .collect();
        // 构造并验证样式类继承注册表。
        let styles = StyleClassResolver::new(document)?;
        // 返回初始展开状态。
        Ok(Self {
            // 写入组件注册表。
            components,
            // 写入 record 注册表。
            records,
            // 初始没有准备语句。
            setup: Vec::new(),
            // 卫生编号从零开始。
            next_id: 0,
            // 初始不在任何组件体内。
            stack: Vec::new(),
            // 写入样式类注册表。
            styles,
            // 文档根尚未进入任何 UIX Component。
            component_scope_stack: Vec::new(),
            // 文档根尚未进入任何 For 实例。
            for_path_stack: Vec::new(),
        })
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
            // 合成容器不属于用户组件实例，因此不携带私有状态作用域。
            component_scopes: Vec::new(),
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
        // 为 For 子树创建实际实例路径名称并写入内部控制属性。
        let for_path = if element.name == "For" {
            // 为当前循环生成卫生路径局部变量。
            let path = self.fresh_ident("for_path", "instance");
            // 保存当前循环路径名称供控制流代码生成读取。
            expanded.attributes.push(Attribute {
                // 使用不属于 UIX 公共属性的内部名称。
                name: "__uix_for_path".to_string(),
                // 保存卫生局部变量名称。
                value: AttributeValue::Literal(path.to_string()),
                // 沿用控制元素跨度。
                span: element.span,
            });
            // 存在父循环时同时保存父实例路径名称。
            if let Some(parent) = self.for_path_stack.last() {
                // 追加父路径内部属性。
                expanded.attributes.push(Attribute {
                    // 使用内部父路径名称。
                    name: "__uix_for_parent_path".to_string(),
                    // 保存父路径卫生名称。
                    value: AttributeValue::Literal(parent.to_string()),
                    // 沿用控制元素跨度。
                    span: element.span,
                });
            }
            // 返回当前循环路径供展开子树期间压栈。
            Some(path)
        } else {
            // 普通元素不创建循环路径。
            None
        };
        // 优先降低组件内动态样式，否则走既有静态样式路径。
        if !self.prepare_dynamic_style(&mut expanded, bindings)? {
            // 合并 class、继承与内联 style。
            self.styles.apply(&mut expanded)?;
        }
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
                // 句柄位属性（value/checked/current/open 等）读取 State 句柄而非读值。
                let handle_mode = is_state_handle_attribute(&element.name, &attribute.name);
                // 改写 props、state 与 setState。
                self.transform_expression(
                    // 可变借用表达式树。
                    &mut expression.expression,
                    // 使用当前属性的字段绑定。
                    &active_bindings,
                    // 传递状态更新作用域。
                    allow_set_state,
                    // 传递句柄位改写模式。
                    handle_mode,
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
                    // 条件不是句柄位。
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
                        // 数据源不是句柄位。
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
                            // key 不是句柄位。
                            false,
                        )?;
                    }
                }
            }
        }
        // For 的直接子树进入动态实例作用域。
        let child_inside_for = inside_for || element.name == "For";
        // For 子节点继承当前实际实例路径名称。
        if let Some(path) = for_path.as_ref() {
            // 压入当前循环路径。
            self.for_path_stack.push(path.clone());
        }
        // 展开全部有序子节点。
        expanded.children = self.expand_nodes(&element.children, bindings, child_inside_for)?;
        // 离开 For 子树后恢复外层实例路径。
        if for_path.is_some() {
            // 弹出刚才压入的循环路径。
            self.for_path_stack.pop();
        }
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
                        // 插值读取值而非句柄。
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
        // 预先判断当前组件是否声明动态样式私有状态。
        let uses_dynamic_style = nodes_use_set_style(&component.children);
        // For 内的状态、prop 或动态样式需要运行时逐实例存储。
        if inside_for
            && (!component.props.is_empty() || !component.states.is_empty() || uses_dynamic_style)
        {
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
            // 仅为拥有私有状态的静态调用创建窗口私有的运行时作用域。
            let scope = if component.states.is_empty() && !uses_dynamic_style {
                // 无私有状态的组件不进入运行时作用域，保留既有 For 语义。
                None
            } else {
                // 为当前静态调用生成卫生的作用域局部变量名称。
                let scope_ident = self.fresh_ident("component_scope", &component.name);
                // 使用组件调用标签与源码跨度形成稳定声明身份。
                // 先拼接仅由静态 UIX 源码决定的声明身份材料。
                let declaration_source = format!(
                    // 保留组件标签与完整调用跨度，区分同类型的相邻静态调用。
                    "{}:{}:{}",
                    // 写入组件调用标签。
                    element.name,
                    // 写入调用开始偏移。
                    element.span.start,
                    // 写入调用结束偏移。
                    element.span.end,
                );
                // 把声明身份材料压缩为运行时 API 约定的稳定无符号编号。
                let declaration_id = Self::stable_component_id(&declaration_source);
                // 在组件体展开前取得当前窗口和本轮构建专属的作用域句柄。
                self.setup.push(quote! {
                    // 为当前静态组件调用取得可跨 reconcile 复用的私有状态作用域。
                    let #scope_ident = ::uix::ui::__private::uix_component_scope(
                        // 由 Rust 宏调用点区分同一 UIX 文档的不同根工厂。
                        concat!(module_path!(), ":", file!(), ":", line!(), ":", column!()),
                        // 由 UIX 静态调用位置区分同一组件的多个实例。
                        #declaration_id,
                    );
                });
                // 返回后续 state 初始化和 View 标记共用的作用域局部变量。
                Some(scope_ident)
            };
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
                self.emit_private_state(
                    // 私有 state 的作用域必定已在存在 state 时创建。
                    scope.as_ref().expect("私有 state 组件必须拥有运行时作用域"),
                    // 传递当前 state 声明。
                    state,
                    // 写入当前组件字段绑定。
                    &mut bindings,
                )?;
            }
            // 动态样式降低期间使用最近 UIX Component 作用域。
            if let Some(scope) = scope.as_ref() {
                // 压入当前组件运行时作用域。
                self.component_scope_stack.push(scope.clone());
            }
            // 沿用调用点的动态实例上下文展开组件体与所有嵌套组件。
            let expanded_nodes = self.expand_nodes(&component.children, &bindings, inside_for);
            // 组件体展开完成后恢复外层组件作用域。
            if scope.is_some() {
                // 弹出当前组件运行时作用域。
                self.component_scope_stack.pop();
            }
            // 在作用域栈恢复后再传播组件体诊断。
            let mut nodes = expanded_nodes?;
            // 拥有私有状态的组件必须把作用域标记附到每个展开后的顶层根。
            if let Some(scope) = scope.as_ref() {
                // 使运行时能在卸载时回收并在 reconcile 时复用正确实例的状态槽。
                self.mark_component_roots(&mut nodes, scope);
            }
            // 返回附带作用域标记的组件展开结果。
            Ok(nodes)
        })();
        // 离开当前组件展开栈。
        self.stack.pop();
        // 返回展开结果或诊断。
        result
    }

    // 为组件展开后的每个顶层实际根附加私有状态作用域标记。
    fn mark_component_roots(&mut self, nodes: &mut [Node], scope: &Ident) {
        // 按展开后的源码顺序分配多根组件的稳定根序号。
        for (root_ordinal, node) in nodes.iter_mut().enumerate() {
            // 只有元素可以承载或向控制流传播 ViewNode 元数据。
            if let Node::Element(element) = node {
                // 把当前组件作用域追加到已有嵌套组件标记之后。
                element.component_scopes.push(ComponentScopeMarker::Scope {
                    // 保存卫生局部变量名称，以便最终令牌重建标识符。
                    scope_name: scope.to_string(),
                    // 保存当前顶层根的稳定序号。
                    root_ordinal: root_ordinal as u64,
                });
            }
        }
    }

    // 使用固定 FNV-1a 算法把静态源码身份映射为跨构建可复现的 u64 编号。
    pub(super) fn stable_component_id(value: &str) -> u64 {
        // 从 FNV-1a 的标准 64 位偏移基开始。
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        // 按 UTF-8 字节顺序吸收全部静态身份材料。
        for byte in value.as_bytes() {
            // 混入当前字节。
            hash ^= u64::from(*byte);
            // 使用 FNV-1a 的标准 64 位乘数推进状态。
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3_u64);
        }
        // 返回确定性声明编号。
        hash
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

// 判断某个元素的属性是否要求 State<T> 句柄表达式。
// 句柄位属性在组件体内引用同名 state 或 State<T> prop 时改写为句柄本身，
// 使受控组件（Modal/Drawer）与双向绑定（value/checked/current）可以直接
// 使用组件私有状态，而不再被迫由 Rust 侧创建状态槽。
pub(super) fn is_state_handle_attribute(element_name: &str, attribute_name: &str) -> bool {
    // 按元素登记需要 State 句柄的属性集合。
    let handle_attributes: &[&str] = match element_name {
        // 受控浮层读取 State<bool> 打开状态。
        "Modal" | "Drawer" => &["open"],
        // 双向绑定输入控件读取 State<String>。
        "Input" | "InputGroup" | "AutoComplete" | "Mentions" => &["value"],
        // 双向绑定数值控件读取 State<T>。
        "InputNumber" | "Slider" | "Rate" => &["value"],
        // 双向绑定勾选控件读取 State<bool>。
        "Checkbox" | "Switch" => &["checked"],
        // 双向绑定选择控件读取 State<String> 或集合状态。
        "Radio" | "Segmented" | "Select" | "TreeSelect" => &["value"],
        // 双向绑定结构化路径与颜色状态。
        "Cascader" | "ColorPicker" => &["value"],
        // 双向绑定日期与时间状态。
        "DatePicker" | "TimePicker" => &["value"],
        // 区间滑块读取对象形式的双 State<f64> 句柄。
        "RangeSlider" | "DateRangePicker" => &["value"],
        // 步骤条与分页器读取 State<usize> 受控状态。
        "Steps" => &["current"],
        "Pagination" => &["current", "pageSize"],
        // 标签页读取 State<String> 活动 key 句柄。
        "Tabs" => &["activeKey"],
        // 菜单读取 typed 单选与展开状态句柄。
        "Menu" => &["selectedKey", "openKeys"],
        // Navigation 复用 Menu 的选择/展开句柄并额外读取整栏折叠句柄。
        "Navigation" => &["activeKey", "openKeys", "collapsed"],
        // 滚动容器与固钉组件读取滚动状态。
        "ScrollView" => &["offset"],
        "Affix" | "BackTop" | "FloatButtonBackTop" => &["scrollY"],
        // 类型化表单读取 State<M> 模型句柄。
        "Form" => &["model"],
        // 其余元素没有句柄位属性。
        _ => &[],
    };
    // 判断当前属性名是否在句柄位集合。
    handle_attributes.contains(&attribute_name)
}
