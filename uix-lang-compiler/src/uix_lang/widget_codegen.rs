// 引入确定性组件、字段与名称集合。
use std::collections::{BTreeMap, BTreeSet};

// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Compiler System 唯一句柄位登记。
use crate::projection_schema::UI_PROJECTION_SCHEMA;

// 引入组件、文档、表达式与视图 AST。
use super::{
    Attribute, AttributeValue, ControlBinding, Declaration, Diagnostic, Document, Element,
    ExpressionKind, ExpressionNode, KeyframesDeclaration, Node, RecordDeclaration,
    StyleClassResolver, WidgetDeclaration, WidgetScopeMarker, with_widget_source_marker,
};
// 引入既有核心 View 生成入口。
use super::generate_view;
// 引入组件动态样式使用检测。
use super::dynamic_style_lower::nodes_use_set_style;
// 引入组件调用属性完整性与必填校验入口。
use super::widget_call_validator::validate_widget_attributes;

// 拆分静态声明基座与 For 实际组件作用域的生成，保持主展开器规模受控。
mod instance_scope;

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
    // 借用入口保留原有按需复制语义，避免其他生成路径复制整份文档。
    let mut expander = WidgetExpander::new(document)?;
    // 为文档根按需建立 hover 或 animation 生命周期作用域。
    let root_style_scope = expander.begin_document_style_scope(document);
    // 展开根元素与全部组件调用。
    let mut root = expander.expand_root(&document.root)?;
    // 恢复作用域栈并把样式状态生命周期标记绑定到实际根。
    expander.finish_document_style_scope(&mut root, root_style_scope);
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

// 消费发射阶段独占的文档，把顶层声明直接移动到展开器注册表。
pub(crate) fn generate_document_view_owned(
    mut document: Document,
) -> Result<TokenStream, Diagnostic> {
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
    let mut expander = WidgetExpander::new_owned(&mut document)?;
    // 为文档根按需建立 hover 或 animation 生命周期作用域。
    let root_style_scope = expander.begin_document_style_scope(&document);
    // 展开根元素与全部组件调用。
    let mut root = expander.expand_root(&document.root)?;
    // 恢复作用域栈并把样式状态生命周期标记绑定到实际根。
    expander.finish_document_style_scope(&mut root, root_style_scope);
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
pub(super) struct WidgetExpander {
    // 保存名称到组件声明的拥有型注册表。
    pub(super) widgets: BTreeMap<String, WidgetDeclaration>,
    // 保存名称到 record 声明的拥有型注册表。
    pub(super) records: BTreeMap<String, RecordDeclaration>,
    // 保存名称到关键帧声明的拥有型注册表。
    pub(super) keyframes: BTreeMap<String, KeyframesDeclaration>,
    // 保存最终 View 之前执行的有序准备语句。
    pub(super) setup: Vec<TokenStream>,
    // 保存卫生名称的单调递增编号。
    pub(super) next_id: usize,
    // 保存当前组件展开栈以拒绝递归定义。
    pub(super) stack: Vec<String>,
    // 保存已经展开继承的样式类注册表。
    pub(super) styles: StyleClassResolver,
    // 保存最近 UIX Widget 的运行时作用域局部变量。
    pub(super) widget_scope_stack: Vec<Ident>,
    // 保存最近 UIX Widget 获准使用的 Rust 外部符号。
    pub(super) external_scope_stack: Vec<BTreeSet<String>>,
    // 保存当前 computed 表达式尚不可引用的派生名称。
    pub(super) pending_computed_scope_stack: Vec<BTreeSet<String>>,
    // 保存最近 UIX Widget 可在事件位置调用的同步 action。
    pub(super) action_scope_stack: Vec<BTreeMap<String, super::WidgetAction>>,
    // 保存正在静态展开的 action 调用链以拒绝递归。
    pub(super) action_expansion_stack: Vec<String>,
    // 保存当前组件调用已经在调用方作用域展开的插槽内容。
    pub(super) slot_projection_stack: Vec<BTreeMap<String, Vec<Node>>>,
    // 保存嵌套 For 引入的词法局部标识符。
    pub(super) local_scope_stack: Vec<BTreeSet<String>>,
    // 保存嵌套 For 当前实际实例路径的局部变量。
    pub(super) for_path_stack: Vec<Ident>,
    // 保存嵌套 For 子树各自需要在每次迭代克隆的拥有型事件捕获。
    pub(super) for_iteration_clone_stack: Vec<Vec<String>>,
    // 保存嵌套 For 子树各自需要在实际迭代中执行的组件准备语句。
    pub(super) for_iteration_setup_stack: Vec<Vec<TokenStream>>,
}

// 实现文档级组件展开与结构校验。
impl WidgetExpander {
    // 从借用文档构造组件注册表，保持既有生成入口的分配边界。
    fn new(document: &Document) -> Result<Self, Diagnostic> {
        // 收集全部已完成名称去重验证的组件声明。
        let widgets = document
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Widget(widget) => Some((widget.name.clone(), widget.clone())),
                _ => None,
            })
            .collect();
        // 收集全部 record 声明供 state 类型与结构体字面量使用。
        let records = document
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Record(record) => Some((record.name.clone(), record.clone())),
                _ => None,
            })
            .collect();
        // 收集全部已完成名称去重验证的关键帧声明。
        let keyframes = document
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Keyframes(keyframes) => {
                    Some((keyframes.name.clone(), keyframes.clone()))
                }
                _ => None,
            })
            .collect();
        // 构造并验证样式类继承注册表。
        let styles = StyleClassResolver::new(document)?;
        Ok(Self::with_registries(widgets, records, keyframes, styles))
    }

    // 从顶层声明构造组件注册表。
    fn new_owned(document: &mut Document) -> Result<Self, Diagnostic> {
        // 构造并验证样式类继承注册表。
        let styles = StyleClassResolver::new(document)?;
        // 发射文档已经由当前 lowering 独占，声明可直接移动到各私有注册表。
        let mut widgets = BTreeMap::new();
        let mut records = BTreeMap::new();
        let mut keyframes = BTreeMap::new();
        for declaration in std::mem::take(&mut document.declarations) {
            match declaration {
                Declaration::Widget(widget) => {
                    widgets.insert(widget.name.clone(), widget);
                }
                Declaration::Record(record) => {
                    records.insert(record.name.clone(), record);
                }
                Declaration::Keyframes(keyframes_declaration) => {
                    keyframes.insert(keyframes_declaration.name.clone(), keyframes_declaration);
                }
                // 样式声明已由 resolver 收口，其余声明不参与 View 组件展开。
                _ => {}
            }
        }
        // 返回初始展开状态。
        Ok(Self::with_registries(widgets, records, keyframes, styles))
    }

    // 用已经确定所有权的声明注册表初始化一次展开。
    fn with_registries(
        widgets: BTreeMap<String, WidgetDeclaration>,
        records: BTreeMap<String, RecordDeclaration>,
        keyframes: BTreeMap<String, KeyframesDeclaration>,
        styles: StyleClassResolver,
    ) -> Self {
        Self {
            // 写入组件注册表。
            widgets,
            // 写入 record 注册表。
            records,
            // 写入关键帧注册表。
            keyframes,
            // 初始没有准备语句。
            setup: Vec::new(),
            // 卫生编号从零开始。
            next_id: 0,
            // 初始不在任何组件体内。
            stack: Vec::new(),
            // 写入样式类注册表。
            styles,
            // 文档根尚未进入任何 UIX Widget。
            widget_scope_stack: Vec::new(),
            // 文档根没有组件 external 白名单。
            external_scope_stack: Vec::new(),
            // 文档根不在 computed 有序求值期间。
            pending_computed_scope_stack: Vec::new(),
            // 文档根没有组件 action。
            action_scope_stack: Vec::new(),
            // 初始没有正在展开的 action。
            action_expansion_stack: Vec::new(),
            // 文档根没有等待模板占位消费的插槽内容。
            slot_projection_stack: Vec::new(),
            // 文档根没有 For 词法局部变量。
            local_scope_stack: Vec::new(),
            // 文档根尚未进入任何 For 实例。
            for_path_stack: Vec::new(),
            // 文档根没有等待收集的循环事件捕获。
            for_iteration_clone_stack: Vec::new(),
            // 文档根没有等待收集的逐迭代组件准备语句。
            for_iteration_setup_stack: Vec::new(),
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
            // 合成容器不属于用户组件实例，因此不携带私有状态作用域。
            widget_scopes: Vec::new(),
            // 合成容器不是 For，因此没有逐迭代事件捕获。
            for_iteration_clones: Vec::new(),
            // 合成容器不是 For，因此没有逐迭代组件准备语句。
            for_iteration_setup: Vec::new(),
        })
    }

    // 把准备语句路由到最近 For 实例或文档根准备区。
    pub(super) fn push_setup(&mut self, statement: TokenStream) {
        // For 子树中的值可能依赖当前项，必须在实际迭代中建立。
        if let Some(setup) = self.for_iteration_setup_stack.last_mut() {
            // 保持展开顺序追加到最近的动态实例边界。
            setup.push(statement);
        } else {
            // 静态组件继续在最终 View 前只执行一次。
            self.setup.push(statement);
        }
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
        // Slot 占位由组件调用投影栈直接内联。
        if element.name == "Slot" {
            // 返回已经在调用方作用域完成展开的节点。
            return self.expand_slot_placeholder(element);
        }
        // 自定义标签交给组件调用展开。
        if self.widgets.contains_key(&element.name) {
            // 返回组件展开节点。
            return self.expand_widget_call(element, bindings, inside_for);
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
        // action 必须先在实际事件调用点静态内联，供随后 setStyle 扫描绑定当前 View。
        for attribute in &mut expanded.attributes {
            if !attribute.name.starts_with('@') {
                continue;
            }
            if let AttributeValue::Expression(expression) = &mut attribute.value {
                let active_bindings = self.clone_event_bindings(bindings);
                self.transform_expression(
                    &mut expression.expression,
                    &active_bindings,
                    true,
                    false,
                )?;
            }
        }
        // 在 class 被消费前绑定状态伪类与元素既有事实。
        let pseudo_style = self.prepare_pseudo_style(&expanded, bindings)?;
        // 优先降低组件内动态样式，否则走既有静态样式路径。
        if !self.prepare_dynamic_style(&mut expanded, bindings)? {
            // 合并 class、继承与内联 style。
            self.styles.apply(&mut expanded)?;
            // 消费 animation 并附加持久化 Animated 装饰。
            self.prepare_animation(&mut expanded)?;
        }
        // 伪类最后叠加，避免自定义动态样式覆盖自动状态外观。
        if let Some(binding) = pseudo_style {
            // 保存最终 View 包裹所需的状态差异元数据。
            expanded
                .widget_scopes
                .push(WidgetScopeMarker::PseudoStyle(binding));
        }
        // 消费 transition 并在全部状态样式之外附加目标比较装饰。
        self.prepare_transition(&mut expanded)?;
        // 逐个改写普通属性与事件表达式。
        for attribute in &mut expanded.attributes {
            // 事件已在动态样式扫描前完成字段与 action 降低。
            if attribute.name.starts_with('@') {
                continue;
            }
            // VirtualScroll item 只声明直接 For 的行绑定名称，不读取组件或宿主值。
            if element.name == "VirtualScroll" && attribute.name == "item" {
                // 保留原标识符，交给 VirtualScroll 契约与直接 For 绑定做一致性校验。
                continue;
            }
            // 只有表达式属性需要字段改写。
            if let AttributeValue::Expression(expression) = &mut attribute.value {
                // 句柄位属性（value/checked/current/open 等）读取 State 句柄而非读值。
                let handle_mode = is_state_handle_attribute(&element.name, &attribute.name);
                // 改写 props、state 与 setState。
                self.transform_expression(
                    // 可变借用表达式树。
                    &mut expression.expression,
                    // 使用当前属性的字段绑定。
                    bindings,
                    // 传递状态更新作用域。
                    false,
                    // 传递句柄位改写模式。
                    handle_mode,
                )?;
            }
        }
        // 标记是否为当前 For 压入了词法局部变量。
        let mut pushed_for_locals = false;
        // 改写 If 或 For 控制表达式。
        if let Some(control) = &mut expanded.control {
            // 按控制绑定形状改写。
            match control {
                // 改写 If 或 ElseIf 条件。
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
                ControlBinding::For {
                    // 借用循环项绑定名称。
                    binding,
                    // 借用可选索引绑定名称。
                    index_binding,
                    // 借用数据源与稳定 key。
                    iterable,
                    key,
                    // 忽略仅用于诊断的绑定跨度。
                    ..
                } => {
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
                    // 保存当前 For 为 key 与子树引入的词法绑定。
                    let mut locals = BTreeSet::new();
                    // 循环项始终属于当前 For 局部作用域。
                    locals.insert(binding.clone());
                    // 可选索引存在时加入同一局部作用域。
                    if let Some(index_binding) = index_binding {
                        // 保存索引绑定名称。
                        locals.insert(index_binding.clone());
                    }
                    // 在 key 与子树展开期间启用当前词法作用域。
                    self.local_scope_stack.push(locals);
                    // 记录当前元素负责恢复该作用域。
                    pushed_for_locals = true;
                    // 存在 key 时同步改写。
                    if let Some(key) = key {
                        // 改写稳定身份表达式。
                        let key_result = self.transform_expression(
                            // 可变借用 key 表达式。
                            &mut key.expression,
                            // 使用当前字段绑定。
                            bindings,
                            // key 不能更新状态。
                            false,
                            // key 不是句柄位。
                            false,
                        );
                        // key 诊断前先恢复词法作用域。
                        if let Err(error) = key_result {
                            // 弹出当前 For 局部变量。
                            self.local_scope_stack.pop();
                            // 返回原始 key 诊断。
                            return Err(error);
                        }
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
            // 为当前 For 建立独立的逐迭代捕获收集区。
            self.for_iteration_clone_stack.push(Vec::new());
            // 为当前 For 建立独立的逐迭代准备语句收集区。
            self.for_iteration_setup_stack.push(Vec::new());
        }
        // 展开全部有序子节点并暂存诊断以确保作用域恢复。
        let expanded_children = self.expand_nodes(&element.children, bindings, child_inside_for);
        // 离开 For 子树后恢复外层实例路径。
        if for_path.is_some() {
            // 把当前 For 子树收集到的捕获写回控制元素契约。
            expanded.for_iteration_clones = self
                // 取出最近循环的独立收集区。
                .for_iteration_clone_stack
                // 弹出当前循环收集区。
                .pop()
                // For 路径存在时收集区必然同步存在。
                .expect("For 捕获收集栈必须与路径栈同步");
            // 取出当前 For 子树按依赖顺序生成的逐迭代准备语句。
            expanded.for_iteration_setup = self
                // 借用最近循环的独立准备语句收集区。
                .for_iteration_setup_stack
                // 弹出当前 For 的完整准备语句列表。
                .pop()
                // For 路径存在时准备语句收集区必然同步存在。
                .expect("For 准备语句栈必须与路径栈同步")
                // 把可克隆令牌保存为可比较的内部 AST 文本。
                .into_iter()
                // 令牌文本只在同一宏展开内重新解析，不成为公开语法。
                .map(|statement| statement.to_string())
                // 保持生成顺序收集到控制元素。
                .collect();
            // 弹出刚才压入的循环路径。
            self.for_path_stack.pop();
        }
        // 离开 For 子树后恢复外层词法变量。
        if pushed_for_locals {
            // 弹出当前 For 局部变量集合。
            self.local_scope_stack.pop();
        }
        // 在所有作用域恢复后传播子树诊断。
        expanded.children = expanded_children?;
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
    fn expand_widget_call(
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
        let widget = self
            // 按标签名称查找组件。
            .widgets
            // 读取已确认存在的声明。
            .get(&element.name)
            // 复制完整声明。
            .cloned()
            // 注册表检查保证存在。
            .expect("组件存在性已在调用前确认");
        // 预先判断当前组件是否声明动态样式私有状态。
        let uses_dynamic_style = nodes_use_set_style(&widget.children)
            // action 内的 setStyle 也必须取得同一组件运行时作用域。
            || widget
                .actions
                .iter()
                .any(|action| super::action_semantic::action_body_uses_set_style(&action.body));
        // 预先判断当前组件是否需要自动 hover 私有状态。
        let uses_hover_style = self.styles.nodes_use_hover(&widget.children);
        // 预先判断当前组件是否需要持久化声明式动画。
        let uses_animation_style = self.styles.nodes_use_animation(&widget.children);
        // 预先判断当前组件是否需要持久化声明式状态过渡。
        let uses_transition_style = self.styles.nodes_use_transition(&widget.children);
        // 拒绝直接或间接递归组件。
        if self.stack.contains(&widget.name) {
            // 构造包含闭环的调用路径。
            let mut cycle = self.stack.clone();
            // 追加再次进入的组件。
            cycle.push(widget.name.clone());
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
        // 没有任何 Slot 声明的组件继续拒绝调用方子节点。
        if widget.slots.is_empty() && element.children.iter().any(is_renderable_node) {
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
        let attributes = validate_widget_attributes(element, &widget)?;
        // 进入当前组件展开栈。
        self.stack.push(widget.name.clone());
        // 标记当前调用是否已进入被调用组件的 external 作用域。
        let mut pushed_external = false;
        // 标记当前调用是否已进入被调用组件的 action 作用域。
        let mut pushed_actions = false;
        // 标记当前调用是否已压入调用方插槽投影。
        let mut pushed_slots = false;
        // 在闭包内展开以确保错误路径也弹栈。
        let result = with_widget_source_marker(&widget.name, || {
            // 在进入被调用组件作用域前先按调用方绑定展开全部插槽内容。
            let projections = self.prepare_slot_projections(
                // 传递完整组件调用节点。
                element,
                // 传递声明的插槽集合。
                &widget,
                // 使用调用方字段绑定。
                outer_bindings,
                // 沿用调用方 For 动态实例事实。
                inside_for,
            )?;
            // 压入本次模板展开可消费的插槽投影。
            self.slot_projection_stack.push(projections);
            // 记录错误路径也必须恢复插槽投影栈。
            pushed_slots = true;
            // 组件体只看见自身字段。
            let mut bindings = Bindings::new();
            // 私有 state 与状态样式需要可由实际根承载的运行时作用域。
            let requires_scope = !widget.states.is_empty()
                // 动态 class 选择拥有组件私有状态。
                || uses_dynamic_style
                // hover 伪类拥有组件私有状态。
                || uses_hover_style
                // animation 播放拥有组件私有状态。
                || uses_animation_style
                // transition 目标拥有组件私有状态。
                || uses_transition_style;
            // 为静态调用或 For 实际实例生成对应作用域。
            let scope = self.prepare_widget_scope(
                // 传递声明名称供卫生标识与身份生成使用。
                &widget.name,
                // 传递实际调用元素及其稳定源码跨度。
                element,
                // 传递是否需要从最近 For 路径派生实例。
                inside_for,
                // 无状态组件保持零运行时作用域开销。
                requires_scope,
            );
            // 按声明顺序生成 props。
            for prop in &widget.props {
                // 为省略的可选 prop 保留一个声明期合成属性槽。
                let default_attribute;
                // 优先读取调用方属性，否则使用已验证默认表达式。
                let attribute = if let Some(attribute) = attributes.get(&prop.name) {
                    // 返回显式调用参数。
                    *attribute
                } else {
                    // 调用校验保证省略的 prop 一定拥有默认值。
                    let default = prop.default.clone().expect("可选 prop 必须拥有默认值");
                    // 字面量复用既有 prop 类型物化，数据构造保留表达式生成。
                    let default_value = match &default.kind {
                        // 字符串默认值交给拥有型 String 转换。
                        ExpressionKind::String(value) => AttributeValue::Literal(value.clone()),
                        // 数字默认值保留作者数值文本。
                        ExpressionKind::Number(value) => AttributeValue::Literal(value.clone()),
                        // 布尔默认值恢复为严格字面文本。
                        ExpressionKind::Boolean(value) => {
                            // 转换为 true 或 false。
                            AttributeValue::Literal(value.to_string())
                        }
                        // 已登记数据构造继续使用表达式路径。
                        _ => AttributeValue::Expression(ExpressionNode {
                            // 保存可辨识的内部来源文本。
                            source: format!("<default:{}>", prop.name),
                            // 保存已验证默认表达式。
                            expression: default,
                            // 沿用 props 声明跨度。
                            span: prop.span,
                        }),
                    };
                    // 构造只在本轮绑定期间借用的合成属性。
                    default_attribute = Attribute {
                        // 沿用 prop 名称供类型诊断使用。
                        name: prop.name.clone(),
                        // 保存按形状物化的默认值。
                        value: default_value,
                        // 沿用 props 声明跨度。
                        span: prop.span,
                    };
                    // 返回合成默认属性借用。
                    &default_attribute
                };
                // 生成类型化 prop 绑定。
                self.emit_prop_binding(prop, attribute, outer_bindings, &mut bindings)?;
            }
            // props 在调用方作用域求值完成后进入被调用组件白名单。
            self.external_scope_stack
                // 转为确定性集合供 state 与组件体表达式查询。
                .push(widget.external.iter().cloned().collect());
            // 记录错误路径也需要恢复被调用组件作用域。
            pushed_external = true;
            // action 只在所属 Widget 模板的事件表达式中可见。
            self.action_scope_stack.push(
                widget
                    // 借用声明中的有序 action。
                    .actions
                    // 遍历全部声明。
                    .iter()
                    // 复制为按名称静态查询的有序映射。
                    .map(|action| (action.name.clone(), action.clone()))
                    // 收集当前 action 作用域。
                    .collect(),
            );
            // 记录错误路径也需要恢复 action 作用域。
            pushed_actions = true;
            // 按声明顺序生成私有状态。
            for state in &widget.states {
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
            // 按声明顺序生成无缓存的派生局部绑定。
            self.emit_computed_bindings(&widget.computed, &mut bindings)?;
            // 动态样式降低期间使用最近 UIX Widget 作用域。
            if let Some(scope) = scope.as_ref() {
                // 压入当前组件运行时作用域。
                self.widget_scope_stack.push(scope.clone());
            }
            // 沿用调用点的动态实例上下文展开组件体与所有嵌套组件。
            let expanded_nodes = self.expand_nodes(&widget.children, &bindings, inside_for);
            // 组件体展开完成后恢复外层组件作用域。
            if scope.is_some() {
                // 弹出当前组件运行时作用域。
                self.widget_scope_stack.pop();
            }
            // 在作用域栈恢复后再传播组件体诊断。
            let mut nodes = expanded_nodes?;
            // 拥有私有状态的组件必须把作用域标记附到每个展开后的顶层根。
            if let Some(scope) = scope.as_ref() {
                // 使运行时能在卸载时回收并在 reconcile 时复用正确实例的状态槽。
                self.mark_widget_roots(&mut nodes, scope);
            }
            // 返回附带作用域标记的组件展开结果。
            Ok(nodes)
        });
        // 已进入被调用组件时恢复调用方 external 作用域。
        if pushed_external {
            // 弹出被调用组件外部符号白名单。
            self.external_scope_stack.pop();
        }
        // 已进入被调用组件时恢复调用方 action 作用域。
        if pushed_actions {
            // 弹出被调用组件的 action 表。
            self.action_scope_stack.pop();
        }
        // 已进入插槽投影上下文时恢复外层组件投影。
        if pushed_slots {
            // 弹出当前组件调用的投影表。
            self.slot_projection_stack.pop();
        }
        // 离开当前组件展开栈。
        self.stack.pop();
        // 返回展开结果或诊断。
        result
    }

    // 为组件展开后的每个顶层实际根附加私有状态作用域标记。
    fn mark_widget_roots(&mut self, nodes: &mut [Node], scope: &Ident) {
        // 按展开后的源码顺序分配多根组件的稳定根序号。
        for (root_ordinal, node) in nodes.iter_mut().enumerate() {
            // 只有元素可以承载或向控制流传播 ViewNode 元数据。
            if let Node::Element(element) = node {
                // 把当前组件作用域追加到已有嵌套组件标记之后。
                element.widget_scopes.push(WidgetScopeMarker::Scope {
                    // 保存卫生局部变量名称，以便最终令牌重建标识符。
                    scope_name: scope.to_string(),
                    // 保存当前顶层根的稳定序号。
                    root_ordinal: root_ordinal as u64,
                });
            }
        }
    }

    // 使用固定 FNV-1a 算法把静态源码身份映射为跨构建可复现的 u64 编号。
    pub(super) fn stable_widget_id(value: &str) -> u64 {
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
    // 句柄位由 UI 投影 schema 唯一登记，组件展开器只做查询。
    UI_PROJECTION_SCHEMA
        .handle_slot(element_name, attribute_name)
        .is_some()
}
