use super::*;

impl Checker<'_> {
    pub(super) fn expr(
        &mut self,
        scope: &mut Scope,
        expression: &Expr,
        expected: Option<&Type>,
    ) -> Result<Fact> {
        self.charge(scope.source, expression.span, 1)?;
        let fact = self.expression(scope, expression, expected)?;
        self.observe_progress(scope, expression.span.end);
        self.charge_type(scope.source, expression.span, &fact.ty)?;
        if self.renderers.contains(&scope.owner)
            && (fact.effects.base > Effect::Pure || !fact.effects.calls.is_empty())
        {
            self.require_effect(
                scope.source,
                expression.span,
                fact.effects.clone(),
                Effect::Query,
                "渲染表达式不能直接或间接写 state、调用事件回调或执行外部效果",
            );
        }
        if let Some(expected) = expected {
            self.compatible(scope.source, expression.span, &fact, expected)?;
        }
        self.record_expression_names(scope.source, expression, &fact);
        self.expressions
            .insert(NodeId::new(scope.source, expression.span), fact.clone());
        Ok(fact)
    }

    fn expression(
        &mut self,
        scope: &mut Scope,
        expression: &Expr,
        expected: Option<&Type>,
    ) -> Result<Fact> {
        use runtime::Value;
        let span = expression.span;
        let literal = |ty, value| {
            let mut fact = Fact::pure(Type::Data(ty));
            fact.constant = Some(value);
            fact
        };
        Ok(match &expression.kind {
            ExprKind::Unit => literal(DataType::Unit, Value::Unit),
            ExprKind::Bool(value) => literal(DataType::Bool, Value::Bool(*value)),
            ExprKind::Integer(value) => {
                let value = i64::try_from(*value).map_err(|_| {
                    self.error(
                        scope.source,
                        span,
                        "component-integer",
                        "正整数字面量超出 i64 范围",
                    )
                })?;
                literal(DataType::Int, Value::Int(value))
            }
            ExprKind::Float(value) => {
                if !value.is_finite() {
                    return Err(self.error(
                        scope.source,
                        span,
                        "component-float",
                        "Float 必须有限",
                    ));
                }
                literal(DataType::Float, Value::Float(*value))
            }
            ExprKind::String(value) => literal(DataType::String, Value::String(value.clone())),
            ExprKind::Name(name) => self.name(scope, name)?,
            ExprKind::Unary {
                op: UnaryOp::Negate,
                value,
            } if matches!(value.kind, ExprKind::Integer(v) if v == i64::MAX as u64 + 1) => {
                literal(DataType::Int, Value::Int(i64::MIN))
            }
            ExprKind::Unary { op, value } => {
                let value = self.expr(scope, value, None)?;
                let valid = match op {
                    UnaryOp::Not => value.ty == Type::Data(DataType::Bool),
                    UnaryOp::Negate => {
                        matches!(value.ty, Type::Data(DataType::Int | DataType::Float))
                    }
                };
                if !valid {
                    return Err(self.error(
                        scope.source,
                        span,
                        "component-unary",
                        "一元运算类型不符",
                    ));
                }
                Fact {
                    ty: value.ty,
                    effects: value.effects,
                    ..Fact::pure(Type::Data(DataType::Unit))
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.expr(scope, left, None)?;
                let right = self.expr(scope, right, None)?;
                let (Type::Data(left_ty), Type::Data(right_ty)) = (&left.ty, &right.ty) else {
                    return Err(self.error(
                        scope.source,
                        span,
                        "component-binary",
                        "数据运算不接受回调或 View",
                    ));
                };
                let ty =
                    runtime::binary_signature(binary(*op), left_ty, right_ty).ok_or_else(|| {
                        self.error(
                            scope.source,
                            span,
                            "component-binary",
                            "二元运算的类型不匹配或不支持此运算",
                        )
                    })?;
                let mut fact = Fact::pure(Type::Data(ty));
                fact.effects.add(&left.effects);
                fact.effects.add(&right.effects);
                fact
            }
            ExprKind::Conditional { condition, yes, no } => {
                let condition = self.expr(scope, condition, Some(&Type::Data(DataType::Bool)))?;
                let yes = self.expr(scope, yes, expected)?;
                let no = self.expr(scope, no, Some(&yes.ty))?;
                let mut fact = Fact::pure(yes.ty);
                fact.effects.add(&condition.effects);
                fact.effects.add(&yes.effects);
                fact.effects.add(&no.effects);
                fact.callable = yes.callable;
                if let Some(callable) = no.callable {
                    fact.callable
                        .get_or_insert_with(Effects::default)
                        .add(&callable);
                }
                fact
            }
            ExprKind::Array(values) => {
                let mut element = expected.and_then(Type::element);
                let mut fact = Fact::pure(Type::Data(DataType::Unit));
                for value in values {
                    let value = self.expr(scope, value, element.as_ref())?;
                    fact.effects.add(&value.effects);
                    element = Some(value.ty);
                }
                let element = element.ok_or_else(|| {
                    self.error(
                        scope.source,
                        span,
                        "component-array",
                        "空数组需要元素类型上下文",
                    )
                })?;
                fact.ty = Type::array(element);
                fact
            }
            ExprKind::Record(fields) => {
                let expected_fields = expected.and_then(Type::fields);
                let mut types = BTreeMap::new();
                let mut fact = Fact::pure(Type::Data(DataType::Unit));
                for (name, value) in fields {
                    let value = self.expr(
                        scope,
                        value,
                        expected_fields
                            .as_ref()
                            .and_then(|fields| fields.get(&name.text)),
                    )?;
                    if types.insert(name.text.clone(), value.ty).is_some() {
                        return Err(self.error(
                            scope.source,
                            name.span,
                            "component-record",
                            "重复记录字段",
                        ));
                    }
                    fact.effects.add(&value.effects);
                }
                fact.ty = Type::record(types);
                fact
            }
            ExprKind::Member { value, name } => {
                let value = self.expr(scope, value, None)?;
                let ty = if let Some(fields) = value.ty.fields() {
                    fields.get(&name.text).cloned().ok_or_else(|| {
                        self.error(
                            scope.source,
                            name.span,
                            "component-member",
                            format!("未知记录字段 {}", name.text),
                        )
                    })?
                } else if name.text == "length"
                    && (value.ty.element().is_some()
                        || matches!(value.ty, Type::Data(DataType::String | DataType::Bytes)))
                {
                    Type::Data(DataType::Int)
                } else {
                    return Err(self.error(
                        scope.source,
                        name.span,
                        "component-member",
                        "类型不提供此字段",
                    ));
                };
                let mut fact = Fact::pure(ty);
                fact.effects = value.effects;
                if name.text == "length" && value.ty.fields().is_none() {
                    fact.resolution = Some(ResolvedName::Length);
                }
                if let Type::Function(signature) = &fact.ty {
                    fact.callable = Some(Effects::known(signature.effect));
                }
                fact
            }
            ExprKind::Index { value, index } => {
                let value = self.expr(scope, value, None)?;
                let index = self.expr(scope, index, Some(&Type::Data(DataType::Int)))?;
                let ty = value.ty.element().ok_or_else(|| {
                    self.error(scope.source, span, "component-index", "索引需要数组")
                })?;
                let mut fact = Fact::pure(ty);
                fact.effects.add(&value.effects);
                fact.effects.add(&index.effects);
                if let Type::Function(signature) = &fact.ty {
                    fact.callable = Some(Effects::known(signature.effect));
                }
                fact
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Member { value, name } = &callee.kind {
                    // 记录中的函数字段仍走普通函数值调用，不按字符串方法名截获。
                    let receiver = self.expr(scope, value, None)?;
                    if receiver.ty.fields().is_none() {
                        return self.method(scope, expression, callee, receiver, name, arguments);
                    }
                }
                let callee = self.expr(scope, callee, None)?;
                let Type::Function(signature) = &callee.ty else {
                    return Err(self.error(scope.source, span, "component-call", "调用需要函数值"));
                };
                if arguments.len() < signature.minimum_arguments
                    || arguments.len() > signature.parameters.len()
                {
                    return Err(self.error(
                        scope.source,
                        span,
                        "component-call-arity",
                        "函数参数数量不符",
                    ));
                }
                let mut fact = Fact::pure(*signature.returns.clone());
                fact.effects.add(&callee.effects);
                for (argument, ty) in arguments.iter().zip(&signature.parameters) {
                    let argument = self.expr(scope, argument, Some(ty))?;
                    fact.effects.add(&argument.effects);
                }
                fact.effects.add(
                    &callee
                        .callable
                        .unwrap_or_else(|| Effects::known(signature.effect)),
                );
                if let Type::Function(signature) = &fact.ty {
                    fact.callable = Some(Effects::known(signature.effect));
                }
                fact
            }
            ExprKind::Lambda { parameters, body } => {
                let expected = match expected {
                    Some(Type::Function(signature)) => Some(signature),
                    _ => None,
                };
                self.lambda(
                    scope,
                    expression.span,
                    parameters,
                    body,
                    expected.map(|signature| signature.parameters.as_slice()),
                    expected.map(|signature| signature.returns.as_ref()),
                )?
            }
            ExprKind::Element(element) => self.element(scope, span, element)?,
            ExprKind::Fragment(children) => {
                let effects = self.children(scope, children, &Type::View)?;
                let mut fact = Fact::pure(Type::View);
                fact.effects = effects;
                fact
            }
        })
    }

    fn name(&mut self, scope: &Scope, name: &Name) -> Result<Fact> {
        if scope.unavailable.contains(&name.text) {
            return Err(self.error(
                scope.source,
                name.span,
                "component-parameter-order",
                "默认值不能读取当前或后继尚未绑定的参数",
            ));
        }
        if let Some(variable) = scope.vars.get(&name.text) {
            let mut fact = Fact::pure(variable.ty.clone());
            if matches!(variable.kind, VariableKind::State)
                || (matches!(variable.kind, VariableKind::Input)
                    && self.renderers.contains(&variable.owner))
            {
                fact.effects.base = Effect::Query;
            }
            if variable.owner != scope.owner {
                self.captures
                    .entry(scope.owner)
                    .or_default()
                    .insert(variable.id);
            }
            fact.callable = variable.callable.clone();
            if fact.callable.is_none() {
                if let Type::Function(signature) = &fact.ty {
                    fact.callable = Some(Effects::known(signature.effect));
                }
            }
            fact.resolution = Some(match variable.kind {
                VariableKind::Function(id) => ResolvedName::Function(id),
                _ => ResolvedName::Binding(variable.id),
            });
            return Ok(fact);
        }
        let binding = self.linked.units[&scope.source]
            .bindings
            .get(&name.text)
            .cloned()
            .ok_or_else(|| {
                self.error(
                    scope.source,
                    name.span,
                    "component-name",
                    format!("未声明名称 {}；不捕获 Rust 作用域", name.text),
                )
            })?;
        let (signature, callable, resolution) = match binding {
            Binding::Declaration(id) => {
                let callable = self.declared_functions.get(&id).copied().ok_or_else(|| {
                    self.error(
                        scope.source,
                        name.span,
                        "component-value",
                        "类型不是值；组件使用 UI 表达式构造",
                    )
                })?;
                (
                    self.functions[&callable].clone(),
                    Effects::function(callable),
                    ResolvedName::Function(callable),
                )
            }
            Binding::NativeImport {
                package,
                name: exported,
            } => {
                let NativeExport::Function(signature) =
                    self.native(&package, &exported, scope.source, name.span)?
                else {
                    return Err(self.error(
                        scope.source,
                        name.span,
                        "component-value",
                        "原生导出不是函数值",
                    ));
                };
                (
                    signature.clone(),
                    Effects::known(signature.effect),
                    ResolvedName::NativeFunction {
                        package,
                        name: exported,
                    },
                )
            }
        };
        let mut fact = Fact::pure(Type::Function(signature));
        fact.callable = Some(callable);
        fact.resolution = Some(resolution);
        Ok(fact)
    }

    fn lambda(
        &mut self,
        scope: &Scope,
        span: Span,
        parameters: &[Parameter],
        body: &LambdaBody,
        expected_parameters: Option<&[Type]>,
        expected_return: Option<&Type>,
    ) -> Result<Fact> {
        if expected_parameters.is_some_and(|types| types.len() != parameters.len()) {
            return Err(self.error(
                scope.source,
                span,
                "component-lambda-arity",
                "回调参数数量与接口不一致",
            ));
        }
        self.charge(scope.source, span, scope.vars.len())?;
        let owner = NodeId::new(scope.source, span);
        let mut inner = scope.clone();
        inner.owner = owner;
        inner.locals.clear();
        inner.effects = Effects::default();
        inner.returns = expected_return.cloned();
        self.observe_scope(&mut inner, span)?;
        self.observe_hidden_parameters(&inner, parameters)?;
        let mut types = Vec::new();
        for (index, parameter) in parameters.iter().enumerate() {
            if parameter.default.is_some() {
                return Err(self.error(
                    scope.source,
                    parameter.span,
                    "component-lambda-default",
                    "lambda 参数不提供默认值",
                ));
            }
            let expected = expected_parameters.and_then(|parameters| parameters.get(index));
            let ty = if let Some(node) = &parameter.ty {
                self.ty(scope.source, node, 0)?
            } else {
                expected.cloned().ok_or_else(|| {
                    self.error(
                        scope.source,
                        parameter.span,
                        "component-lambda-type",
                        "回调参数需要类型或调用点上下文",
                    )
                })?
            };
            if let Some(expected) = expected {
                self.compatible(
                    scope.source,
                    parameter.span,
                    &Fact::pure(ty.clone()),
                    expected,
                )?;
            }
            inner.visible_from = parameter.span.end;
            self.declare(
                &mut inner,
                &parameter.name,
                ty.clone(),
                VariableKind::Input,
                None,
            )?;
            inner.unavailable.remove(&parameter.name.text);
            types.push(ty);
        }
        let returns = match body {
            LambdaBody::Expression(expression) => {
                let fact = self.expr(&mut inner, expression, expected_return)?;
                inner.effects.add(&fact.effects);
                self.finish_scope(&inner);
                fact.ty
            }
            LambdaBody::Block(block) => {
                let all_return = self.statements(&mut inner, &block.statements)?;
                self.finish_scope(&inner);
                let returns = inner.returns.unwrap_or(Type::Data(DataType::Unit));
                if returns != Type::Data(DataType::Unit) && !all_return {
                    return Err(self.error(
                        scope.source,
                        block.span,
                        "component-return",
                        "回调的所有路径必须返回值",
                    ));
                }
                returns
            }
        };
        let signature = FunctionSignature {
            minimum_arguments: types.len(),
            parameters: types,
            returns: Box::new(returns),
            effect: Effect::Command,
        };
        self.functions.insert(owner, signature.clone());
        self.function_effects.insert(owner, inner.effects);
        let mut fact = Fact::pure(Type::Function(signature));
        fact.callable = Some(Effects::function(owner));
        fact.resolution = Some(ResolvedName::Function(owner));
        Ok(fact)
    }

    fn method(
        &mut self,
        scope: &mut Scope,
        expression: &Expr,
        callee: &Expr,
        receiver: Fact,
        name: &Name,
        arguments: &[Expr],
    ) -> Result<Fact> {
        let mut effects = receiver.effects;
        let (ty, signature) = if matches!(name.text.as_str(), "map" | "filter") {
            let element = receiver.ty.element().ok_or_else(|| {
                self.error(
                    scope.source,
                    name.span,
                    "component-map",
                    "map/filter 需要数组",
                )
            })?;
            if arguments.len() != 1 {
                return Err(self.error(
                    scope.source,
                    expression.span,
                    "component-call-arity",
                    "map/filter 需要一个回调",
                ));
            }
            let argument = &arguments[0];
            let expected_return = (name.text == "filter").then_some(Type::Data(DataType::Bool));
            let callback = if let ExprKind::Lambda { parameters, body } = &argument.kind {
                let callback = self.lambda(
                    scope,
                    argument.span,
                    parameters,
                    body,
                    Some(&[element.clone()]),
                    expected_return.as_ref(),
                )?;
                self.expressions
                    .insert(NodeId::new(scope.source, argument.span), callback.clone());
                callback
            } else {
                self.expr(scope, argument, None)?
            };
            let Type::Function(signature) = &callback.ty else {
                return Err(self.error(
                    scope.source,
                    argument.span,
                    "component-map",
                    "map/filter 需要函数值",
                ));
            };
            if signature.parameters != [element]
                || expected_return
                    .as_ref()
                    .is_some_and(|ty| ty != signature.returns.as_ref())
            {
                return Err(self.error(
                    scope.source,
                    argument.span,
                    "component-map",
                    "数组回调的参数或返回类型不符",
                ));
            }
            effects.add(&callback.effects);
            effects.add(
                &callback
                    .callable
                    .clone()
                    .unwrap_or_else(|| Effects::known(signature.effect)),
            );
            let ty = if name.text == "filter" {
                receiver.ty
            } else {
                Type::array(*signature.returns.clone())
            };
            let method_signature = FunctionSignature {
                minimum_arguments: 1,
                parameters: vec![callback.ty.clone()],
                returns: Box::new(ty.clone()),
                effect: Effect::Pure,
            };
            (ty, method_signature)
        } else {
            let Type::Data(data_type) = &receiver.ty else {
                return Err(self.error(
                    scope.source,
                    name.span,
                    "component-method",
                    "该类型不提供普通数据方法",
                ));
            };
            let (parameters, returns) = runtime::method_signature(data_type, &name.text)
                .ok_or_else(|| {
                    self.error(
                        scope.source,
                        name.span,
                        "component-method",
                        format!("类型不提供方法 {}", name.text),
                    )
                })?;
            if parameters.len() != arguments.len() {
                return Err(self.error(
                    scope.source,
                    expression.span,
                    "component-call-arity",
                    "方法参数数量不符",
                ));
            }
            for (argument, ty) in arguments.iter().zip(&parameters) {
                let argument = self.expr(scope, argument, Some(&Type::Data(ty.clone())))?;
                effects.add(&argument.effects);
            }
            let ty = Type::Data(returns);
            let signature = FunctionSignature {
                minimum_arguments: parameters.len(),
                parameters: parameters.into_iter().map(Type::Data).collect(),
                returns: Box::new(ty.clone()),
                effect: Effect::Pure,
            };
            (ty, signature)
        };
        let mut member = Fact::pure(Type::Function(signature));
        member.resolution = Some(ResolvedName::Method(name.text.clone()));
        self.record_expression_names(scope.source, callee, &member);
        self.expressions
            .insert(NodeId::new(scope.source, callee.span), member);
        let mut fact = Fact::pure(ty);
        fact.effects = effects;
        fact.resolution = Some(ResolvedName::Method(name.text.clone()));
        Ok(fact)
    }

    fn element(&mut self, scope: &mut Scope, span: Span, element: &Element) -> Result<Fact> {
        if element.name.len() != 1 {
            return Err(self.error(
                scope.source,
                span,
                "component-element",
                "使用显式导入的组件名称",
            ));
        }
        let name = &element.name[0];
        if scope.vars.contains_key(&name.text) {
            return Err(self.error(
                scope.source,
                name.span,
                "component-element",
                "此名称被局部值遮蔽，不是组件导出",
            ));
        }
        let binding = self.linked.units[&scope.source]
            .bindings
            .get(&name.text)
            .cloned()
            .ok_or_else(|| {
                self.error(
                    scope.source,
                    name.span,
                    "component-element",
                    format!("未导入组件 {}", name.text),
                )
            })?;
        let (signature, resolution) = match binding {
            Binding::Declaration(id) => (
                self.components.get(&id).cloned().ok_or_else(|| {
                    self.error(
                        scope.source,
                        name.span,
                        "component-element",
                        "此导出不是组件",
                    )
                })?,
                ResolvedName::Component(id),
            ),
            Binding::NativeImport {
                package,
                name: exported,
            } => {
                let NativeExport::Component(signature) =
                    self.native(&package, &exported, scope.source, name.span)?
                else {
                    return Err(self.error(
                        scope.source,
                        name.span,
                        "component-element",
                        "此原生导出不是组件",
                    ));
                };
                (
                    signature.clone(),
                    ResolvedName::NativeComponent {
                        package,
                        name: exported,
                    },
                )
            }
        };
        let fields = signature
            .parameters
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        self.observe_progress(scope, name.span.end);
        let mut provided = BTreeSet::new();
        let mut fact = Fact::pure(Type::View);
        fact.resolution = Some(resolution);
        for attribute in &element.attributes {
            let name = &attribute.name;
            if !provided.insert(name.text.clone()) {
                return Err(self.error(
                    scope.source,
                    name.span,
                    "component-property",
                    "重复组件属性",
                ));
            }
            let value = if name.text == "key" {
                let value = self.expr(scope, &attribute.value, None)?;
                if !matches!(value.ty, Type::Data(DataType::Int | DataType::String)) {
                    return Err(self.error(
                        scope.source,
                        attribute.value.span,
                        "component-key",
                        "key 需要 Int 或 String",
                    ));
                }
                value
            } else {
                let ty = fields.get(&name.text).ok_or_else(|| {
                    self.error(
                        scope.source,
                        name.span,
                        "component-property",
                        format!("组件未声明属性 {}", name.text),
                    )
                })?;
                self.expr(scope, &attribute.value, Some(ty))?
            };
            fact.effects.add(&value.effects);
        }
        self.observe_progress(scope, element.opening_span.end);
        let has_children = (!element.children.is_empty()
            && fields.get("children") == Some(&Type::Data(DataType::String)))
            || element.children.iter().any(
                |child| !matches!(child, ViewChild::Text { value, .. } if value.trim().is_empty()),
            );
        if has_children {
            if !provided.insert("children".into()) {
                return Err(self.error(
                    scope.source,
                    span,
                    "component-children",
                    "children 属性和标签子内容不能同时提供",
                ));
            }
            let ty = fields.get("children").ok_or_else(|| {
                self.error(
                    scope.source,
                    span,
                    "component-children",
                    "此组件不接收子内容",
                )
            })?;
            fact.effects
                .add(&self.children(scope, &element.children, ty)?);
        }
        if let Some(missing) = signature
            .required
            .iter()
            .find(|name| !provided.contains(*name))
        {
            return Err(self.error(
                scope.source,
                span,
                "component-required-property",
                format!("组件缺少必需输入 {missing}"),
            ));
        }
        Ok(fact)
    }

    fn children(
        &mut self,
        scope: &mut Scope,
        children: &[ViewChild],
        ty: &Type,
    ) -> Result<Effects> {
        if *ty != Type::View && *ty != Type::Data(DataType::String) {
            return Err(self.error(
                scope.source,
                Span {
                    start: scope.owner.start,
                    end: scope.owner.end,
                },
                "component-children-type",
                "标签子内容需要 View 或 String 类型；其他命名内容使用属性表达式",
            ));
        }
        let mut effects = Effects::default();
        for child in children {
            match child {
                ViewChild::Text { value, span } => {
                    if *ty == Type::View && !value.trim().is_empty() {
                        return Err(self.error(
                            scope.source,
                            *span,
                            "component-view-text",
                            "View 子内容中的文字需要明确的文字组件",
                        ));
                    }
                }
                ViewChild::Expression(expression) => {
                    let fact = self.expr(scope, expression, Some(ty))?;
                    effects.add(&fact.effects);
                }
            }
        }
        Ok(effects)
    }
}

fn binary(op: BinaryOp) -> runtime::Binary {
    use runtime::Binary as B;
    match op {
        BinaryOp::Or => B::Or,
        BinaryOp::And => B::And,
        BinaryOp::Equal => B::Equal,
        BinaryOp::NotEqual => B::NotEqual,
        BinaryOp::Less => B::Less,
        BinaryOp::LessEqual => B::LessEqual,
        BinaryOp::Greater => B::Greater,
        BinaryOp::GreaterEqual => B::GreaterEqual,
        BinaryOp::Add => B::Add,
        BinaryOp::Subtract => B::Subtract,
        BinaryOp::Multiply => B::Multiply,
        BinaryOp::Divide => B::Divide,
        BinaryOp::Remainder => B::Remainder,
    }
}
