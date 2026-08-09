// 引入状态目标去重集合。
use std::collections::HashSet;

// 引入过程宏标识符与卫生跨度。
use proc_macro2::{Ident, Span};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器与字段绑定。
use super::component_codegen::{Bindings, ComponentExpander};
// 引入调用参数、表达式与诊断 AST。
use super::{CallArgument, Diagnostic, Expression, ExpressionKind};

// 实现组件字段表达式改写与 setState 降低。
impl ComponentExpander {
    // 递归改写表达式中的组件字段与 setState。
    pub(super) fn transform_expression(
        // 可变借用展开状态。
        &mut self,
        // 接收待改写表达式。
        expression: &mut Expression,
        // 接收当前字段绑定。
        bindings: &Bindings,
        // 标记当前位置是否允许状态更新。
        allow_set_state: bool,
    ) -> Result<(), Diagnostic> {
        // 先识别需要替换整个节点的 setState 调用。
        let is_set_state = matches!(
            // 借用表达式形状。
            &expression.kind,
            // 只匹配 callee 为 setState 的调用。
            ExpressionKind::Call { callee, .. }
                if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setState")
        );
        // setState 使用专用降低路径。
        if is_set_state {
            // 非事件位置明确拒绝副作用。
            if !allow_set_state {
                // 返回作用域诊断。
                return Err(Diagnostic::new(
                    // 指向完整调用。
                    expression.span,
                    // 说明 setState 只属于事件处理器。
                    "setState 只能在 Component 的事件处理器中使用",
                    // 给出合法位置。
                    "把 setState(...) 移入 @click 等事件属性",
                ));
            }
            // 复制调用参数以允许替换整个表达式。
            let arguments = match &expression.kind {
                // 复制命名参数列表。
                ExpressionKind::Call { arguments, .. } => arguments.clone(),
                // 外层匹配保证不会进入此分支。
                _ => unreachable!("setState 形状已确认"),
            };
            // 生成独立可移动的更新器调用。
            return self.lower_set_state(expression, arguments, bindings);
        }
        // 语言 String 使用拥有所有权的 Rust String，而表达式生成器的原始字面量是 &str。
        if let ExpressionKind::String(value) = &expression.kind {
            // 复制字符串值以替换整个表达式节点。
            let value = value.clone();
            // 为拥有所有权转换器生成卫生名称。
            let converter_ident = self.fresh_ident("owned_string", "literal");
            // 生成只接收静态字面量的 String 转换闭包。
            self.setup.push(quote! {
                // 把语言字符串字面量规范化为拥有所有权的 String。
                let #converter_ident = |value: &'static str| ::std::string::String::from(value);
            });
            // 用普通闭包调用替换字符串字面量。
            expression.kind = ExpressionKind::Call {
                // 调用卫生转换器。
                callee: Box::new(Expression {
                    // 使用普通标识符交给既有生成器。
                    kind: ExpressionKind::Identifier(converter_ident.to_string()),
                    // 沿用字符串跨度。
                    span: expression.span,
                }),
                // 传入原始静态字符串字面量。
                arguments: vec![CallArgument {
                    // 普通闭包调用使用位置参数。
                    name: None,
                    // 保存原始字符串值。
                    value: Expression {
                        // 参数仍由既有生成器产生 &str 字面量。
                        kind: ExpressionKind::String(value),
                        // 沿用字符串跨度。
                        span: expression.span,
                    },
                    // 沿用字符串跨度。
                    span: expression.span,
                }],
            };
            // 当前字符串节点已经完成规范化。
            return Ok(());
        }
        // 按表达式形状递归改写。
        match &mut expression.kind {
            // 组件字段标识符替换为卫生名称。
            ExpressionKind::Identifier(name) => {
                // 查找同名组件字段。
                if let Some(binding) = bindings.get(name) {
                    // 替换为局部值名称。
                    *name = binding.value_name.clone();
                }
            }
            // 一元表达式递归改写操作数。
            ExpressionKind::Unary { operand, .. } => {
                // 改写内部操作数。
                self.transform_expression(operand, bindings, allow_set_state)?;
            }
            // 二元表达式递归改写两侧。
            ExpressionKind::Binary { left, right, .. } => {
                // 改写左侧。
                self.transform_expression(left, bindings, allow_set_state)?;
                // 改写右侧。
                self.transform_expression(right, bindings, allow_set_state)?;
            }
            // 三元表达式递归改写条件与分支。
            ExpressionKind::Ternary {
                // 可变借用条件。
                condition,
                // 可变借用真分支。
                then_branch,
                // 可变借用假分支。
                else_branch,
            } => {
                // 改写条件。
                self.transform_expression(condition, bindings, allow_set_state)?;
                // 改写真分支。
                self.transform_expression(then_branch, bindings, allow_set_state)?;
                // 改写假分支。
                self.transform_expression(else_branch, bindings, allow_set_state)?;
            }
            // 成员访问递归改写对象。
            ExpressionKind::Member { object, .. } => {
                // 改写成员所属对象。
                self.transform_expression(object, bindings, allow_set_state)?;
            }
            // 下标访问递归改写对象与索引。
            ExpressionKind::Index { object, index } => {
                // 改写被索引对象。
                self.transform_expression(object, bindings, allow_set_state)?;
                // 改写索引表达式。
                self.transform_expression(index, bindings, allow_set_state)?;
            }
            // 普通调用递归改写目标与参数。
            ExpressionKind::Call { callee, arguments } => {
                // 改写调用目标。
                self.transform_expression(callee, bindings, allow_set_state)?;
                // 按源码顺序改写参数值。
                for argument in arguments {
                    // 改写当前参数表达式。
                    self.transform_expression(
                        // 可变借用参数值。
                        &mut argument.value,
                        // 使用同一字段绑定。
                        bindings,
                        // 传递状态更新作用域。
                        allow_set_state,
                    )?;
                }
            }
            // 组件 number 语义统一为 f64，整数形态补充小数点。
            ExpressionKind::Number(source) => {
                // 只改写没有小数点或指数的整数形态。
                if !source.contains('.') && !source.contains('e') && !source.contains('E') {
                    // 追加零小数以参与 f64 运算。
                    source.push_str(".0");
                }
            }
            // 字符串字面量已在递归分派前转换为拥有所有权的 String。
            ExpressionKind::String(_) => unreachable!("字符串已在分派前规范化"),
            // 布尔字面量不含组件字段。
            ExpressionKind::Boolean(_) => {}
        }
        // 报告表达式改写成功。
        Ok(())
    }

    // 把 setState 命名参数调用降低为卫生闭包调用。
    fn lower_set_state(
        // 可变借用展开状态。
        &mut self,
        // 接收将被替换的表达式。
        expression: &mut Expression,
        // 接收源码顺序命名参数。
        mut arguments: Vec<CallArgument>,
        // 接收当前组件字段绑定。
        bindings: &Bindings,
    ) -> Result<(), Diagnostic> {
        // setState 至少需要一个更新字段。
        if arguments.is_empty() {
            // 返回空更新诊断。
            return Err(Diagnostic::new(
                // 指向完整调用。
                expression.span,
                // 说明空更新无语义。
                "setState 至少需要一个命名状态参数",
                // 给出规范示例。
                "使用 setState(count: count + 1)",
            ));
        }
        // 保存本次调用的目标名称。
        let mut targets = HashSet::new();
        // 保存每个目标 State 句柄。
        let mut state_idents = Vec::new();
        // 保存更新器闭包参数名称。
        let mut value_idents = Vec::new();
        // 按源码顺序验证并改写更新值。
        for argument in &mut arguments {
            // setState 参数必须具名。
            let name = argument.name.as_deref().ok_or_else(|| {
                // 构造位置参数诊断。
                Diagnostic::new(
                    // 指向当前参数。
                    argument.span,
                    // 说明需要状态名。
                    "setState 不接受位置参数",
                    // 给出规范命名参数。
                    "使用 setState(stateName: value)",
                )
            })?;
            // 同一次调用不能重复写入字段。
            if !targets.insert(name.to_string()) {
                // 返回重复目标诊断。
                return Err(Diagnostic::new(
                    // 指向重复参数。
                    argument.span,
                    // 说明重复状态目标。
                    format!("setState 重复更新状态 {name}"),
                    // 给出合并建议。
                    "每个状态在一次 setState 调用中只声明一个新值",
                ));
            }
            // 查找组件内同名字段。
            let binding = bindings.get(name).ok_or_else(|| {
                // 构造越界状态诊断。
                Diagnostic::new(
                    // 指向当前参数。
                    argument.span,
                    // 说明目标不在状态范围。
                    format!("setState 目标 {name} 不是当前组件的 state 或 State<T> prop"),
                    // 给出合法目标来源。
                    "只更新 state=... 声明的字段或 State<T> props",
                )
            })?;
            // 普通 prop 或回调不能写入。
            let state_name = binding.state_name.as_deref().ok_or_else(|| {
                // 构造只读 prop 诊断。
                Diagnostic::new(
                    // 指向当前参数。
                    argument.span,
                    // 说明普通 prop 只读。
                    format!("setState 不能写入只读 prop {name}"),
                    // 给出共享状态修复建议。
                    format!("把 {name} 声明为 State<T> prop，或改为组件私有 state"),
                )
            })?;
            // 复制名称以在可变改写后继续使用。
            let owned_name = name.to_string();
            // 改写更新值中的字段读取。
            self.transform_expression(&mut argument.value, bindings, false)?;
            // 保存目标 State 句柄。
            state_idents.push(ident_from_name(state_name));
            // 为更新器参数生成卫生名称。
            value_idents.push(self.fresh_ident("set_value", &owned_name));
            // 普通闭包调用必须移除参数名。
            argument.name = None;
        }
        // 为更新器闭包生成卫生名称。
        let setter_ident = self.fresh_ident("set_state", "call");
        // 为每个 State 句柄生成捕获名称。
        let captured_states = state_idents
            // 遍历目标位置。
            .iter()
            // 同时读取位置编号。
            .enumerate()
            // 分配卫生捕获名称。
            .map(|(index, _)| self.fresh_ident("set_state_handle", &index.to_string()))
            // 收集全部捕获名称。
            .collect::<Vec<_>>();
        // 生成更新器闭包并克隆目标句柄。
        self.setup.push(quote! {
            // 每个 setState 表达式持有独立更新器。
            let #setter_ident = {
                // 克隆句柄而不复制底层状态槽。
                #(let #captured_states = #state_idents.clone();)*
                // 所有新值求值后再按顺序提交。
                move |#(#value_idents),*| {
                    // 写回每个私有或共享状态。
                    #(#captured_states.set(#value_idents);)*
                }
            };
        });
        // 用普通位置参数闭包调用替换 AST。
        expression.kind = ExpressionKind::Call {
            // 更新调用目标为卫生更新器。
            callee: Box::new(Expression {
                // 使用普通标识符交给既有生成器。
                kind: ExpressionKind::Identifier(setter_ident.to_string()),
                // 沿用原调用跨度。
                span: expression.span,
            }),
            // 写入已移除名称的参数。
            arguments,
        };
        // 报告 setState 降低成功。
        Ok(())
    }
}

// 把已生成的卫生名称恢复为标识符。
pub(super) fn ident_from_name(name: &str) -> Ident {
    // 使用与 fresh_ident 一致的调用点跨度恢复同一局部绑定。
    Ident::new(name, Span::call_site())
}
