use super::*;

impl Checker<'_> {
    pub(super) fn signatures(&mut self) -> Result<()> {
        let linked = self.linked;
        for unit in linked.units.values() {
            for import in &unit.imports {
                for name in &import.names {
                    if let Some(Binding::NativeImport {
                        package,
                        name: exported,
                    }) = unit.bindings.get(&name.local.text)
                    {
                        self.native(package, exported, unit.source_id, name.imported.span)?;
                    }
                }
            }
            for (index, declaration) in unit.declarations.iter().enumerate() {
                let id = DeclarationId {
                    source_id: unit.source_id,
                    index,
                };
                match &declaration.kind {
                    DeclarationKind::Type(node) => {
                        self.ty(unit.source_id, node, 0)?;
                    }
                    DeclarationKind::Component { parameters, .. } => {
                        self.renderers
                            .insert(NodeId::new(unit.source_id, declaration.span));
                        let mut required = BTreeSet::new();
                        let mut names = BTreeSet::new();
                        let mut types = Vec::new();
                        for parameter in parameters {
                            if parameter.name.text == "key" {
                                return Err(self.error(
                                    unit.source_id,
                                    parameter.name.span,
                                    "component-key",
                                    "key 是实例身份元数据，不能声明为业务参数",
                                ));
                            }
                            if !names.insert(&parameter.name.text) {
                                return Err(self.error(
                                    unit.source_id,
                                    parameter.name.span,
                                    "component-parameter",
                                    "重复组件参数",
                                ));
                            }
                            let ty = self.parameter_type(unit.source_id, parameter)?;
                            if parameter.default.is_none() {
                                required.insert(parameter.name.text.clone());
                            }
                            types.push((parameter.name.text.clone(), ty));
                        }
                        self.components.insert(
                            id,
                            ComponentSignature {
                                parameters: types,
                                required,
                            },
                        );
                    }
                    DeclarationKind::Function(function) => {
                        let callable = NodeId::new(unit.source_id, declaration.span);
                        let signature = self.function_signature(unit.source_id, function)?;
                        self.functions.insert(callable, signature);
                        self.function_effects.insert(callable, Effects::default());
                        self.declared_functions.insert(id, callable);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn parameter_type(
        &mut self,
        source: SourceId,
        parameter: &Parameter,
    ) -> Result<Type> {
        let Some(ty) = &parameter.ty else {
            return Err(self.error(
                source,
                parameter.span,
                "component-parameter-type",
                "此参数需要类型",
            ));
        };
        self.ty(source, ty, 0)
    }
    fn function_signature(
        &mut self,
        source: SourceId,
        function: &Function,
    ) -> Result<FunctionSignature> {
        let mut names = BTreeSet::new();
        let mut parameters = Vec::new();
        for parameter in &function.parameters {
            if !names.insert(&parameter.name.text) {
                return Err(self.error(
                    source,
                    parameter.span,
                    "component-parameter",
                    "重复函数参数",
                ));
            }
            parameters.push(self.parameter_type(source, parameter)?);
        }
        Ok(FunctionSignature {
            minimum_arguments: function
                .parameters
                .iter()
                .rposition(|parameter| parameter.default.is_none())
                .map_or(0, |index| index + 1),
            parameters,
            returns: Box::new(self.ty(source, &function.returns, 0)?),
            effect: Effect::Command,
        })
    }
    fn parameters(
        &mut self,
        scope: &mut Scope,
        parameters: &[Parameter],
        types: &[Type],
    ) -> Result<()> {
        self.observe_hidden_parameters(scope, parameters)?;
        scope.unavailable.extend(
            parameters
                .iter()
                .map(|parameter| parameter.name.text.clone()),
        );
        for (parameter, ty) in parameters.iter().zip(types) {
            if let Some(default) = &parameter.default {
                let fact = self.expr(scope, default, Some(ty))?;
                scope.effects.add(&fact.effects);
                self.require_effect(
                    scope.source,
                    default.span,
                    fact.effects,
                    Effect::Query,
                    "参数默认值不能写状态或调用有外部效果的函数",
                );
            }
            scope.visible_from = parameter.span.end;
            self.declare(
                scope,
                &parameter.name,
                ty.clone(),
                VariableKind::Input,
                None,
            )?;
            scope.unavailable.remove(&parameter.name.text);
        }
        Ok(())
    }
    pub(super) fn declare(
        &mut self,
        scope: &mut Scope,
        name: &Name,
        ty: Type,
        kind: VariableKind,
        callable: Option<Effects>,
    ) -> Result<()> {
        if !scope.locals.insert(name.text.clone()) {
            return Err(self.error(
                scope.source,
                name.span,
                "component-local-name",
                format!("当前作用域重复声明 {}", name.text),
            ));
        }
        let id = NodeId::new(scope.source, name.span);
        self.bindings.insert(
            id,
            BindingFact {
                name: name.text.clone(),
                ty: ty.clone(),
                owner: scope.owner,
                kind: match kind {
                    VariableKind::Input => BindingKind::Input,
                    VariableKind::State => BindingKind::State,
                    VariableKind::Local => BindingKind::Local,
                    VariableKind::Function(_) => BindingKind::Function,
                },
            },
        );
        scope.vars.insert(
            name.text.clone(),
            Variable {
                id,
                owner: scope.owner,
                ty,
                kind,
                callable,
            },
        );
        self.observe_binding(scope, name)?;
        Ok(())
    }

    pub(super) fn bodies(&mut self) -> Result<()> {
        let linked = self.linked;
        for unit in linked.units.values() {
            for (index, declaration) in unit.declarations.iter().enumerate() {
                let id = DeclarationId {
                    source_id: unit.source_id,
                    index,
                };
                let owner = NodeId::new(unit.source_id, declaration.span);
                match &declaration.kind {
                    DeclarationKind::Function(function) => {
                        let signature = self.functions[&owner].clone();
                        let mut scope = Scope::new(owner, Some(*signature.returns.clone()));
                        self.observe_scope(&mut scope, declaration.span)?;
                        self.parameters(&mut scope, &function.parameters, &signature.parameters)?;
                        self.finish_function(&mut scope, &function.body)?;
                    }
                    DeclarationKind::Component { parameters, body } => {
                        let signature = self.components[&id].clone();
                        let mut scope = Scope::new(owner, Some(Type::View));
                        self.observe_scope(&mut scope, declaration.span)?;
                        self.functions.insert(
                            owner,
                            FunctionSignature {
                                minimum_arguments: signature.parameters.len(),
                                parameters: signature
                                    .parameters
                                    .iter()
                                    .map(|(_, ty)| ty.clone())
                                    .collect(),
                                returns: Box::new(Type::View),
                                effect: Effect::Query,
                            },
                        );
                        self.parameters(
                            &mut scope,
                            parameters,
                            &signature
                                .parameters
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect::<Vec<_>>(),
                        )?;
                        let mut state_prefix = true;
                        for member in body {
                            if let ComponentMember::State {
                                name,
                                ty,
                                initial,
                                span,
                            } = member
                            {
                                if !state_prefix {
                                    return Err(self.error(
                                        scope.source,
                                        *span,
                                        "component-state-order",
                                        "实例 state 初始化声明需要位于函数和渲染语句之前",
                                    ));
                                }
                                let ty = self.ty(scope.source, ty, 0)?;
                                if !matches!(ty, Type::Data(_)) {
                                    return Err(self.error(
                                        scope.source,
                                        name.span,
                                        "component-state-type",
                                        "state 只保存拥有型数据，不保存回调或 View",
                                    ));
                                }
                                let value = self.expr(&mut scope, initial, Some(&ty))?;
                                self.require_effect(
                                    scope.source,
                                    initial.span,
                                    value.effects,
                                    Effect::Query,
                                    "state 初始化不能写状态或执行外部效果",
                                );
                                scope.visible_from = initial.span.end;
                                self.declare(&mut scope, name, ty, VariableKind::State, None)?;
                            } else {
                                state_prefix = false;
                            }
                        }
                        // 函数名在组件作用域内可前向引用；函数的局部值捕获仍按声明处词法环境。
                        scope.visible_from = body
                            .iter()
                            .find_map(|member| match member {
                                ComponentMember::State { .. } => None,
                                ComponentMember::Function { span, .. } => Some(span.start),
                                ComponentMember::Statement(statement) => Some(statement.span.start),
                            })
                            .unwrap_or(declaration.span.end);
                        for member in body {
                            if let ComponentMember::Function {
                                name,
                                function,
                                span,
                            } = member
                            {
                                let callable = NodeId::new(scope.source, *span);
                                let signature = self.function_signature(scope.source, function)?;
                                self.functions.insert(callable, signature.clone());
                                self.function_effects.insert(callable, Effects::default());
                                self.declare(
                                    &mut scope,
                                    name,
                                    Type::Function(signature),
                                    VariableKind::Function(callable),
                                    Some(Effects::function(callable)),
                                )?;
                            }
                        }
                        let mut returns = false;
                        for member in body {
                            match member {
                                ComponentMember::State { .. } => {}
                                ComponentMember::Function { function, span, .. } => {
                                    let callable = NodeId::new(scope.source, *span);
                                    let signature = self.functions[&callable].clone();
                                    self.charge(scope.source, *span, scope.vars.len())?;
                                    let mut inner = scope.clone();
                                    inner.owner = callable;
                                    inner.locals.clear();
                                    inner.effects = Effects::default();
                                    inner.returns = Some(*signature.returns.clone());
                                    self.observe_scope(&mut inner, *span)?;
                                    self.parameters(
                                        &mut inner,
                                        &function.parameters,
                                        &signature.parameters,
                                    )?;
                                    self.finish_function(&mut inner, &function.body)?;
                                }
                                ComponentMember::Statement(statement) => {
                                    if returns {
                                        return Err(self.error(
                                            scope.source,
                                            statement.span,
                                            "component-unreachable",
                                            "return 后的渲染语句不可达",
                                        ));
                                    }
                                    returns = self.statement(&mut scope, statement)?;
                                }
                            }
                        }
                        self.finish_scope(&scope);
                        if !returns {
                            return Err(self.error(
                                scope.source,
                                declaration.span,
                                "component-return",
                                "组件所有路径必须返回 View",
                            ));
                        }
                        self.require_effect(
                            scope.source,
                            declaration.span,
                            scope.effects.clone(),
                            Effect::Query,
                            "渲染不能直接或间接写 state、调用事件回调或执行外部效果",
                        );
                        self.function_effects.insert(owner, scope.effects);
                    }
                    DeclarationKind::Type(_) => {}
                }
            }
        }
        Ok(())
    }
    fn finish_function(&mut self, scope: &mut Scope, body: &Block) -> Result<()> {
        let returns = self.statements(scope, &body.statements)?;
        self.finish_scope(scope);
        if scope
            .returns
            .as_ref()
            .is_some_and(|ty| *ty != Type::Data(DataType::Unit))
            && !returns
        {
            return Err(self.error(
                scope.source,
                body.span,
                "component-return",
                "非 Unit 函数的所有路径必须返回值",
            ));
        }
        self.function_effects
            .insert(scope.owner, scope.effects.clone());
        Ok(())
    }
    pub(super) fn statements(
        &mut self,
        scope: &mut Scope,
        statements: &[Statement],
    ) -> Result<bool> {
        let mut returns = false;
        for statement in statements {
            if returns {
                return Err(self.error(
                    scope.source,
                    statement.span,
                    "component-unreachable",
                    "return 后的语句不可达",
                ));
            }
            returns = self.statement(scope, statement)?;
        }
        Ok(returns)
    }
    fn statement(&mut self, scope: &mut Scope, statement: &Statement) -> Result<bool> {
        self.charge(scope.source, statement.span, 1)?;
        match &statement.kind {
            StatementKind::Let { name, ty, value } => {
                let ty = ty
                    .as_ref()
                    .map(|ty| self.ty(scope.source, ty, 0))
                    .transpose()?;
                let fact = self.expr(scope, value, ty.as_ref())?;
                scope.effects.add(&fact.effects);
                scope.visible_from = value.span.end;
                self.declare(
                    scope,
                    name,
                    ty.unwrap_or(fact.ty),
                    VariableKind::Local,
                    fact.callable,
                )?;
            }
            StatementKind::Assign { target, value } => {
                let ExprKind::Name(name) = &target.kind else {
                    return Err(self.error(
                        scope.source,
                        target.span,
                        "component-assignment",
                        "赋值目标需要本地变量或 state 名称；数据集合整体替换",
                    ));
                };
                let variable = scope.vars.get(&name.text).cloned().ok_or_else(|| {
                    self.error(
                        scope.source,
                        target.span,
                        "component-name",
                        "赋值名称未声明",
                    )
                })?;
                if matches!(
                    variable.kind,
                    VariableKind::Input | VariableKind::Function(_)
                ) {
                    return Err(self.error(
                        scope.source,
                        target.span,
                        "component-readonly",
                        "组件/函数输入和函数声明只读",
                    ));
                }
                if matches!(variable.kind, VariableKind::Local) && variable.owner != scope.owner {
                    return Err(self.error(
                        scope.source,
                        target.span,
                        "component-readonly-capture",
                        "捕获的局部值只读；持久交互状态使用 state",
                    ));
                }
                let target_fact = self.expr(scope, target, None)?;
                let fact = self.expr(scope, value, Some(&variable.ty))?;
                scope.effects.add(&target_fact.effects);
                scope.effects.add(&fact.effects);
                if matches!(variable.kind, VariableKind::State) {
                    if self.renderers.contains(&scope.owner) {
                        return Err(self.error(
                            scope.source,
                            target.span,
                            "component-effect",
                            "渲染语句不能写 state",
                        ));
                    }
                    scope.effects.base = Effect::Command;
                }
                if let Some(callable) = fact.callable {
                    scope
                        .vars
                        .get_mut(&name.text)
                        .unwrap()
                        .callable
                        .get_or_insert_with(Effects::default)
                        .add(&callable);
                }
            }
            StatementKind::Evaluate(value) => {
                let fact = self.expr(scope, value, None)?;
                scope.effects.add(&fact.effects);
            }
            StatementKind::Return(value) => {
                let expected = scope.returns.clone();
                let fact = if let Some(value) = value {
                    self.expr(scope, value, expected.as_ref())?
                } else {
                    Fact::pure(Type::Data(DataType::Unit))
                };
                if let Some(expected) = &expected {
                    self.compatible(scope.source, statement.span, &fact, expected)?;
                } else {
                    scope.returns = Some(fact.ty.clone());
                }
                scope.effects.add(&fact.effects);
                return Ok(true);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition = self.expr(scope, condition, Some(&Type::Data(DataType::Bool)))?;
                scope.effects.add(&condition.effects);
                self.charge(
                    scope.source,
                    statement.span,
                    scope.vars.len().saturating_mul(2),
                )?;
                let mut branch = scope.clone();
                branch.locals.clear();
                branch.effects = Effects::default();
                self.observe_scope(&mut branch, then_block.span)?;
                let then_returns = self.statements(&mut branch, &then_block.statements)?;
                self.finish_scope(&branch);
                merge_callable_assignments(scope, &branch);
                scope.effects.add(&branch.effects);
                scope.returns = branch.returns;
                let else_returns = if let Some(block) = else_block {
                    let mut branch = scope.clone();
                    branch.locals.clear();
                    branch.effects = Effects::default();
                    self.observe_scope(&mut branch, block.span)?;
                    let returns = self.statements(&mut branch, &block.statements)?;
                    self.finish_scope(&branch);
                    merge_callable_assignments(scope, &branch);
                    scope.effects.add(&branch.effects);
                    scope.returns = branch.returns;
                    returns
                } else {
                    false
                };
                return Ok(then_returns && else_returns);
            }
        }
        Ok(false)
    }
}

fn merge_callable_assignments(target: &mut Scope, branch: &Scope) {
    for (name, variable) in &branch.vars {
        if let Some(existing) = target.vars.get_mut(name) {
            if existing.id == variable.id {
                if let Some(callable) = &variable.callable {
                    existing
                        .callable
                        .get_or_insert_with(Effects::default)
                        .add(callable);
                }
            }
        }
    }
}
