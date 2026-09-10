// 引入过程宏标识符以传递文档根作用域。
use proc_macro2::Ident;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器、字段绑定与状态句柄位判断。
use super::widget_codegen::{Bindings, WidgetExpander, is_state_handle_attribute};
// 引入元素、属性值、诊断与伪类绑定 AST。
use super::{
    Attribute, AttributeValue, Diagnostic, Document, Element, Node, PseudoStyleBinding,
    PseudoStyleCondition, WidgetScopeMarker,
};

// 实现状态伪类的编译期事实绑定。
impl WidgetExpander {
    // 为直接使用 hover、animation 或 transition 的文档根建立状态生命周期作用域。
    pub(super) fn begin_document_style_scope(
        // 可变借用展开器以登记准备语句和作用域。
        &mut self,
        // 接收当前完整文档。
        document: &Document,
    ) -> Option<Ident> {
        // 根子树没有任何样式状态能力时维持零状态开销。
        if !self
            // 从已解析样式注册表检测根子树。
            .styles
            // 把根包装为节点复用递归检测。
            .nodes_use_hover(&[Node::Element(document.root.clone())])
            // animation 同样需要跨 reconcile 私有状态。
            && !self
                .styles
                .nodes_use_animation(&[Node::Element(document.root.clone())])
            // transition 同样需要跨 reconcile 私有状态。
            && !self
                .styles
                .nodes_use_transition(&[Node::Element(document.root.clone())])
        {
            // 报告无需附加作用域。
            return None;
        }
        // 为当前宏文档根生成卫生作用域名称。
        let scope = self.fresh_ident("document_scope", &document.root.name);
        // 使用根标签与源码跨度形成稳定声明身份。
        let declaration_id = Self::stable_widget_id(&format!(
            // 固定身份前缀并纳入源码位置。
            "document-style-state:{}:{}:{}",
            // 使用根标签名称。
            document.root.name,
            // 使用根起始偏移。
            document.root.span.start,
            // 使用根结束偏移。
            document.root.span.end
        ));
        // 在展开前取得窗口私有的既有组件状态作用域。
        self.push_setup(quote! {
            // 复用组件状态存储作为文档根样式状态生命周期所有者。
            let #scope = ::uix_app::ui::__private::uix_widget_scope(
                concat!(module_path!(), ":", file!(), ":", line!(), ":", column!()),
                #declaration_id,
            );
        });
        // 让根子树展开期间能派生逐节点 hover、animation 与 transition 状态。
        self.widget_scope_stack.push(scope.clone());
        // 保存作用域供展开后附加生命周期标记。
        Some(scope)
    }

    // 在文档根展开后恢复栈并绑定样式状态生命周期。
    pub(super) fn finish_document_style_scope(
        // 可变借用展开器以恢复作用域栈。
        &mut self,
        // 接收已经展开的最终核心根元素。
        root: &mut Element,
        // 接收可选文档根作用域。
        scope: Option<Ident>,
    ) {
        // 仅在建立过作用域时执行恢复与绑定。
        let Some(scope) = scope else {
            // 没有样式状态时无需收尾。
            return;
        };
        // 弹出刚才创建的文档根作用域。
        self.widget_scope_stack.pop();
        // 追加普通组件作用域标记而不改变运行时契约。
        root.widget_scopes.push(WidgetScopeMarker::Scope {
            // 保存卫生作用域名称。
            scope_name: scope.to_string(),
            // 文档只有一个实际根。
            root_ordinal: 0,
        });
    }

    // 解析元素引用的状态变体并降低既有 disabled/checked 事实。
    pub(super) fn prepare_pseudo_style(
        // 可变借用展开状态以改写表达式绑定。
        &mut self,
        // 接收尚未消费 class 的原始核心元素。
        element: &Element,
        // 接收当前组件字段绑定。
        bindings: &Bindings,
    ) -> Result<Option<PseudoStyleBinding>, Diagnostic> {
        // 按元素 class 顺序合并三个状态的差异字段。
        let styles = self.styles.pseudo_styles(element)?;
        // 没有状态变体时保持现有静态样式路径。
        if styles.is_empty() {
            // 报告没有伪类元数据。
            return Ok(None);
        }
        // 伪类 animation 会要求状态进入时重建播放实例，当前由 transition 契约负责。
        if let Some(property) = styles
            // 依次检查 hover、disabled 与 checked 差异。
            .hover
            // 借用 hover 属性。
            .iter()
            // 串接 disabled 属性。
            .chain(styles.disabled.iter())
            // 串接 checked 属性。
            .chain(styles.checked.iter())
            // 定位 animation 简写。
            .find(|property| property.name == "animation")
        {
            // 返回明确组合诊断。
            return Err(Diagnostic::new(
                // 指向伪类 animation 属性。
                property.span,
                // 陈述失败原因。
                "状态伪类暂不支持启动 animation 播放实例",
                // 指向状态变化的正式契约。
                "把 animation 放到基础样式；hover/disabled/checked 的平滑变化使用 transition",
            ));
        }
        // hover 需要现有组件私有状态存储来跨 reconcile 保持事实。
        let hover_scope_name = if styles.hover.is_empty() {
            // 没有 hover 时不创建私有状态。
            None
        } else {
            // hover 必须位于组件或文档根提供的既有状态作用域中。
            let Some(scope) = self.widget_scope_stack.last() else {
                // 返回作用域诊断。
                return Err(Diagnostic::new(
                    element.span,
                    ":hover 状态伪类缺少声明式生命周期所有者",
                    "从 uix! 或 uix_app! 文档根展开该 View，或把它放入 Widget",
                ));
            };
            // 保存卫生作用域名称供代码生成恢复。
            Some(scope.to_string())
        };
        // disabled 变体必须绑定元素同名布尔事实。
        let disabled = if styles.disabled.is_empty() {
            // 没有 disabled 变体时不读取属性。
            None
        } else {
            // 查找元素现有 disabled 属性。
            let attribute = required_state_attribute(element, "disabled")?;
            // disabled 始终按布尔值表达式降低。
            let condition = self.lower_pseudo_condition(element, attribute, bindings, false)?;
            // 保存事实与差异字段。
            Some((condition, styles.disabled))
        };
        // checked 变体必须绑定元素同名布尔或 State<bool> 事实。
        let checked = if styles.checked.is_empty() {
            // 没有 checked 变体时不读取属性。
            None
        } else {
            // 查找元素现有 checked 属性。
            let attribute = required_state_attribute(element, "checked")?;
            // 已登记句柄位保持 State<bool> 所有权并在选择时读取 get()。
            let state_handle = is_state_handle_attribute(&element.name, "checked");
            // 降低静态、值或 State 条件。
            let condition =
                self.lower_pseudo_condition(element, attribute, bindings, state_handle)?;
            // 保存事实与差异字段。
            Some((condition, styles.checked))
        };
        // 使用节点类型与静态跨度形成跨构建稳定声明身份。
        let declaration_id = Self::stable_widget_id(&format!(
            "pseudo-style:{}:{}:{}",
            element.name, element.span.start, element.span.end
        ));
        // 返回只消费既有事实的伪类元数据。
        Ok(Some(PseudoStyleBinding {
            // 保存可选 hover 组件作用域。
            hover_scope_name,
            // 保存稳定子作用域声明标识。
            declaration_id,
            // 继承最近 For 的实际实例路径。
            instance_path_name: self.for_path_stack.last().map(ToString::to_string),
            // 保存 hover 差异字段。
            hover: styles.hover,
            // 保存 disabled 事实与差异。
            disabled,
            // 保存 checked 事实与差异。
            checked,
        }))
    }

    // 把元素布尔属性降低为伪类条件。
    fn lower_pseudo_condition(
        // 可变借用展开状态以生成字段克隆。
        &mut self,
        // 接收元素类型以判断句柄位。
        element: &Element,
        // 接收同名运行时事实属性。
        attribute: &Attribute,
        // 接收当前组件绑定。
        bindings: &Bindings,
        // 指示表达式是否保持 State<bool> 句柄。
        state_handle: bool,
    ) -> Result<PseudoStyleCondition, Diagnostic> {
        // 按属性值形状生成条件。
        match &attribute.value {
            // 字面量只接受严格布尔文本。
            AttributeValue::Literal(value) => match value.as_str() {
                // 保存静态真值。
                "true" => Ok(PseudoStyleCondition::Literal(true)),
                // 保存静态假值。
                "false" => Ok(PseudoStyleCondition::Literal(false)),
                // 拒绝非布尔字面量。
                _ => Err(Diagnostic::new(
                    attribute.span,
                    format!("伪类事实 {} 必须是布尔值", attribute.name),
                    "使用 true、false 或布尔表达式",
                )),
            },
            // 表达式复用组件字段降低与类型检查。
            AttributeValue::Expression(expression) => {
                // 克隆条件以免改变组件属性自身的生成语义。
                let mut expression = expression.clone();
                // 按值位或句柄位改写组件字段。
                self.transform_expression(
                    &mut expression.expression,
                    bindings,
                    false,
                    state_handle,
                )?;
                // 句柄位在选择分支时读取现有 State<bool>。
                if state_handle {
                    // 返回状态句柄条件。
                    Ok(PseudoStyleCondition::State(expression))
                } else {
                    // 返回普通布尔值条件。
                    Ok(PseudoStyleCondition::Value(expression))
                }
            }
            // 样式属性不能充当运行时事实。
            AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
                attribute.span,
                format!("伪类事实 {} 不能使用样式值", attribute.name),
                format!("在 <{}> 上使用布尔属性或表达式", element.name),
            )),
        }
    }
}

// 查找状态变体要求的同名元素属性。
fn required_state_attribute<'a>(
    // 接收待检查元素。
    element: &'a Element,
    // 接收 disabled 或 checked 名称。
    name: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 解析器已保证同名属性唯一。
    element
        // 遍历有序属性。
        .attributes
        // 查找精确名称。
        .iter()
        // 选择目标事实。
        .find(|attribute| attribute.name == name)
        // 缺失事实时返回定向诊断。
        .ok_or_else(|| {
            Diagnostic::new(
                element.span,
                format!(":{name} 状态伪类要求元素声明 {name} 属性"),
                format!(
                    "在 <{}> 上添加 {name}=\"true\" 或绑定布尔状态",
                    element.name
                ),
            )
        })
}
