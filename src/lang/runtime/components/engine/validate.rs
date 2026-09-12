use super::*;

pub(super) fn program(
    program: &Program,
    natives: &NativeBindings,
    limits: &ComponentLimits,
) -> RuntimeResult<()> {
    if limits.evaluation.depth == 0
        || limits.evaluation.depth > 64
        || limits.instances == 0
        || limits.nodes == 0
        || limits.events == 0
    {
        return Err(invalid("组件预算无效；深度范围为 1..=64"));
    }
    if program.entry >= program.components.len()
        || program
            .bindings
            .len()
            .saturating_add(program.functions.len())
            .saturating_add(program.components.len())
            > 65536
    {
        return Err(invalid("组件产物入口或规模无效"));
    }
    // 先检查类型图的深度/规模，再做递归比较或克隆。
    for binding in &program.bindings {
        type_shape(&binding.ty)?;
    }
    for function in &program.functions {
        function_shape(&function.signature)?;
    }
    for component in &program.components {
        component_shape(&component.signature)?;
    }
    for export in program.natives.values() {
        export_shape(export)?;
    }
    for native in natives.values() {
        match native {
            NativeBinding::Function { signature, .. } => function_shape(signature)?,
            NativeBinding::Component(signature) => component_shape(signature)?,
            NativeBinding::Type(ty) => type_shape(ty)?,
        }
    }
    for (key, expected) in &program.natives {
        if natives
            .get(key)
            .is_none_or(|native| native.interface() != *expected)
        {
            return Err(RuntimeError::new(
                ErrorKind::CapabilityDenied,
                format!("原生接口缺失或不匹配：{}::{}", key.package, key.name),
            ));
        }
    }
    for (index, binding) in program.bindings.iter().enumerate() {
        match (binding.owner, binding.kind) {
            (Owner::Component(owner), BindingKind::Input)
                if program
                    .components
                    .get(owner)
                    .is_some_and(|c| c.inputs.contains(&index)) => {}
            (Owner::Component(owner), BindingKind::State)
                if program
                    .components
                    .get(owner)
                    .is_some_and(|c| c.states.iter().any(|(id, _)| *id == index))
                    && matches!(binding.ty, Type::Data(_)) => {}
            (Owner::Function(owner), BindingKind::Parameter)
                if program
                    .functions
                    .get(owner)
                    .is_some_and(|f| f.parameters.contains(&index)) => {}
            (Owner::Function(owner), BindingKind::Local) if owner < program.functions.len() => {}
            (Owner::Function(owner), BindingKind::Function(target))
                if owner < program.functions.len()
                    && program
                        .functions
                        .get(target)
                        .is_some_and(|f| binding.ty == Type::Function(f.signature.clone())) => {}
            _ => return Err(invalid("绑定 owner/kind 与声明不符")),
        }
    }
    for (id, function) in program.functions.iter().enumerate() {
        if function.parameters.len() != function.signature.parameters.len()
            || function.defaults.len() != function.parameters.len()
            || function
                .component
                .is_some_and(|c| c >= program.components.len())
        {
            return Err(invalid("函数元数据不完整"));
        }
        let mut seen = BTreeSet::new();
        for (index, binding) in function.parameters.iter().enumerate() {
            let Some(binding) = program
                .bindings
                .get(*binding)
                .filter(|_| seen.insert(*binding))
            else {
                return Err(invalid("参数绑定缺失或重复"));
            };
            if binding.owner != Owner::Function(id)
                || binding.kind != BindingKind::Parameter
                || binding.ty != function.signature.parameters[index]
                || function.defaults[index].is_some()
                    != (index >= function.signature.minimum_arguments)
            {
                return Err(invalid("参数类型或默认值不符"));
            }
        }
        seen.clear();
        for capture in &function.captures {
            if !seen.insert(*capture)
                || !program.bindings.get(*capture).is_some_and(|binding| {
                    matches!(binding.owner,Owner::Function(owner) if owner!=id)
                        && matches!(binding.kind, BindingKind::Parameter | BindingKind::Local)
                })
            {
                return Err(invalid("闭包捕获必须是外层函数的局部值"));
            }
        }
    }
    for (id, component) in program.components.iter().enumerate() {
        if component.inputs.len() != component.signature.parameters.len()
            || component.defaults.len() != component.inputs.len()
        {
            return Err(invalid("组件输入元数据不完整"));
        }
        let Some(render) = program.functions.get(component.render) else {
            return Err(invalid("组件渲染函数不存在"));
        };
        if render.component != Some(id)
            || !render.parameters.is_empty()
            || !render.captures.is_empty()
            || *render.signature.returns != Type::View
            || render.signature.effect > Effect::Query
        {
            return Err(invalid("组件渲染函数合同不符"));
        }
        let mut seen = BTreeSet::new();
        for (index, binding) in component.inputs.iter().enumerate() {
            let Some(binding) = program
                .bindings
                .get(*binding)
                .filter(|_| seen.insert(*binding))
            else {
                return Err(invalid("组件输入绑定缺失或重复"));
            };
            let (name, ty) = &component.signature.parameters[index];
            if binding.name != *name
                || binding.ty != *ty
                || binding.owner != Owner::Component(id)
                || binding.kind != BindingKind::Input
                || component.defaults[index].is_some()
                    == component.signature.required.contains(name)
            {
                return Err(invalid("组件输入合同不符"));
            }
        }
        for (binding, _) in &component.states {
            if !seen.insert(*binding)
                || !program.bindings.get(*binding).is_some_and(|binding| {
                    binding.owner == Owner::Component(id)
                        && binding.kind == BindingKind::State
                        && matches!(binding.ty, Type::Data(_))
                })
            {
                return Err(invalid("组件 state 合同不符"));
            }
        }
    }
    Ok(())
}

fn export_shape(export: &NativeExport) -> RuntimeResult<()> {
    match export {
        NativeExport::Type(ty) => type_shape(ty),
        NativeExport::Function(signature) => function_shape(signature),
        NativeExport::Component(signature) => component_shape(signature),
    }
}
fn function_shape(signature: &FunctionSignature) -> RuntimeResult<()> {
    if signature.minimum_arguments > signature.parameters.len() || signature.parameters.len() > 4096
    {
        return Err(invalid("函数参数合同无效"));
    }
    for ty in signature
        .parameters
        .iter()
        .chain(std::iter::once(signature.returns.as_ref()))
    {
        type_shape(ty)?;
    }
    Ok(())
}
fn component_shape(signature: &ComponentSignature) -> RuntimeResult<()> {
    if signature.parameters.len() > 4096 {
        return Err(invalid("组件输入过多"));
    }
    let mut names = BTreeSet::new();
    for (name, ty) in &signature.parameters {
        if name == "key" || !names.insert(name) {
            return Err(invalid("组件输入重复或占用 key"));
        }
        type_shape(ty)?;
    }
    if signature.required.iter().any(|name| !names.contains(name)) {
        return Err(invalid("必需输入不存在"));
    }
    Ok(())
}
fn type_shape(ty: &Type) -> RuntimeResult<()> {
    enum Part<'a> {
        Rich(&'a Type),
        Data(&'a DataType),
    }
    let mut pending = vec![(Part::Rich(ty), 0usize)];
    let mut count = 0usize;
    while let Some((part, depth)) = pending.pop() {
        count += 1;
        if depth > 64 || count.saturating_add(pending.len()) > 4096 {
            return Err(invalid("类型图超过预算"));
        }
        let added = match &part {
            Part::Rich(Type::Record(fields)) => fields.len(),
            Part::Data(DataType::Record(fields)) => fields.len(),
            Part::Rich(Type::Function(signature)) => signature.parameters.len().saturating_add(1),
            _ => 2,
        };
        if added.saturating_add(count).saturating_add(pending.len()) > 4096 {
            return Err(invalid("类型图超过预算"));
        }
        match part {
            Part::Rich(Type::Data(ty)) => pending.push((Part::Data(ty), depth + 1)),
            Part::Rich(Type::Array(ty)) => pending.push((Part::Rich(ty), depth + 1)),
            Part::Rich(Type::Record(fields)) => {
                pending.extend(fields.values().map(|ty| (Part::Rich(ty), depth + 1)))
            }
            Part::Rich(Type::Function(signature)) => {
                if signature.minimum_arguments > signature.parameters.len() {
                    return Err(invalid("函数参数合同无效"));
                }
                pending.extend(
                    signature
                        .parameters
                        .iter()
                        .chain(std::iter::once(signature.returns.as_ref()))
                        .map(|ty| (Part::Rich(ty), depth + 1)),
                );
            }
            Part::Data(DataType::Array(ty) | DataType::Optional(ty)) => {
                pending.push((Part::Data(ty), depth + 1))
            }
            Part::Data(DataType::Record(fields)) => {
                pending.extend(fields.values().map(|ty| (Part::Data(ty), depth + 1)))
            }
            Part::Data(DataType::Result(ok, err)) => {
                pending.push((Part::Data(ok), depth + 1));
                pending.push((Part::Data(err), depth + 1));
            }
            _ => {}
        }
        if count.saturating_add(pending.len()) > 4096 {
            return Err(invalid("类型图超过预算"));
        }
    }
    Ok(())
}

impl Session<'_> {
    pub(super) fn accept(&self, ty: &Type, value: &Value) -> RuntimeResult<()> {
        self.value_budget(value)?;
        if self.matches(ty, value)? {
            Ok(())
        } else {
            Err(RuntimeError::new(ErrorKind::Type, "组件值与接口类型不符"))
        }
    }
    fn matches(&self, ty: &Type, value: &Value) -> RuntimeResult<bool> {
        Ok(match (ty, value) {
            (Type::Data(ty), Value::Data(value)) => ty.accepts(value),
            (Type::View, Value::View(_)) => true,
            (Type::View, Value::Array(values)) => {
                for value in values.iter() {
                    if !self.matches(ty, value)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Type::Array(ty), Value::Array(values)) => {
                for value in values.iter() {
                    if !self.matches(ty, value)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Type::Record(types), Value::Record(values)) if types.len() == values.len() => {
                for (name, ty) in types {
                    let Some(value) = values.get(name) else {
                        return Ok(false);
                    };
                    if !self.matches(ty, value)? {
                        return Ok(false);
                    }
                }
                true
            }
            (Type::Function(expected), Value::Function(callback)) => {
                let actual = self.callback_signature(callback)?;
                actual.minimum_arguments <= expected.minimum_arguments
                    && actual.parameters == expected.parameters
                    && actual.returns == expected.returns
                    && actual.effect <= expected.effect
            }
            _ => false,
        })
    }
    pub(super) fn value_budget(&self, value: &Value) -> RuntimeResult<()> {
        enum Part<'a> {
            Rich(&'a Value),
            Data(&'a DataValue),
        }
        let limits = &self.limits.evaluation;
        let mut pending = vec![(Part::Rich(value), 0usize)];
        let mut bytes = 0usize;
        let mut items = 0usize;
        let quota = || RuntimeError::new(ErrorKind::Quota, "组件值图超过预算");
        while let Some((part, depth)) = pending.pop() {
            items = items.saturating_add(1);
            bytes = bytes.saturating_add(std::mem::size_of::<Value>());
            if depth > 64 || items.saturating_add(pending.len()) > limits.value_items {
                return Err(quota());
            }
            let added = match &part {
                Part::Rich(Value::Array(values)) => values.len(),
                Part::Rich(Value::Record(fields)) => fields.len(),
                Part::Rich(Value::Function(callback)) => callback.captures.len(),
                Part::Rich(Value::View(elements)) => {
                    elements.iter().fold(elements.len(), |count, element| {
                        count.saturating_add(element.properties.len())
                    })
                }
                Part::Data(DataValue::Array(values)) => values.len(),
                Part::Data(DataValue::Record(fields)) => fields.len(),
                _ => 1,
            };
            if added.saturating_add(items).saturating_add(pending.len()) > limits.value_items {
                return Err(quota());
            }
            match part {
                Part::Rich(Value::Data(value)) => pending.push((Part::Data(value), depth)),
                Part::Rich(Value::Function(callback)) => {
                    self.callback_signature(callback)?;
                    pending.extend(
                        callback
                            .captures
                            .values()
                            .map(|value| (Part::Rich(value), depth + 1)),
                    );
                }
                Part::Rich(Value::View(elements)) => {
                    if elements.len() > self.limits.nodes {
                        return Err(quota());
                    }
                    for element in elements.iter() {
                        if element.engine != self.engine
                            || element
                                .owner
                                .is_some_and(|owner| !self.records.contains_key(&owner))
                        {
                            return Err(RuntimeError::new(
                                ErrorKind::Closed,
                                "View 所属组件已卸载或属于其他 owner",
                            ));
                        }
                        bytes = bytes.saturating_add(std::mem::size_of::<Element>());
                        bytes = bytes.saturating_add(element.location.source.len());
                        if let Some(Key::String(key)) = &element.key {
                            bytes = bytes.saturating_add(key.len());
                        }
                        for (name, value) in &element.properties {
                            bytes = bytes.saturating_add(name.len());
                            pending.push((Part::Rich(value), depth + 1));
                        }
                    }
                }
                Part::Rich(Value::Array(values)) => {
                    pending.extend(values.iter().map(|value| (Part::Rich(value), depth + 1)))
                }
                Part::Rich(Value::Record(fields)) => {
                    for (name, value) in fields.iter() {
                        bytes = bytes.saturating_add(name.len());
                        pending.push((Part::Rich(value), depth + 1));
                    }
                }
                Part::Data(DataValue::String(value)) => bytes = bytes.saturating_add(value.len()),
                Part::Data(DataValue::Bytes(value)) => bytes = bytes.saturating_add(value.len()),
                Part::Data(DataValue::Float(value)) if !value.is_finite() => {
                    return Err(RuntimeError::new(ErrorKind::Type, "浮点必须有限"));
                }
                Part::Data(DataValue::Array(values)) => {
                    pending.extend(values.iter().map(|value| (Part::Data(value), depth + 1)))
                }
                Part::Data(DataValue::Record(fields)) => {
                    for (name, value) in fields {
                        bytes = bytes.saturating_add(name.len());
                        pending.push((Part::Data(value), depth + 1));
                    }
                }
                Part::Data(
                    DataValue::Optional(Some(value))
                    | DataValue::Result(Ok(value))
                    | DataValue::Result(Err(value)),
                ) => pending.push((Part::Data(value), depth + 1)),
                _ => {}
            }
            if bytes > limits.value_bytes
                || items.saturating_add(pending.len()) > limits.value_items
            {
                return Err(quota());
            }
        }
        Ok(())
    }
}
