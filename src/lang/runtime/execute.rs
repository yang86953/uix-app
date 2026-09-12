//! 实例单写者、动作事务和两种执行方式共同的资源与宿主边界。

use crate::lang::runtime::value::{quota_error, type_error};
use crate::lang::runtime::*;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
mod tasks;
mod view;
pub(crate) use tasks::TaskExecution;
use view::RenderedView;

/// 每次公开调用重新建立的预算。
#[derive(Debug, Clone)]
pub struct Limits {
    pub steps: u64,
    pub depth: usize,
    pub value_bytes: usize,
    pub value_items: usize,
    pub timeout: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            steps: 100_000,
            depth: 64,
            value_bytes: 1_048_576,
            value_items: 65_536,
            timeout: Duration::from_secs(2),
        }
    }
}

/// 调用方可跨线程请求取消；请求不冒充外部效果回滚。
#[derive(Debug, Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
    parent: Option<Arc<AtomicBool>>,
    wake: Arc<Mutex<Option<std::thread::Thread>>>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        self.wake();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
            || self
                .parent
                .as_ref()
                .is_some_and(|p| p.load(Ordering::Acquire))
    }
    pub(crate) fn child(&self) -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            parent: Some(self.flag.clone()),
            wake: self.wake.clone(),
        }
    }
    pub(crate) fn attach_worker(&self) {
        *self.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current());
    }
    pub(crate) fn wake(&self) {
        if let Some(thread) = self.wake.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            thread.unpark();
        }
    }
}

/// 应用持有的运行实例。公开命令失败时不提交模块状态；宿主效果不重放。
pub struct Instance {
    module: Arc<Module>,
    state: Vec<Value>,
    ports: HostPorts,
    async_ports: AsyncHostPorts,
    limits: Limits,
    revision: u64,
    generation: u64,
    closed: bool,
    revoked: bool,
    rendered: RenderedView,
}

impl Instance {
    pub fn new(module: Module, ports: HostPorts, limits: Limits) -> RuntimeResult<Self> {
        Self::new_with_async_ports(module, ports, AsyncHostPorts::new(), limits)
    }

    /// 异步能力独立显式授权，不能用同步端口冒充。
    pub fn new_with_async_ports(
        module: Module,
        ports: HostPorts,
        async_ports: AsyncHostPorts,
        limits: Limits,
    ) -> RuntimeResult<Self> {
        if module.name.is_empty()
            || module.functions.len() > 1024
            || module.states.len() > 1024
            || module.tasks.len() > 1024
        {
            return Err(RuntimeError::new(
                ErrorKind::InvalidModule,
                "模块名称或规模无效",
            ));
        }
        for required in &module.ports {
            let matches = if required.asynchronous {
                async_ports
                    .get(&required.name)
                    .is_some_and(|port| port.signature == *required)
            } else {
                ports
                    .get(&required.name)
                    .is_some_and(|port| port.signature == *required)
            };
            if !matches {
                return Err(RuntimeError::new(
                    ErrorKind::CapabilityDenied,
                    format!("宿主端口 {} 未授予或签名不符", required.name),
                ));
            }
        }
        let state: Vec<_> = module
            .states
            .iter()
            .map(|field| field.initial.clone())
            .collect();
        for (field, value) in module.states.iter().zip(&state) {
            value.validate_budget(limits.value_bytes, limits.value_items)?;
            if !field.ty.accepts(value) {
                return Err(type_error("状态初始值类型不符"));
            }
        }
        Value::Array(state.clone()).validate_budget(limits.value_bytes, limits.value_items)?;
        let mut instance = Self {
            module: Arc::new(module),
            state,
            ports,
            async_ports,
            limits,
            revision: 0,
            generation: 1,
            closed: false,
            revoked: false,
            rendered: RenderedView::default(),
        };
        instance.rendered = instance.render()?;
        Ok(instance)
    }

    pub fn module(&self) -> &Module {
        &self.module
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn state(&self) -> BTreeMap<String, Value> {
        self.module
            .states
            .iter()
            .zip(&self.state)
            .map(|(f, v)| (f.name.clone(), v.clone()))
            .collect()
    }

    pub fn call(&mut self, name: &str, arguments: &[Value]) -> RuntimeResult<Value> {
        self.call_with_cancellation(name, arguments, &Cancellation::default())
    }

    pub fn call_with_cancellation(
        &mut self,
        name: &str,
        arguments: &[Value],
        cancellation: &Cancellation,
    ) -> RuntimeResult<Value> {
        self.ensure_active()?;
        if self
            .module
            .tasks
            .iter()
            .any(|t| t.exported && t.signature.name == name)
        {
            return Err(RuntimeError::new(
                ErrorKind::Argument,
                "AsyncCommand 必须通过 ModuleWorker 投递",
            ));
        }
        if !self
            .module
            .functions
            .iter()
            .any(|f| f.exported && f.signature.name == name)
        {
            return Err(RuntimeError::new(
                ErrorKind::UnknownCommand,
                format!("未导出命令 {name}"),
            ));
        }
        self.invoke(name, arguments, cancellation)
    }

    pub(crate) fn invoke(
        &mut self,
        name: &str,
        arguments: &[Value],
        cancellation: &Cancellation,
    ) -> RuntimeResult<Value> {
        let next_revision = self.revision.checked_add(1).ok_or_else(quota_error)?;
        let mut state = self.state.clone();
        let mut machine = Machine {
            module: &self.module,
            state: &mut state,
            ports: &self.ports,
            limits: &self.limits,
            cancellation,
            steps: 0,
            depth: 0,
            started: Instant::now(),
            rendering: false,
        };
        let result = machine.invoke(name, arguments.to_vec(), Effect::Command)?;
        machine.rendering = true;
        let rendered = if *machine.state != self.state {
            Some(machine.render()?)
        } else {
            None
        };
        machine.check()?;
        Value::Array(state.clone())
            .validate_budget(self.limits.value_bytes, self.limits.value_items)?;
        if state != self.state {
            self.state = state;
            self.revision = next_revision;
        }
        if let Some(rendered) = rendered {
            self.rendered = rendered;
        }
        Ok(result)
    }

    /// 只读返回与状态原子提交的拥有型界面，不重新执行任何模块函数。
    pub fn view(&self) -> RuntimeResult<Option<ViewSnapshot>> {
        self.ensure_active()?;
        Ok(self.rendered.snapshot.clone())
    }

    fn render(&self) -> RuntimeResult<RenderedView> {
        let mut state = self.state.clone();
        let cancellation = Cancellation::default();
        let mut machine = Machine {
            module: &self.module,
            state: &mut state,
            ports: &self.ports,
            limits: &self.limits,
            cancellation: &cancellation,
            steps: 0,
            depth: 0,
            started: Instant::now(),
            rendering: true,
        };
        machine.render()
    }

    /// 事件由当前界面 key 与代际解析，不能借事件调用任意私有函数。
    pub fn event(
        &mut self,
        key: &str,
        generation: u64,
        arguments: &[Value],
        cancellation: &Cancellation,
    ) -> RuntimeResult<Value> {
        let (handler, values) = self.resolve_event(key, generation, arguments)?;
        if self.is_async(&handler) {
            return Err(RuntimeError::new(
                ErrorKind::Argument,
                "异步界面事件必须通过 ModuleWorker 投递",
            ));
        }
        self.invoke(&handler, &values, cancellation)
    }

    pub(crate) fn resolve_event(
        &self,
        key: &str,
        generation: u64,
        arguments: &[Value],
    ) -> RuntimeResult<(String, Vec<Value>)> {
        self.ensure_active()?;
        if generation != self.generation {
            return Err(RuntimeError::new(ErrorKind::Conflict, "界面事件来自旧代"));
        }
        let target =
            self.rendered.events.get(key).ok_or_else(|| {
                RuntimeError::new(ErrorKind::UnknownCommand, "界面事件目标不存在")
            })?;
        if target.disabled {
            return Err(RuntimeError::new(ErrorKind::CapabilityDenied, "按钮已禁用"));
        }
        let handler = target.handler.clone();
        let mut values = target.captures.clone();
        values.extend_from_slice(arguments);
        Ok((handler, values))
    }

    /// 兼容 schema 的候选原子替换；失败保持旧产物、状态和代际。
    pub fn replace(&mut self, candidate: Module, expected_generation: u64) -> RuntimeResult<()> {
        self.replace_cancelled(candidate, expected_generation, &Cancellation::default())
    }
    pub(crate) fn replace_cancelled(
        &mut self,
        candidate: Module,
        expected_generation: u64,
        cancellation: &Cancellation,
    ) -> RuntimeResult<()> {
        self.ensure_active()?;
        if cancellation.is_cancelled() {
            return Err(RuntimeError::new(ErrorKind::Cancelled, "替换已取消"));
        }
        if self.generation != expected_generation || candidate.name != self.module.name {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "模块身份或预期代际不符",
            ));
        }
        let mut next = Self::new_with_async_ports(
            candidate,
            self.ports.clone(),
            self.async_ports.clone(),
            self.limits.clone(),
        )?;
        if next.module.state_schema != self.module.state_schema {
            return Err(RuntimeError::new(ErrorKind::Conflict, "状态 schema 不兼容"));
        }
        let identities = |module: &Module| {
            module
                .dependencies
                .iter()
                .map(|dependency| {
                    (
                        dependency.alias.clone(),
                        dependency.name.clone(),
                        dependency.state_schema,
                    )
                })
                .collect::<std::collections::BTreeSet<_>>()
        };
        if identities(&next.module) != identities(&self.module) {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "依赖别名、模块身份或状态 schema 不兼容",
            ));
        }
        let current = self.state();
        for (field, value) in next.module.states.iter().zip(next.state.iter_mut()) {
            if let Some(existing) = current.get(&field.name) {
                if self
                    .module
                    .states
                    .iter()
                    .find(|old| old.name == field.name)
                    .is_none_or(|old| old.ty != field.ty)
                    || !field.ty.accepts(existing)
                {
                    return Err(type_error("迁移状态类型不兼容"));
                }
                *value = existing.clone();
            }
        }
        Value::Array(next.state.clone())
            .validate_budget(self.limits.value_bytes, self.limits.value_items)?;
        next.generation = self.generation.checked_add(1).ok_or_else(quota_error)?;
        next.revision = self.revision.checked_add(1).ok_or_else(quota_error)?;
        next.rendered = next.render()?;
        if cancellation.is_cancelled() {
            return Err(RuntimeError::new(ErrorKind::Cancelled, "替换已取消"));
        }
        *self = next;
        Ok(())
    }

    pub fn revoke(&mut self) {
        self.revoked = true;
        self.rendered = RenderedView::default();
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.state.clear();
        self.ports.clear();
        self.async_ports.clear();
        self.rendered = RenderedView::default();
    }
    fn ensure_active(&self) -> RuntimeResult<()> {
        if self.closed {
            return Err(RuntimeError::new(ErrorKind::Closed, "实例已关闭"));
        }
        if self.revoked {
            return Err(RuntimeError::new(ErrorKind::CapabilityDenied, "实例已撤权"));
        }
        Ok(())
    }
}

struct Machine<'a> {
    module: &'a Module,
    state: &'a mut Vec<Value>,
    ports: &'a HostPorts,
    limits: &'a Limits,
    cancellation: &'a Cancellation,
    steps: u64,
    depth: usize,
    started: Instant,
    rendering: bool,
}

impl Machine<'_> {
    fn check(&mut self) -> RuntimeResult<()> {
        if self.cancellation.is_cancelled() {
            return Err(RuntimeError::new(ErrorKind::Cancelled, "调用已取消"));
        }
        if self.started.elapsed() >= self.limits.timeout {
            return Err(RuntimeError::new(ErrorKind::Timeout, "调用超时"));
        }
        self.steps = self.steps.checked_add(1).ok_or_else(quota_error)?;
        if self.steps > self.limits.steps || self.depth > self.limits.depth {
            return Err(quota_error());
        }
        Ok(())
    }
    fn value(&self, value: Value) -> RuntimeResult<Value> {
        value.validate_budget(self.limits.value_bytes, self.limits.value_items)?;
        Ok(value)
    }
    fn invoke(
        &mut self,
        name: &str,
        arguments: Vec<Value>,
        allowed: Effect,
    ) -> RuntimeResult<Value> {
        self.check()?;
        if let Some(function) = self
            .module
            .functions
            .iter()
            .find(|f| f.signature.name == name)
        {
            let function = function.clone();
            validate_arguments(&function.signature, &arguments, allowed)?;
            self.value(Value::Array(arguments.clone()))?;
            if self.depth >= self.limits.depth {
                return Err(quota_error());
            }
            self.depth += 1;
            let result = {
                let mut frame = Frame {
                    machine: self,
                    locals: arguments,
                    effect: function.signature.effect,
                };
                frame.locals.resize(function.local_count, Value::Unit);
                match &function.body {
                    Body::Dynamic(body) => frame.block(body).map(|v| v.unwrap_or(Value::Unit)),
                    Body::Native(run) => run(&mut frame),
                }
            };
            self.depth -= 1;
            let result = result.map_err(|e| e.at(&function.location))?;
            if !function.signature.returns.accepts(&result) {
                return Err(type_error("函数返回类型不符").at(&function.location));
            }
            return self.value(result);
        }
        let declaration = self
            .module
            .ports
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| {
                RuntimeError::new(ErrorKind::UnknownCommand, format!("未知调用 {name}"))
            })?;
        if declaration.asynchronous {
            return Err(RuntimeError::new(
                ErrorKind::Argument,
                "异步端口只能通过 await 调用",
            ));
        }
        if self.rendering {
            return Err(RuntimeError::new(
                ErrorKind::CapabilityDenied,
                "界面求值不能调用宿主端口",
            ));
        }
        validate_arguments(declaration, &arguments, allowed)?;
        self.value(Value::Array(arguments.clone()))?;
        let port = self
            .ports
            .get(name)
            .ok_or_else(|| RuntimeError::new(ErrorKind::CapabilityDenied, "端口未授予"))?;
        // 原生宿主回调不支持抢占；panic 隔离并如实报告未知外部效果。
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (port.callback)(&arguments)))
                .map_err(|_| {
                    RuntimeError::new(ErrorKind::HostFailure, "宿主回调 panic，外部效果可能已发生")
                })??;
        self.check()?;
        if !declaration.returns.accepts(&result) {
            return Err(type_error("宿主返回类型不符"));
        }
        self.value(result)
    }
}

fn validate_arguments(signature: &Signature, args: &[Value], allowed: Effect) -> RuntimeResult<()> {
    if signature.effect > allowed {
        return Err(RuntimeError::new(
            ErrorKind::CapabilityDenied,
            "纯求值不能调用有副作用入口",
        ));
    }
    if signature.parameters.len() != args.len()
        || !signature
            .parameters
            .iter()
            .zip(args)
            .all(|((_, t), v)| t.accepts(v))
    {
        return Err(RuntimeError::new(
            ErrorKind::Argument,
            format!("{} 参数类型或数量不符", signature.name),
        ));
    }
    Ok(())
}

/// 生成代码使用的窄执行帧；所有状态、端口与预算仍由实例仲裁。
pub struct Frame<'a, 'env> {
    machine: &'a mut Machine<'env>,
    locals: Vec<Value>,
    effect: Effect,
}

impl Frame<'_, '_> {
    pub fn allocation(&self, bytes: usize) -> RuntimeResult<()> {
        if bytes > self.machine.limits.value_bytes {
            Err(quota_error())
        } else {
            Ok(())
        }
    }
    pub fn step(&mut self, location: &Location) -> RuntimeResult<()> {
        self.machine.check().map_err(|e| e.at(location))
    }
    pub fn checked(&self, value: Value, location: &Location) -> RuntimeResult<Value> {
        self.machine.value(value).map_err(|e| e.at(location))
    }
    pub fn local(&self, slot: usize) -> RuntimeResult<Value> {
        self.locals
            .get(slot)
            .cloned()
            .ok_or_else(|| type_error("局部槽无效"))
    }
    pub fn set_local(&mut self, slot: usize, value: Value) -> RuntimeResult<()> {
        self.machine.value(value.clone())?;
        let target = self
            .locals
            .get_mut(slot)
            .ok_or_else(|| type_error("局部槽无效"))?;
        *target = value;
        self.machine.value(Value::Array(self.locals.clone()))?;
        Ok(())
    }
    pub fn state(&self, slot: usize) -> RuntimeResult<Value> {
        if self.effect == Effect::Pure {
            return Err(type_error("纯函数不能读取实例状态"));
        }
        self.machine
            .state
            .get(slot)
            .cloned()
            .ok_or_else(|| type_error("状态槽无效"))
    }
    pub fn set_state(&mut self, updates: Vec<(usize, Value)>) -> RuntimeResult<()> {
        if self.effect != Effect::Command {
            return Err(type_error("只允许命令更新状态"));
        }
        for (slot, value) in &updates {
            let field = self
                .machine
                .module
                .states
                .get(*slot)
                .ok_or_else(|| type_error("状态槽无效"))?;
            if !field.ty.accepts(value) {
                return Err(type_error("状态更新类型不符"));
            }
        }
        for (slot, value) in updates {
            self.machine.state[slot] = value;
        }
        self.machine
            .value(Value::Array(self.machine.state.clone()))?;
        Ok(())
    }
    pub fn call(&mut self, name: &str, arguments: Vec<Value>) -> RuntimeResult<Value> {
        self.machine.invoke(name, arguments, self.effect)
    }
    pub fn unary(&self, negate: bool, value: Value) -> RuntimeResult<Value> {
        match (negate, value) {
            (false, Value::Bool(v)) => Ok(Value::Bool(!v)),
            (true, Value::Int(v)) => v
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| RuntimeError::new(ErrorKind::Arithmetic, "整数取负溢出")),
            (true, Value::Float(v)) if v.is_finite() => Ok(Value::Float(-v)),
            _ => Err(type_error("一元操作数无效")),
        }
    }
    pub fn member(&self, value: Value, name: &str) -> RuntimeResult<Value> {
        match value {
            Value::Record(v) => v
                .get(name)
                .cloned()
                .ok_or_else(|| type_error("记录字段不存在")),
            _ => Err(type_error("字段访问需要记录")),
        }
    }
    pub fn index(&self, value: Value, index: Value) -> RuntimeResult<Value> {
        let i = usize::try_from(index.as_int()?)
            .map_err(|_| RuntimeError::new(ErrorKind::Bounds, "下标为负"))?;
        let result = match value {
            Value::Array(v) => v.get(i).cloned(),
            Value::Bytes(v) => v.get(i).map(|v| Value::Int(i64::from(*v))),
            Value::String(v) => v.chars().nth(i).map(|v| Value::String(v.to_string())),
            _ => return Err(type_error("下标需要集合或字符串")),
        };
        result.ok_or_else(|| RuntimeError::new(ErrorKind::Bounds, "下标越界"))
    }

    pub fn map(
        &mut self,
        value: Value,
        slot: usize,
        filter: bool,
        mut run: impl FnMut(&mut Self) -> RuntimeResult<Value>,
    ) -> RuntimeResult<Value> {
        let values = value.as_array()?;
        let mut output = Vec::new();
        for value in values {
            self.machine.check()?;
            self.set_local(slot, value.clone())?;
            let result = run(self)?;
            if filter {
                if result.as_bool()? {
                    output.push(value.clone());
                }
            } else {
                output.push(result);
            }
            self.machine.value(Value::Array(output.clone()))?;
        }
        self.machine.value(Value::Array(output))
    }

    pub fn eval(&mut self, expr: &Expr) -> RuntimeResult<Value> {
        self.step(&expr.location)?;
        let value = (|| match &expr.kind {
            ExprKind::Literal(v) => Ok(v.clone()),
            ExprKind::Local(s) => self.local(*s),
            ExprKind::State(s) => self.state(*s),
            ExprKind::Unary { negate, value } => {
                let value = self.eval(value)?;
                self.unary(*negate, value)
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.eval(left)?;
                if *op == Binary::And && !left.as_bool()? {
                    return Ok(Value::Bool(false));
                }
                if *op == Binary::Or && left.as_bool()? {
                    return Ok(Value::Bool(true));
                }
                binary(*op, left, self.eval(right)?)
            }
            ExprKind::Conditional { condition, yes, no } => {
                if self.eval(condition)?.as_bool()? {
                    self.eval(yes)
                } else {
                    self.eval(no)
                }
            }
            ExprKind::Array(values) => values
                .iter()
                .map(|v| self.eval(v))
                .collect::<RuntimeResult<Vec<_>>>()
                .map(Value::Array),
            ExprKind::Record(values) => values
                .iter()
                .map(|(k, v)| self.eval(v).map(|v| (k.clone(), v)))
                .collect::<RuntimeResult<BTreeMap<_, _>>>()
                .map(Value::Record),
            ExprKind::Member { value, name } => {
                let v = self.eval(value)?;
                self.member(v, name)
            }
            ExprKind::Index { value, index } => {
                let v = self.eval(value)?;
                let i = self.eval(index)?;
                self.index(v, i)
            }
            ExprKind::Call { name, arguments } => {
                let args = arguments
                    .iter()
                    .map(|a| self.eval(a))
                    .collect::<RuntimeResult<Vec<_>>>()?;
                if matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") {
                    construct(name, args)
                } else {
                    self.call(name, args)
                }
            }
            ExprKind::Method {
                value,
                name,
                arguments,
            } => {
                let v = self.eval(value)?;
                let args = arguments
                    .iter()
                    .map(|a| self.eval(a))
                    .collect::<RuntimeResult<Vec<_>>>()?;
                self.method(v, name, args)
            }
            ExprKind::Map {
                value,
                slot,
                body,
                filter,
            } => {
                let v = self.eval(value)?;
                self.map(v, *slot, *filter, |frame| frame.eval(body))
            }
        })()
        .map_err(|e| e.at(&expr.location))?;
        self.checked(value, &expr.location)
    }

    fn block(&mut self, statements: &[Statement]) -> RuntimeResult<Option<Value>> {
        for statement in statements {
            self.machine.check()?;
            match statement {
                Statement::Local(slot, value) => {
                    let v = self.eval(value)?;
                    self.set_local(*slot, v)?;
                }
                Statement::State(values) => {
                    let updates = values
                        .iter()
                        .map(|(s, v)| self.eval(v).map(|v| (*s, v)))
                        .collect::<RuntimeResult<Vec<_>>>()?;
                    self.set_state(updates)?;
                }
                Statement::Evaluate(v) => {
                    self.eval(v)?;
                }
                Statement::If(condition, yes, no) => {
                    let branch = if self.eval(condition)?.as_bool()? {
                        yes
                    } else {
                        no
                    };
                    if let Some(v) = self.block(branch)? {
                        return Ok(Some(v));
                    }
                }
                Statement::Return(v) => {
                    return Ok(Some(match v {
                        Some(v) => self.eval(v)?,
                        None => Value::Unit,
                    }));
                }
            }
        }
        Ok(None)
    }
}

/// 显式可选值与结果值构造，错误仍然是业务数据。
pub fn construct(name: &str, mut args: Vec<Value>) -> RuntimeResult<Value> {
    if name == "None" && args.is_empty() {
        return Ok(Value::Optional(None));
    }
    if args.len() != 1 {
        return Err(type_error("构造参数数量无效"));
    }
    let v = Box::new(args.remove(0));
    match name {
        "Some" => Ok(Value::Optional(Some(v))),
        "Ok" => Ok(Value::Result(Ok(v))),
        "Err" => Ok(Value::Result(Err(v))),
        _ => Err(type_error("未知构造")),
    }
}
