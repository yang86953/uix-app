// 引入状态目标去重集合。
use std::collections::HashSet;

// 引入过程宏标识符与卫生跨度。
use proc_macro2::{Ident, Span};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器与字段绑定。
use super::component_codegen::{Bindings, ComponentExpander};
// 引入数据类型构造链与根名称识别。
use super::{is_data_constructor_chain, is_registered_data_type};
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
        // 标记句柄位属性：State 字段改写为句柄而非读值。
        handle_mode: bool,
    ) -> Result<(), Diagnostic> {
        // 委托内部实现，组件语义默认规范化字符串与数字。
        self.transform_expression_inner(expression, bindings, allow_set_state, handle_mode, true)
    }

    // 按形状递归改写表达式，normalize_literals 关闭时保持作者字面量形状。
    fn transform_expression_inner(
        // 可变借用展开状态。
        &mut self,
        // 接收待改写表达式。
        expression: &mut Expression,
        // 接收当前字段绑定。
        bindings: &Bindings,
        // 标记当前位置是否允许状态更新。
        allow_set_state: bool,
        // 标记句柄位属性：State 字段改写为句柄而非读值。
        handle_mode: bool,
        // 标记是否把字符串规范化为 String、把整数补为 f64 形状。
        normalize_literals: bool,
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
        // 数据构造链保持字面量形状，生成 &str 参数交给公开 new/成员 API。
        if normalize_literals {
            // 识别需要规范化的字符串节点。
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
        } else if matches!(&expression.kind, ExpressionKind::String(_)) {
            // 保持字面量形状的字符串节点无需改写。
            return Ok(());
        }
        // 按表达式形状递归改写。
        match &mut expression.kind {
            // 组件字段标识符替换为卫生名称。
            ExpressionKind::Identifier(name) => {
                // 查找同名组件字段。
                if let Some(binding) = bindings.get(name) {
                    // 句柄位属性读取可写 State 句柄本身。
                    if handle_mode {
                        // 句柄位只能接受 state 或 State<T> prop。
                        let Some(state_name) = binding.state_name.as_deref() else {
                            // 返回句柄位类型诊断。
                            return Err(Diagnostic::new(
                                // 指向非法引用。
                                expression.span,
                                // 说明只读字段不能提供句柄。
                                format!("该句柄位属性不能引用只读 prop 或回调字段 {name}"),
                                // 给出合法状态来源。
                                "绑定组件私有 state、State<T> prop 或 Rust 侧 State<T>",
                            ));
                        };
                        // 替换为 State 句柄名称。
                        *name = state_name.to_string();
                    } else {
                        // 普通位置替换为读值名称。
                        *name = binding.value_name.clone();
                    }
                } else if !self.is_component_identifier_allowed(name) {
                    // 返回组件封装边界诊断。
                    return Err(Diagnostic::new(
                        // 指向未声明标识符的表达式位置。
                        expression.span,
                        // 说明缺失的外部依赖名称。
                        format!("组件表达式引用了未声明的外部符号 {name}"),
                        // 给出最小声明修复。
                        format!("在 Component 上添加 external=\"{name}\"，或把它声明为 prop/state"),
                    ));
                }
            }
            // 一元表达式递归改写操作数。
            ExpressionKind::Unary { operand, .. } => {
                // 改写内部操作数。
                self.transform_expression_inner(
                    operand,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
            }
            // 二元表达式递归改写两侧。
            ExpressionKind::Binary { left, right, .. } => {
                // 改写左侧。
                self.transform_expression_inner(
                    left,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
                // 改写右侧。
                self.transform_expression_inner(
                    right,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
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
                self.transform_expression_inner(
                    condition,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
                // 改写真分支。
                self.transform_expression_inner(
                    then_branch,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
                // 改写假分支。
                self.transform_expression_inner(
                    else_branch,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
            }
            // 成员访问递归改写对象。
            ExpressionKind::Member { object, .. } => {
                // 改写成员所属对象。
                self.transform_expression_inner(
                    object,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
            }
            // 下标访问递归改写对象与索引。
            ExpressionKind::Index { object, index } => {
                // 改写被索引对象。
                self.transform_expression_inner(
                    object,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
                // 改写索引表达式。
                self.transform_expression_inner(
                    index,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    normalize_literals,
                )?;
            }
            // 调用递归改写目标与参数，数据构造链保持字面量形状。
            ExpressionKind::Call { callee, arguments } => {
                // 已登记数据类型构造链的参数交给公开 &str/枚举 API。
                let chain_normalizes = !is_data_constructor_chain(callee);
                // 改写调用目标。
                self.transform_expression_inner(
                    callee,
                    bindings,
                    allow_set_state,
                    handle_mode,
                    chain_normalizes && normalize_literals,
                )?;
                // 按源码顺序改写参数值。
                for argument in arguments {
                    // 改写当前参数表达式。
                    self.transform_expression_inner(
                        // 可变借用参数值。
                        &mut argument.value,
                        // 使用同一字段绑定。
                        bindings,
                        // 传递状态更新作用域。
                        allow_set_state,
                        // 传递句柄位模式。
                        handle_mode,
                        // 数据构造链参数保持作者字面量形状。
                        chain_normalizes && normalize_literals,
                    )?;
                }
            }
            // 数组字面量递归改写全部元素。
            ExpressionKind::Array(items) => {
                // 按源码顺序改写元素。
                for item in items {
                    // 改写当前元素表达式。
                    self.transform_expression_inner(
                        item,
                        bindings,
                        allow_set_state,
                        handle_mode,
                        normalize_literals,
                    )?;
                }
            }
            // 对象字段值继续复用现有表达式降低规则。
            ExpressionKind::Object(fields) => {
                // 按源码顺序降低全部字段值。
                for field in fields {
                    // 结构属性保留作者数字形状，由专用 codegen 决定目标类型。
                    let authored_numbers = collect_number_sources(&field.value);
                    // 递归改写字段值中的绑定与调用。
                    self.transform_expression_inner(
                        &mut field.value,
                        bindings,
                        allow_set_state,
                        handle_mode,
                        normalize_literals,
                    )?;
                    // 恢复对象字段中被通用组件语义改写的数字源码。
                    restore_number_sources(&mut field.value, &authored_numbers);
                }
            }
            // 组件 number 语义统一为 f64，整数形态补充小数点。
            ExpressionKind::Number(source) => {
                // 数据构造链保持作者数字形状。
                if normalize_literals {
                    // 只改写没有小数点或指数的整数形态。
                    if !source.contains('.') && !source.contains('e') && !source.contains('E') {
                        // 追加零小数以参与 f64 运算。
                        source.push_str(".0");
                    }
                }
            }
            // 字符串字面量已在递归分派前处理。
            ExpressionKind::String(_) => unreachable!("字符串已在分派前处理"),
            // 布尔字面量不含组件字段。
            ExpressionKind::Boolean(_) => {}
        }
        // 报告表达式改写成功。
        Ok(())
    }

    // 判断标识符是否属于当前组件允许的封闭名称集合。
    fn is_component_identifier_allowed(&self, name: &str) -> bool {
        // 文档根不执行 Component external 封装检查。
        let Some(external) = self.external_scope_stack.last() else {
            // 保持公开宏根表达式由 Rust 名称解析负责。
            return true;
        };
        // external 显式白名单直接放行。
        if external.contains(name) {
            // 返回已声明结果。
            return true;
        }
        // 任一嵌套 For 词法作用域可提供局部名称。
        if self
            // 遍历当前嵌套的循环作用域。
            .local_scope_stack
            // 借用作用域迭代器。
            .iter()
            // 查找同名局部变量。
            .any(|scope| scope.contains(name))
        {
            // 返回词法绑定结果。
            return true;
        }
        // UIX 内建、事件占位符与 Option 构造不依赖调用方声明。
        if matches!(
            // 匹配语言保留名称。
            name,
            // 列出当前表达式与专用组件生成器拥有的内建。
            "$event" | "setState" | "setStyle" | "setTheme" | "submitForm" | "Some" | "None"
        ) {
            // 返回内建结果。
            return true;
        }
        // 已登记数据构造器属于语言标准名称表。
        if is_registered_data_type(name) {
            // 返回数据构造器结果。
            return true;
        }
        // 生成器内部卫生标识符不属于作者依赖。
        name.starts_with("__uix_")
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
            // 类型化整数目标需要在改写后恢复作者数字形状。
            let authored_numbers = if binding.authored_numbers {
                // 收集更新值中的原始数字源码。
                collect_number_sources(&argument.value)
            } else {
                // f64/String/bool 目标沿用组件 number 语义。
                Vec::new()
            };
            // 改写更新值中的字段读取。
            self.transform_expression(&mut argument.value, bindings, false, false)?;
            // 类型化整数目标恢复作者数字形状，避免整数字面量被附加小数点。
            if binding.authored_numbers {
                // 按源码跨度恢复更新值中的数字。
                restore_number_sources(&mut argument.value, &authored_numbers);
            }
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

// 收集表达式树中全部数字节点的作者源码。
pub(super) fn collect_number_sources(expression: &Expression) -> Vec<(super::SourceSpan, String)> {
    // 保存按遍历顺序发现的数字。
    let mut sources = Vec::new();
    // 递归收集当前表达式。
    visit_numbers(expression, &mut |number| {
        // 只保存数字节点。
        if let ExpressionKind::Number(source) = &number.kind {
            // 记录稳定跨度与作者源码。
            sources.push((number.span, source.clone()));
        }
    });
    // 返回全部数字来源。
    sources
}

// 按跨度恢复结构属性中的作者数字源码。
pub(super) fn restore_number_sources(
    expression: &mut Expression,
    sources: &[(super::SourceSpan, String)],
) {
    // 递归访问当前表达式中的数字节点。
    visit_numbers_mut(expression, &mut |number| {
        // 只处理数字节点。
        if let ExpressionKind::Number(source) = &mut number.kind {
            // 查找同一源码跨度的作者数字。
            if let Some((_, authored)) = sources.iter().find(|(span, _)| *span == number.span) {
                // 恢复精确作者形状。
                *source = authored.clone();
            }
        }
    });
}

// 递归只读访问表达式树中的全部节点。
fn visit_numbers(expression: &Expression, visitor: &mut impl FnMut(&Expression)) {
    // 先访问当前节点。
    visitor(expression);
    // 再按结构访问子节点。
    match &expression.kind {
        // 一元表达式访问操作数。
        ExpressionKind::Unary { operand, .. } => visit_numbers(operand, visitor),
        // 二元表达式访问两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 访问左侧。
            visit_numbers(left, visitor);
            // 访问右侧。
            visit_numbers(right, visitor);
        }
        // 三元表达式访问条件与两分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 访问条件。
            visit_numbers(condition, visitor);
            // 访问真分支。
            visit_numbers(then_branch, visitor);
            // 访问假分支。
            visit_numbers(else_branch, visitor);
        }
        // 成员访问递归对象。
        ExpressionKind::Member { object, .. } => visit_numbers(object, visitor),
        // 索引访问递归对象与下标。
        ExpressionKind::Index { object, index } => {
            // 访问对象。
            visit_numbers(object, visitor);
            // 访问下标。
            visit_numbers(index, visitor);
        }
        // 调用访问目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 访问调用目标。
            visit_numbers(callee, visitor);
            // 访问全部参数值。
            for argument in arguments {
                // 递归参数值。
                visit_numbers(&argument.value, visitor);
            }
        }
        // 对象访问全部字段值。
        ExpressionKind::Object(fields) => {
            // 按字段顺序访问。
            for field in fields {
                // 递归字段值。
                visit_numbers(&field.value, visitor);
            }
        }
        // 数组访问全部元素。
        ExpressionKind::Array(items) => {
            // 按元素顺序访问。
            for item in items {
                // 递归元素值。
                visit_numbers(item, visitor);
            }
        }
        // 叶节点没有子表达式。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
}

// 递归可变访问表达式树中的全部节点。
fn visit_numbers_mut(expression: &mut Expression, visitor: &mut impl FnMut(&mut Expression)) {
    // 先访问当前节点。
    visitor(expression);
    // 再按结构访问子节点。
    match &mut expression.kind {
        // 一元表达式访问操作数。
        ExpressionKind::Unary { operand, .. } => visit_numbers_mut(operand, visitor),
        // 二元表达式访问两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 访问左侧。
            visit_numbers_mut(left, visitor);
            // 访问右侧。
            visit_numbers_mut(right, visitor);
        }
        // 三元表达式访问条件与两分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 访问条件。
            visit_numbers_mut(condition, visitor);
            // 访问真分支。
            visit_numbers_mut(then_branch, visitor);
            // 访问假分支。
            visit_numbers_mut(else_branch, visitor);
        }
        // 成员访问递归对象。
        ExpressionKind::Member { object, .. } => visit_numbers_mut(object, visitor),
        // 索引访问递归对象与下标。
        ExpressionKind::Index { object, index } => {
            // 访问对象。
            visit_numbers_mut(object, visitor);
            // 访问下标。
            visit_numbers_mut(index, visitor);
        }
        // 调用访问目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 访问调用目标。
            visit_numbers_mut(callee, visitor);
            // 访问全部参数值。
            for argument in arguments {
                // 递归参数值。
                visit_numbers_mut(&mut argument.value, visitor);
            }
        }
        // 对象访问全部字段值。
        ExpressionKind::Object(fields) => {
            // 按字段顺序访问。
            for field in fields {
                // 递归字段值。
                visit_numbers_mut(&mut field.value, visitor);
            }
        }
        // 数组访问全部元素。
        ExpressionKind::Array(items) => {
            // 按元素顺序访问。
            for item in items {
                // 递归元素值。
                visit_numbers_mut(item, visitor);
            }
        }
        // 叶节点没有子表达式。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
}

// 把已生成的卫生名称恢复为标识符。
pub(super) fn ident_from_name(name: &str) -> Ident {
    // 使用与 fresh_ident 一致的调用点跨度恢复同一局部绑定。
    Ident::new(name, Span::call_site())
}
