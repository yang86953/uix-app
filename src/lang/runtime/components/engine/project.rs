use super::*;

impl Session<'_> {
    pub(super) fn component(
        &mut self,
        identity: Identity,
        component: ComponentId,
        supplied: BTreeMap<String, Value>,
    ) -> RuntimeResult<Vec<NativeNode>> {
        let program = self.program.clone();
        let definition = program
            .components
            .get(component)
            .ok_or_else(|| invalid("组件定义不存在"))?;
        self.path_budget(&identity)?;
        self.step(&definition.location)?;
        self.properties(&definition.signature, &supplied)?;
        let existing = self.identities.get(&identity).copied().filter(|id| {
            self.records
                .get(id)
                .is_some_and(|record| record.component == component)
        });
        let (id, new) = if let Some(id) = existing {
            (id, false)
        } else {
            if self.seen.len() >= self.limits.instances {
                return Err(RuntimeError::new(ErrorKind::Quota, "组件实例预算耗尽"));
            }
            let id = InstanceId(*self.next_instance);
            *self.next_instance = self
                .next_instance
                .checked_add(1)
                .ok_or_else(|| invalid("组件身份耗尽"))?;
            self.identities.insert(identity.clone(), id);
            self.records.insert(
                id,
                Arc::new(InstanceRecord {
                    component,
                    supplied: BTreeMap::new(),
                    inputs: BTreeMap::new(),
                    states: BTreeMap::new(),
                    rendered: None,
                    dirty: true,
                }),
            );
            (id, true)
        };
        if !self.seen.insert(id) {
            return Err(invalid("同一组件身份重复投影"));
        }
        if self.seen.len() > self.limits.instances {
            return Err(RuntimeError::new(ErrorKind::Quota, "组件实例预算耗尽"));
        }
        let changed = new || !same_properties(&self.records[&id].supplied, &supplied);
        if changed {
            // 输入默认值只能看到本轮已经绑定的左侧输入，不能误读上轮后续输入。
            Arc::make_mut(self.records.get_mut(&id).unwrap())
                .inputs
                .clear();
            for (index, binding) in definition.inputs.iter().enumerate() {
                let (name, ty) = &definition.signature.parameters[index];
                let value = if let Some(value) = supplied.get(name) {
                    value.clone()
                } else {
                    let body = definition.defaults[index]
                        .as_ref()
                        .ok_or_else(|| invalid("组件缺少必需输入"))?;
                    self.initializer(id, body)?
                };
                self.accept(ty, &value)?;
                Arc::make_mut(self.records.get_mut(&id).unwrap())
                    .inputs
                    .insert(*binding, value);
            }
            let record = Arc::make_mut(self.records.get_mut(&id).unwrap());
            record.supplied = supplied;
            record.dirty = true;
        }
        if new {
            for (binding, body) in &definition.states {
                let value = self.initializer(id, body)?;
                self.accept(&program.bindings[*binding].ty, &value)?;
                Arc::make_mut(self.records.get_mut(&id).unwrap())
                    .states
                    .insert(*binding, value);
            }
        }
        if self.records[&id].dirty || self.records[&id].rendered.is_none() {
            let callback = Arc::new(Callback {
                engine: self.engine,
                owner: Some(id),
                target: CallbackTarget::Function(definition.render),
                captures: BTreeMap::new(),
            });
            let value = self.invoke(&callback, Vec::new(), Effect::Query)?;
            let record = Arc::make_mut(self.records.get_mut(&id).unwrap());
            record.rendered = Some(value);
            record.dirty = false;
            self.stats.rendered_instances += 1;
        } else {
            self.stats.reused_instances += 1;
        }
        let rendered = self.records[&id].rendered.as_ref().unwrap().clone();
        self.view(&identity, &rendered)
            .map_err(|error| error.at(&definition.location))
    }
    fn initializer(&mut self, owner: InstanceId, body: &Body) -> RuntimeResult<Value> {
        Frame {
            session: self,
            function: None,
            owner: Some(owner),
            captures: BTreeMap::new(),
            locals: BTreeMap::new(),
            effect: Effect::Query,
        }
        .body(body)
    }
    fn path_budget(&self, path: &Identity) -> RuntimeResult<()> {
        if path.len() > self.limits.evaluation.depth {
            Err(RuntimeError::new(ErrorKind::Quota, "组件投影深度耗尽"))
        } else {
            Ok(())
        }
    }
    fn properties(
        &self,
        signature: &ComponentSignature,
        properties: &BTreeMap<String, Value>,
    ) -> RuntimeResult<()> {
        if properties.keys().any(|name| {
            !signature
                .parameters
                .iter()
                .any(|(expected, _)| name == expected)
        }) || signature
            .required
            .iter()
            .any(|name| !properties.contains_key(name))
        {
            return Err(RuntimeError::new(
                ErrorKind::Argument,
                "组件输入缺失或包含未知属性",
            ));
        }
        for (name, ty) in &signature.parameters {
            if let Some(value) = properties.get(name) {
                self.accept(ty, value)?
            }
        }
        Ok(())
    }
    fn view(&mut self, parent: &Identity, value: &Value) -> RuntimeResult<Vec<NativeNode>> {
        self.path_budget(parent)?;
        self.value_budget(value)?;
        let mut pending = vec![value];
        let mut elements = Vec::new();
        while let Some(value) = pending.pop() {
            match value {
                Value::View(values) => elements.extend(values.iter()),
                Value::Array(values) => pending.extend(values.iter().rev()),
                _ => return Err(invalid("投影需要 View")),
            }
        }
        let mut roots = Vec::new();
        let mut keys = BTreeSet::new();
        for (position, element) in elements.into_iter().enumerate() {
            self.step(&element.location)?;
            if element
                .key
                .as_ref()
                .is_some_and(|key| !keys.insert(key.clone()))
            {
                return Err(invalid("同层 View 的 key 重复").at(&element.location));
            }
            let mut identity = parent.clone();
            identity.push(IdentityStep::Element {
                site: element.site,
                key: element.key.clone(),
                position: if element.key.is_some() { 0 } else { position },
            });
            self.path_budget(&identity)
                .map_err(|error| error.at(&element.location))?;
            match &element.target {
                Target::Component(component) => roots.extend(self.component(
                    identity,
                    *component,
                    element.properties.clone(),
                )?),
                Target::Native(export) => {
                    let Some(NativeExport::Component(signature)) = self.program.natives.get(export)
                    else {
                        return Err(RuntimeError::new(
                            ErrorKind::CapabilityDenied,
                            "原生组件未在产物声明",
                        ));
                    };
                    self.properties(signature, &element.properties)?;
                    self.nodes += 1;
                    if self.nodes > self.limits.nodes {
                        return Err(RuntimeError::new(ErrorKind::Quota, "原生节点预算耗尽"));
                    }
                    let mut properties = BTreeMap::new();
                    for (name, value) in &element.properties {
                        let mut slot = identity.clone();
                        slot.push(IdentityStep::Slot(name.clone()));
                        properties.insert(name.clone(), self.project_value(&slot, value)?);
                    }
                    roots.push(NativeNode {
                        location: element.location.clone(),
                        identity,
                        export: export.clone(),
                        properties,
                    });
                }
            }
        }
        Ok(roots)
    }
    fn project_value(&mut self, path: &Identity, value: &Value) -> RuntimeResult<ProjectedValue> {
        self.path_budget(path)?;
        self.step(&Location::default())?;
        Ok(match value {
            Value::Data(value) => ProjectedValue::Data(value.clone()),
            Value::Function(callback) => {
                self.callback_signature(callback)?;
                if self.events.len() >= self.limits.events {
                    return Err(RuntimeError::new(ErrorKind::Quota, "事件预算耗尽"));
                }
                let index = self.events.len() as u64;
                self.events.insert(index, callback.clone());
                ProjectedValue::Event(EventToken {
                    engine: self.engine,
                    revision: self.revision,
                    index,
                })
            }
            Value::View(_) => ProjectedValue::View(self.view(path, value)?),
            Value::Array(values) => {
                let mut output = Vec::new();
                for (index, value) in values.iter().enumerate() {
                    let mut path = path.clone();
                    path.push(IdentityStep::Index(index));
                    output.push(self.project_value(&path, value)?)
                }
                ProjectedValue::Array(output)
            }
            Value::Record(values) => {
                let mut output = BTreeMap::new();
                for (name, value) in values.iter() {
                    let mut path = path.clone();
                    path.push(IdentityStep::Slot(name.clone()));
                    output.insert(name.clone(), self.project_value(&path, value)?);
                }
                ProjectedValue::Record(output)
            }
        })
    }
}
fn same_properties(left: &BTreeMap<String, Value>, right: &BTreeMap<String, Value>) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|(name, value)| right.get(name).is_some_and(|other| value.same(other)))
}
