use super::*;
use crate::lang::runtime::{Binary, binary};

/// 两条路径共用的窄执行帧；生成代码不持有 owner、状态或宿主的可变引用。
pub struct Frame<'a, 'env> {
    pub(super) session: &'a mut Session<'env>,
    pub(super) function: Option<FunctionId>,
    pub(super) owner: Option<InstanceId>,
    pub(super) captures: BTreeMap<BindingId, Value>,
    pub(super) locals: BTreeMap<BindingId, Value>,
    pub(super) effect: Effect,
}

impl Frame<'_, '_> {
    /// AOT 和解释器在相同语义节点进入预算/来源范围，控制流不靠 AOT 解释 IR。
    pub fn scope<T>(
        &mut self,
        location: &Location,
        run: impl FnOnce(&mut Self) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        self.session.step(location)?;
        if self.session.depth >= self.session.limits.evaluation.depth {
            return Err(RuntimeError::new(ErrorKind::Quota, "组件求值深度耗尽").at(location));
        }
        self.session.depth += 1;
        let result = run(self);
        self.session.depth -= 1;
        result.map_err(|error| error.at(location))
    }
    pub(super) fn body(&mut self, body: &Body) -> RuntimeResult<Value> {
        match body {
            Body::Native(run) => {
                let depth = self.session.depth;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(self)))
                    .map_err(|_| {
                        RuntimeError::new(
                            ErrorKind::HostFailure,
                            "原生组件函数 panic；外部效果可能已发生",
                        )
                    });
                self.session.depth = depth;
                result?
            }
            #[cfg(feature = "uix-dynamic")]
            Body::Dynamic(statements) => self
                .block(statements)
                .map(|value| value.unwrap_or_else(Value::unit)),
            #[cfg(all(feature = "lang-build", not(feature = "uix-dynamic")))]
            Body::Dynamic(_) => Err(invalid("IR 执行需要显式 uix-dynamic；AOT 请先生成原生函数")),
        }
    }
    pub fn get(&mut self, id: BindingId) -> RuntimeResult<Value> {
        let binding = self
            .session
            .program
            .bindings
            .get(id)
            .ok_or_else(|| invalid("绑定不存在"))?;
        if let BindingKind::Function(function) = binding.kind {
            return self.closure(function);
        }
        if let Some(value) = self.locals.get(&id).or_else(|| self.captures.get(&id)) {
            return Ok(value.clone());
        }
        let Owner::Component(component) = binding.owner else {
            return Err(invalid(format!("局部值 {} 尚未绑定", binding.name)));
        };
        let record = self.owner_record(component)?;
        match binding.kind {
            BindingKind::Input => record.inputs.get(&id),
            BindingKind::State => record.states.get(&id),
            _ => None,
        }
        .cloned()
        .ok_or_else(|| invalid(format!("组件值 {} 尚未初始化", binding.name)))
    }
    pub fn local(&mut self, id: BindingId, value: Value) -> RuntimeResult<()> {
        let binding = self
            .session
            .program
            .bindings
            .get(id)
            .ok_or_else(|| invalid("局部绑定不存在"))?;
        if !matches!(binding.kind, BindingKind::Local)
            || !matches!(binding.owner, Owner::Function(owner) if Some(owner)==self.function)
        {
            return Err(invalid("不能重定义输入或其他作用域的局部值"));
        }
        self.session.accept(&binding.ty, &value)?;
        self.locals.insert(id, value);
        Ok(())
    }
    pub fn set(&mut self, id: BindingId, value: Value) -> RuntimeResult<()> {
        let binding = self
            .session
            .program
            .bindings
            .get(id)
            .ok_or_else(|| invalid("赋值绑定不存在"))?;
        self.session.accept(&binding.ty, &value)?;
        match (binding.kind, binding.owner) {
            (BindingKind::Local, Owner::Function(owner))
                if Some(owner) == self.function && self.locals.contains_key(&id) =>
            {
                self.locals.insert(id, value);
            }
            (BindingKind::State, Owner::Component(component)) if self.effect == Effect::Command => {
                self.owner_record(component)?;
                let record =
                    Arc::make_mut(self.session.records.get_mut(&self.owner.unwrap()).unwrap());
                let old = record
                    .states
                    .get(&id)
                    .ok_or_else(|| invalid("state 尚未初始化"))?;
                if !old.same(&value) {
                    record.states.insert(id, value);
                    record.dirty = true;
                }
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::CapabilityDenied,
                    "输入/捕获只读，渲染不能写 state",
                ));
            }
        }
        Ok(())
    }
    fn owner_record(&self, component: ComponentId) -> RuntimeResult<&InstanceRecord> {
        let record = self
            .owner
            .and_then(|owner| self.session.records.get(&owner))
            .ok_or_else(|| RuntimeError::new(ErrorKind::Closed, "组件实例已卸载"))?;
        if record.component != component {
            return Err(invalid("绑定与组件实例类型不符"));
        }
        Ok(record)
    }
    pub fn closure(&mut self, function: FunctionId) -> RuntimeResult<Value> {
        let program = self.session.program.clone();
        let definition = program
            .functions
            .get(function)
            .ok_or_else(|| invalid("函数不存在"))?;
        if let Some(component) = definition.component {
            self.owner_record(component)?;
        }
        let mut captures = BTreeMap::new();
        for binding in &definition.captures {
            captures.insert(*binding, self.get(*binding)?);
        }
        let value = Value::Function(Arc::new(Callback {
            engine: self.session.engine,
            owner: self.owner,
            target: CallbackTarget::Function(function),
            captures,
        }));
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn native_function(&self, key: ExportKey) -> RuntimeResult<Value> {
        if !matches!(
            self.session.program.natives.get(&key),
            Some(NativeExport::Function(_))
        ) {
            return Err(RuntimeError::new(
                ErrorKind::CapabilityDenied,
                "原生函数未在产物中声明",
            ));
        }
        Ok(Value::Function(Arc::new(Callback {
            engine: self.session.engine,
            owner: self.owner,
            target: CallbackTarget::Native(key),
            captures: BTreeMap::new(),
        })))
    }
    pub fn call(&mut self, value: Value, arguments: Vec<Value>) -> RuntimeResult<Value> {
        let Value::Function(callback) = value else {
            return Err(invalid("调用需要函数值"));
        };
        self.session.invoke(&callback, arguments, self.effect)
    }
    pub fn unary(&self, negate: bool, value: Value) -> RuntimeResult<Value> {
        Ok(Value::data(match (negate, value.as_data()?) {
            (false, DataValue::Bool(value)) => DataValue::Bool(!value),
            (true, DataValue::Int(value)) => DataValue::Int(
                value
                    .checked_neg()
                    .ok_or_else(|| RuntimeError::new(ErrorKind::Arithmetic, "整数取负溢出"))?,
            ),
            (true, DataValue::Float(value)) if value.is_finite() => DataValue::Float(-value),
            _ => return Err(invalid("一元操作数类型不符")),
        }))
    }
    pub fn binary(&self, op: Binary, left: Value, right: Value) -> RuntimeResult<Value> {
        let value = Value::data(binary(
            op,
            left.as_data()?.clone(),
            right.as_data()?.clone(),
        )?);
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn boolean(&self, value: &Value) -> RuntimeResult<bool> {
        value.as_data()?.as_bool()
    }
    pub fn checked(&self, value: Value) -> RuntimeResult<Value> {
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn array(&self, values: Vec<Value>) -> RuntimeResult<Value> {
        let value = Value::array(values);
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn array_typed(&self, values: Vec<Value>, rich: bool) -> RuntimeResult<Value> {
        let candidate = Value::Array(values.into());
        self.session.value_budget(&candidate)?;
        if rich {
            Ok(candidate)
        } else {
            let Value::Array(values) = candidate else {
                unreachable!()
            };
            self.array(values.to_vec())
        }
    }
    pub fn record(&self, fields: Vec<(String, Value)>) -> RuntimeResult<Value> {
        let count = fields.len();
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        if count != fields.len() {
            return Err(invalid("重复记录字段"));
        }
        let value = Value::record(fields);
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn member(&self, value: Value, name: &str) -> RuntimeResult<Value> {
        match value {
            Value::Data(data) => match data.as_ref() {
                DataValue::Record(fields) => fields
                    .get(name)
                    .cloned()
                    .map(Value::data)
                    .ok_or_else(|| invalid("记录字段不存在")),
                DataValue::Array(values) if name == "length" => {
                    Ok(Value::data(DataValue::Int(values.len() as i64)))
                }
                DataValue::String(value) if name == "length" => {
                    Ok(Value::data(DataValue::Int(value.chars().count() as i64)))
                }
                DataValue::Bytes(value) if name == "length" => {
                    Ok(Value::data(DataValue::Int(value.len() as i64)))
                }
                _ => Err(invalid("数据不提供此字段")),
            },
            Value::Record(fields) => fields
                .get(name)
                .cloned()
                .ok_or_else(|| invalid("记录字段不存在")),
            Value::Array(values) if name == "length" => {
                Ok(Value::data(DataValue::Int(values.len() as i64)))
            }
            _ => Err(invalid("类型不提供此字段")),
        }
    }
    pub fn index(&self, value: Value, index: Value) -> RuntimeResult<Value> {
        let index = usize::try_from(index.as_data()?.as_int()?)
            .map_err(|_| RuntimeError::new(ErrorKind::Bounds, "索引为负"))?;
        let result = match value {
            Value::Data(data) => match data.as_ref() {
                DataValue::Array(values) => values.get(index).cloned().map(Value::data),
                _ => return Err(invalid("索引需要数组")),
            },
            Value::Array(values) => values.get(index).cloned(),
            _ => return Err(invalid("索引需要数组")),
        };
        result.ok_or_else(|| RuntimeError::new(ErrorKind::Bounds, "索引越界"))
    }
    pub fn method(&self, value: Value, name: &str, arguments: Vec<Value>) -> RuntimeResult<Value> {
        let arguments = arguments
            .iter()
            .map(|value| value.as_data().cloned())
            .collect::<RuntimeResult<Vec<_>>>()?;
        let value = super::super::super::data_methods::method_data(
            value.as_data()?.clone(),
            name,
            arguments,
            &self.session.limits.evaluation,
        )?;
        Ok(Value::data(value))
    }
    pub fn map(&mut self, value: Value, callback: Value, filter: bool) -> RuntimeResult<Value> {
        let Value::Function(function) = &callback else {
            return Err(invalid("map/filter 需要函数"));
        };
        let signature = self.session.callback_signature(function)?;
        let rich = if filter {
            matches!(value, Value::Array(_))
        } else {
            !matches!(signature.returns.as_ref(), Type::Data(_))
        };
        let values: Vec<Value> = match value {
            Value::Data(data) => match data.as_ref() {
                DataValue::Array(values) => values.iter().cloned().map(Value::data).collect(),
                _ => return Err(invalid("map/filter 需要数组")),
            },
            Value::Array(values) => values.to_vec(),
            _ => return Err(invalid("map/filter 需要数组")),
        };
        if values.len() > self.session.limits.evaluation.value_items {
            return Err(RuntimeError::new(ErrorKind::Quota, "数组规模超过预算"));
        }
        let mut output = Vec::with_capacity(values.len());
        for value in values {
            let result = self.call(callback.clone(), vec![value.clone()])?;
            if !filter {
                output.push(result);
            } else if self.boolean(&result)? {
                output.push(value);
            }
        }
        self.array_typed(output, rich)
    }
    pub fn element(
        &self,
        location: &Location,
        site: usize,
        target: Target,
        key: Option<Value>,
        properties: Vec<(String, Value)>,
    ) -> RuntimeResult<Value> {
        let key = match key {
            None => None,
            Some(value) => Some(match value.as_data()? {
                DataValue::Int(value) => Key::Int(*value),
                DataValue::String(value) => Key::String(value.clone()),
                _ => return Err(invalid("key 需要 Int 或 String")),
            }),
        };
        let count = properties.len();
        let properties = properties.into_iter().collect::<BTreeMap<_, _>>();
        if count != properties.len() {
            return Err(invalid("重复组件属性"));
        }
        let value = Value::View(
            vec![Element {
                location: location.clone(),
                engine: self.session.engine,
                owner: self.owner,
                site,
                target,
                key,
                properties,
            }]
            .into(),
        );
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn fragment(&self, values: Vec<Value>) -> RuntimeResult<Value> {
        let mut elements = Vec::new();
        let mut pending = values.into_iter().rev().collect::<Vec<_>>();
        while let Some(value) = pending.pop() {
            match value {
                Value::View(value) => elements.extend(value.iter().cloned()),
                Value::Array(value) => pending.extend(value.iter().rev().cloned()),
                _ => return Err(invalid("View 子内容类型不符")),
            }
            if elements.len() + pending.len() > self.session.limits.nodes {
                return Err(RuntimeError::new(ErrorKind::Quota, "View 节点超过预算"));
            }
        }
        let value = Value::View(elements.into());
        self.session.value_budget(&value)?;
        Ok(value)
    }
    pub fn concat(&self, values: Vec<Value>) -> RuntimeResult<Value> {
        let mut output = String::new();
        for value in values {
            let text = value.as_data()?.as_str()?;
            if output.len().saturating_add(text.len()) > self.session.limits.evaluation.value_bytes
            {
                return Err(RuntimeError::new(ErrorKind::Quota, "文本超过预算"));
            }
            output.push_str(text);
        }
        Ok(Value::data(DataValue::String(output)))
    }
}

#[cfg(feature = "uix-dynamic")]
impl Frame<'_, '_> {
    fn block(&mut self, statements: &[ir::Statement]) -> RuntimeResult<Option<Value>> {
        for statement in statements {
            let result = self.scope(&statement.location, |frame| match &statement.kind {
                ir::StatementKind::Let(id, expression) => {
                    let value = frame.eval(expression)?;
                    frame.local(*id, value)?;
                    Ok(None)
                }
                ir::StatementKind::Set(id, expression) => {
                    let value = frame.eval(expression)?;
                    frame.set(*id, value)?;
                    Ok(None)
                }
                ir::StatementKind::Evaluate(expression) => {
                    frame.eval(expression)?;
                    Ok(None)
                }
                ir::StatementKind::Return(expression) => expression
                    .as_ref()
                    .map(|expression| frame.eval(expression))
                    .transpose()
                    .map(|value| Some(value.unwrap_or_else(Value::unit))),
                ir::StatementKind::If(condition, yes, no) => {
                    let condition = frame.eval(condition)?;
                    frame.block(if frame.boolean(&condition)? { yes } else { no })
                }
            })?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }
    fn eval(&mut self, expression: &ir::Expr) -> RuntimeResult<Value> {
        self.scope(&expression.location, |frame| {
            let result = match &expression.kind {
                ir::ExprKind::Constant(value) => Ok(Value::data(value.clone())),
                ir::ExprKind::Get(id) => frame.get(*id),
                ir::ExprKind::Closure(id) => frame.closure(*id),
                ir::ExprKind::NativeFunction(key) => frame.native_function(key.clone()),
                ir::ExprKind::Unary { negate, value } => {
                    let value = frame.eval(value)?;
                    frame.unary(*negate, value)
                }
                ir::ExprKind::Binary { op, left, right } => {
                    let left = frame.eval(left)?;
                    if (*op == Binary::And && !frame.boolean(&left)?)
                        || (*op == Binary::Or && frame.boolean(&left)?)
                    {
                        Ok(left)
                    } else {
                        let right = frame.eval(right)?;
                        frame.binary(*op, left, right)
                    }
                }
                ir::ExprKind::Conditional { condition, yes, no } => {
                    let condition = frame.eval(condition)?;
                    frame.eval(if frame.boolean(&condition)? { yes } else { no })
                }
                ir::ExprKind::Array { values, rich } => {
                    let values = values
                        .iter()
                        .map(|value| frame.eval(value))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.array_typed(values, *rich)
                }
                ir::ExprKind::Record(fields) => {
                    let fields = fields
                        .iter()
                        .map(|(name, value)| Ok((name.clone(), frame.eval(value)?)))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.record(fields)
                }
                ir::ExprKind::Member { value, name } => {
                    let value = frame.eval(value)?;
                    frame.member(value, name)
                }
                ir::ExprKind::Index { value, index } => {
                    let value = frame.eval(value)?;
                    let index = frame.eval(index)?;
                    frame.index(value, index)
                }
                ir::ExprKind::Call { callee, arguments } => {
                    let callee = frame.eval(callee)?;
                    let arguments = arguments
                        .iter()
                        .map(|value| frame.eval(value))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.call(callee, arguments)
                }
                ir::ExprKind::Method {
                    value,
                    name,
                    arguments,
                } => {
                    let value = frame.eval(value)?;
                    let arguments = arguments
                        .iter()
                        .map(|value| frame.eval(value))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.method(value, name, arguments)
                }
                ir::ExprKind::Map {
                    value,
                    callback,
                    filter,
                } => {
                    let value = frame.eval(value)?;
                    let callback = frame.eval(callback)?;
                    frame.map(value, callback, *filter)
                }
                ir::ExprKind::Element {
                    site,
                    target,
                    attributes,
                } => {
                    let mut key = None;
                    let mut properties = Vec::new();
                    for attribute in attributes {
                        match attribute {
                            ir::Attribute::Key(value) => key = Some(frame.eval(value)?),
                            ir::Attribute::Property(name, value) => {
                                properties.push((name.clone(), frame.eval(value)?))
                            }
                        }
                    }
                    frame.element(&expression.location, *site, target.clone(), key, properties)
                }
                ir::ExprKind::Fragment(values) => {
                    let values = values
                        .iter()
                        .map(|value| frame.eval(value))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.fragment(values)
                }
                ir::ExprKind::Concat(values) => {
                    let values = values
                        .iter()
                        .map(|value| frame.eval(value))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    frame.concat(values)
                }
            }?;
            frame.session.value_budget(&result)?;
            Ok(result)
        })
    }
}
