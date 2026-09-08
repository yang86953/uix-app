//! await 之间共享一个状态工作副本；只有完整同步片段成功后发布。

use super::*;

pub(crate) struct TaskExecution {
    task: usize,
    stage: usize,
    locals: Vec<Value>,
    pub generation: u64,
    pub revision: u64,
    pub started: Instant,
    steps: u64,
    pub location: Location,
    pub await_ordinal: u64,
}
impl TaskExecution {
    pub(crate) fn resume(&mut self, slot: usize, next: usize, value: Value) -> RuntimeResult<()> {
        *self
            .locals
            .get_mut(slot)
            .ok_or_else(|| type_error("异步恢复局部槽无效"))? = value;
        self.stage = next;
        Ok(())
    }
}

impl Instance {
    pub(crate) fn limits(&self) -> &Limits {
        &self.limits
    }
    pub(crate) fn is_async(&self, name: &str) -> bool {
        self.module
            .tasks
            .iter()
            .any(|task| task.signature.name == name)
    }
    pub(crate) fn async_port(&self, name: &str) -> RuntimeResult<AsyncHostPort> {
        self.async_ports
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::new(ErrorKind::CapabilityDenied, "异步端口未授予"))
    }
    pub(crate) fn start_task(
        &self,
        name: &str,
        arguments: Vec<Value>,
        exported: bool,
    ) -> RuntimeResult<TaskExecution> {
        self.ensure_active()?;
        let task = self
            .module
            .tasks
            .iter()
            .position(|task| task.signature.name == name && (!exported || task.exported))
            .ok_or_else(|| {
                RuntimeError::new(ErrorKind::UnknownCommand, format!("未导出异步命令 {name}"))
            })?;
        let definition = &self.module.tasks[task];
        validate_arguments(&definition.signature, &arguments, Effect::Command)?;
        if definition.local_count > self.limits.value_items {
            return Err(quota_error());
        }
        let mut locals = arguments;
        locals.resize(definition.local_count, Value::Unit);
        Value::Array(locals.clone())
            .validate_budget(self.limits.value_bytes, self.limits.value_items)?;
        Ok(TaskExecution {
            task,
            stage: 0,
            locals,
            generation: self.generation,
            revision: self.revision,
            started: Instant::now(),
            steps: 0,
            location: definition
                .stages
                .first()
                .map(|stage| stage.location.clone())
                .unwrap_or_default(),
            await_ordinal: 0,
        })
    }
    pub(crate) fn advance_task(
        &mut self,
        execution: &mut TaskExecution,
        cancellation: &Cancellation,
    ) -> RuntimeResult<TaskStep> {
        self.ensure_active()?;
        if self.generation != execution.generation || self.revision != execution.revision {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "异步恢复所依赖的模块代或状态 revision 已改变",
            )
            .at(&execution.location));
        }
        let mut state = self.state.clone();
        let mut machine = Machine {
            module: &self.module,
            state: &mut state,
            ports: &self.ports,
            limits: &self.limits,
            cancellation,
            steps: execution.steps,
            depth: 0,
            started: execution.started,
            rendering: false,
        };
        let result = (|| {
            loop {
                machine.check()?;
                let task = machine
                    .module
                    .tasks
                    .get(execution.task)
                    .ok_or_else(|| type_error("异步任务无效"))?;
                let stage = task
                    .stages
                    .get(execution.stage)
                    .ok_or_else(|| type_error("异步片段无效"))?
                    .clone();
                execution.location = stage.location.clone();
                let mut frame = Frame {
                    machine: &mut machine,
                    locals: std::mem::take(&mut execution.locals),
                    effect: Effect::Command,
                };
                let step = match &stage.body {
                    TaskBody::Native(run) => run(&mut frame),
                    TaskBody::Dynamic { statements, exit } => match frame.block(statements)? {
                        Some(value) => Ok(TaskStep::Complete(value)),
                        None => frame.task_exit(exit),
                    },
                };
                execution.locals = frame.locals;
                let step = step.map_err(|error| error.at(&stage.location))?;
                machine.value(Value::Array(execution.locals.clone()))?;
                match step {
                    TaskStep::Continue(next) => execution.stage = next,
                    TaskStep::TailCall { name, arguments } => {
                        let index = machine
                            .module
                            .tasks
                            .iter()
                            .position(|task| task.signature.name == name)
                            .ok_or_else(|| type_error("异步事件目标不存在"))?;
                        let target = &machine.module.tasks[index];
                        validate_arguments(&target.signature, &arguments, Effect::Command)?;
                        if target.local_count > machine.limits.value_items {
                            return Err(quota_error());
                        }
                        execution.locals = arguments;
                        execution.locals.resize(target.local_count, Value::Unit);
                        execution.task = index;
                        execution.stage = 0;
                    }
                    TaskStep::Complete(value) => {
                        if !task.signature.returns.accepts(&value) {
                            return Err(type_error("AsyncCommand 返回类型不符"));
                        }
                        machine.value(value.clone())?;
                        break Ok(TaskStep::Complete(value));
                    }
                    step @ TaskStep::Await { .. } => {
                        if let TaskStep::Await {
                            name, arguments, ..
                        } = &step
                        {
                            let declaration = machine
                                .module
                                .ports
                                .iter()
                                .find(|port| port.name == *name && port.asynchronous)
                                .ok_or_else(|| type_error("await 必须等待声明的异步端口"))?;
                            validate_arguments(declaration, arguments, Effect::Command)?;
                            machine.value(Value::Array(arguments.clone()))?;
                        }
                        break Ok(step);
                    }
                }
            }
        })()
        .map_err(|error: RuntimeError| error.at(&execution.location));
        execution.steps = machine.steps;
        let step = result?;
        machine.rendering = true;
        let rendered = if *machine.state != self.state {
            Some(machine.render()?)
        } else {
            None
        };
        machine.check()?;
        execution.steps = machine.steps;
        Value::Array(state.clone())
            .validate_budget(self.limits.value_bytes, self.limits.value_items)?;
        if state != self.state {
            self.revision = self.revision.checked_add(1).ok_or_else(quota_error)?;
            self.state = state;
        }
        if let Some(rendered) = rendered {
            self.rendered = rendered;
        }
        execution.revision = self.revision;
        Ok(step)
    }
}
impl Frame<'_, '_> {
    fn task_exit(&mut self, exit: &TaskExit) -> RuntimeResult<TaskStep> {
        match exit {
            TaskExit::Jump(next) => Ok(TaskStep::Continue(*next)),
            TaskExit::Branch { condition, yes, no } => {
                Ok(TaskStep::Continue(if self.eval(condition)?.as_bool()? {
                    *yes
                } else {
                    *no
                }))
            }
            TaskExit::Await {
                name,
                arguments,
                next,
                slot,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|value| self.eval(value))
                    .collect::<RuntimeResult<Vec<_>>>()?;
                Ok(TaskStep::Await {
                    name: name.clone(),
                    arguments,
                    next: *next,
                    slot: *slot,
                })
            }
            TaskExit::TailCall { name, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(|value| self.eval(value))
                    .collect::<RuntimeResult<Vec<_>>>()?;
                Ok(TaskStep::TailCall {
                    name: name.clone(),
                    arguments,
                })
            }
            TaskExit::Finish => Ok(TaskStep::Complete(Value::Unit)),
        }
    }
}
