use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

mod frame;
mod project;
mod validate;
pub use frame::Frame;

#[derive(Clone)]
struct InstanceRecord {
    component: ComponentId,
    supplied: BTreeMap<String, Value>,
    inputs: BTreeMap<BindingId, Value>,
    states: BTreeMap<BindingId, Value>,
    rendered: Option<Value>,
    dirty: bool,
}

/// 同一 owner 上的组件实例树；没有线程、轮询或独立后台 worker。
pub struct Engine {
    id: u64,
    program: Arc<Program>,
    natives: NativeBindings,
    limits: ComponentLimits,
    records: BTreeMap<InstanceId, Arc<InstanceRecord>>,
    identities: BTreeMap<Identity, InstanceId>,
    root_inputs: BTreeMap<String, Value>,
    events: BTreeMap<u64, Arc<Callback>>,
    snapshot: Snapshot,
    next_instance: u64,
    next_revision: u64,
    closed: bool,
}

#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub value: Value,
    pub stats: UpdateStats,
}

impl Engine {
    pub fn new(
        program: Program,
        natives: NativeBindings,
        inputs: BTreeMap<String, Value>,
        limits: ComponentLimits,
    ) -> RuntimeResult<Self> {
        Self::new_with(program, natives, inputs, limits, |_| Ok(())).map(|(engine, _)| engine)
    }
    /// 在提交前准备宿主投影；prepare 只构造候选，不应发布界面或执行业务外部效果。
    pub fn new_with<T>(
        program: Program,
        natives: NativeBindings,
        inputs: BTreeMap<String, Value>,
        limits: ComponentLimits,
        prepare: impl FnOnce(&Snapshot) -> RuntimeResult<T>,
    ) -> RuntimeResult<(Self, T)> {
        validate::program(&program, &natives, &limits)?;
        static NEXT_ENGINE: AtomicU64 = AtomicU64::new(1);
        let mut engine = Self {
            id: NEXT_ENGINE
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| invalid("组件 owner 身份耗尽"))?,
            program: Arc::new(program),
            natives,
            limits,
            records: BTreeMap::new(),
            identities: BTreeMap::new(),
            root_inputs: BTreeMap::new(),
            events: BTreeMap::new(),
            snapshot: Snapshot {
                revision: 0,
                roots: Vec::new(),
                instances: 0,
            },
            next_instance: 1,
            next_revision: 1,
            closed: false,
        };
        let (_, prepared) = engine.update_with(inputs, &Cancellation::default(), prepare)?;
        Ok((engine, prepared))
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn update(
        &mut self,
        inputs: BTreeMap<String, Value>,
        cancellation: &Cancellation,
    ) -> RuntimeResult<UpdateStats> {
        self.update_with(inputs, cancellation, |_| Ok(()))
            .map(|(stats, _)| stats)
    }
    /// 宿主准备失败、panic 或准备后已取消时，不提交组件候选状态与事件。
    pub fn update_with<T>(
        &mut self,
        inputs: BTreeMap<String, Value>,
        cancellation: &Cancellation,
        prepare: impl FnOnce(&Snapshot) -> RuntimeResult<T>,
    ) -> RuntimeResult<(UpdateStats, T)> {
        self.transact(inputs, None, cancellation, prepare)
            .map(|(_, stats, prepared)| (stats, prepared))
    }
    pub fn dispatch(
        &mut self,
        token: EventToken,
        arguments: Vec<Value>,
        cancellation: &Cancellation,
    ) -> RuntimeResult<DispatchResult> {
        self.dispatch_with(token, arguments, cancellation, |_| Ok(()))
            .map(|(result, _)| result)
    }
    /// 事件和宿主候选使用同一提交边界；不回滚已经执行的外部函数。
    pub fn dispatch_with<T>(
        &mut self,
        token: EventToken,
        arguments: Vec<Value>,
        cancellation: &Cancellation,
        prepare: impl FnOnce(&Snapshot) -> RuntimeResult<T>,
    ) -> RuntimeResult<(DispatchResult, T)> {
        self.ensure_open()?;
        if token.engine != self.id || token.revision != self.snapshot.revision {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "事件不属于当前组件投影",
            ));
        }
        let callback = self
            .events
            .get(&token.index)
            .cloned()
            .ok_or_else(|| RuntimeError::new(ErrorKind::UnknownEvent, "事件已卸载或未登记"))?;
        let (value, stats, prepared) = self.transact(
            self.root_inputs.clone(),
            Some((callback, arguments)),
            cancellation,
            prepare,
        )?;
        Ok((DispatchResult { value, stats }, prepared))
    }
    /// 仅供原生挂载适配器保留当前树回调；公开 Snapshot token 仍逐版本失效。
    #[cfg(feature = "ui")]
    pub(super) fn mounted_event(&self, token: EventToken) -> RuntimeResult<Arc<Callback>> {
        self.ensure_open()?;
        if token.engine != self.id || token.revision != self.snapshot.revision {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "挂载事件不属于当前投影",
            ));
        }
        self.events
            .get(&token.index)
            .cloned()
            .ok_or_else(|| RuntimeError::new(ErrorKind::UnknownEvent, "挂载事件不存在"))
    }
    #[cfg(feature = "ui")]
    pub(super) fn dispatch_mounted_with<T>(
        &mut self,
        callback: Arc<Callback>,
        arguments: Vec<Value>,
        cancellation: &Cancellation,
        prepare: impl FnOnce(&Snapshot) -> RuntimeResult<T>,
    ) -> RuntimeResult<(DispatchResult, T)> {
        // Session 仍验证 owner 存活、捕获、参数与效果；UI 适配器另管理事件槽的挂载租期。
        let (value, stats, prepared) = self.transact(
            self.root_inputs.clone(),
            Some((callback, arguments)),
            cancellation,
            prepare,
        )?;
        Ok((DispatchResult { value, stats }, prepared))
    }
    /// 幂等关闭会释放状态、捕获与事件；旧 token 不能恢复关闭的 owner。
    pub fn close(&mut self) {
        self.closed = true;
        self.records.clear();
        self.identities.clear();
        self.events.clear();
        self.root_inputs.clear();
        self.natives.clear();
        self.snapshot.roots.clear();
        self.snapshot.instances = 0;
    }
    fn ensure_open(&self) -> RuntimeResult<()> {
        if self.closed {
            Err(RuntimeError::new(ErrorKind::Closed, "组件 owner 已关闭"))
        } else {
            Ok(())
        }
    }
    fn transact<T>(
        &mut self,
        inputs: BTreeMap<String, Value>,
        event: Option<(Arc<Callback>, Vec<Value>)>,
        cancellation: &Cancellation,
        prepare: impl FnOnce(&Snapshot) -> RuntimeResult<T>,
    ) -> RuntimeResult<(Value, UpdateStats, T)> {
        self.ensure_open()?;
        let revision = self.next_revision;
        self.next_revision = revision
            .checked_add(1)
            .ok_or_else(|| invalid("投影版本耗尽"))?;
        // 候选仅复制 Arc 索引；实际写入/失效的实例按需复制。失败不提交状态或事件表。
        let mut session = Session {
            engine: self.id,
            program: self.program.clone(),
            natives: &self.natives,
            limits: &self.limits,
            records: self.records.clone(),
            identities: self.identities.clone(),
            next_instance: &mut self.next_instance,
            revision,
            events: BTreeMap::new(),
            seen: BTreeSet::new(),
            nodes: 0,
            steps: 0,
            depth: 0,
            started: Instant::now(),
            cancellation,
            stats: UpdateStats::default(),
        };
        session.step(&Location::default())?;
        let result = if let Some((callback, arguments)) = event {
            session.invoke(&callback, arguments, Effect::Command)?
        } else {
            Value::unit()
        };
        let roots = session.component(Vec::new(), session.program.entry, inputs.clone())?;
        session.step(&Location::default())?;
        let before = session.records.len();
        session.records.retain(|id, _| session.seen.contains(id));
        session.identities.retain(|_, id| session.seen.contains(id));
        for callback in session.events.values() {
            session.callback_signature(callback)?;
        }
        session.stats.unmounted_instances = before - session.records.len();
        let snapshot = Snapshot {
            revision,
            roots,
            instances: session.records.len(),
        };
        let prepared =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(&snapshot)))
                .map_err(|_| {
                    RuntimeError::new(ErrorKind::HostFailure, "宿主投影准备 panic；组件候选未提交")
                })??;
        session.step(&Location::default())?;
        self.snapshot = snapshot;
        self.records = session.records;
        self.identities = session.identities;
        self.events = session.events;
        self.root_inputs = inputs;
        Ok((result, session.stats, prepared))
    }
}

struct Session<'env> {
    engine: u64,
    program: Arc<Program>,
    natives: &'env NativeBindings,
    limits: &'env ComponentLimits,
    records: BTreeMap<InstanceId, Arc<InstanceRecord>>,
    identities: BTreeMap<Identity, InstanceId>,
    next_instance: &'env mut u64,
    revision: u64,
    events: BTreeMap<u64, Arc<Callback>>,
    seen: BTreeSet<InstanceId>,
    nodes: usize,
    steps: u64,
    depth: usize,
    started: Instant,
    cancellation: &'env Cancellation,
    stats: UpdateStats,
}

impl Session<'_> {
    fn step(&mut self, location: &Location) -> RuntimeResult<()> {
        self.steps = self.steps.saturating_add(1);
        let error = if self.cancellation.is_cancelled() {
            Some((ErrorKind::Cancelled, "组件操作已取消"))
        } else if self.started.elapsed() > self.limits.evaluation.timeout {
            Some((ErrorKind::Timeout, "组件操作超时"))
        } else if self.steps > self.limits.evaluation.steps {
            Some((ErrorKind::Quota, "组件操作步骤预算耗尽"))
        } else {
            None
        };
        if let Some((kind, message)) = error {
            Err(RuntimeError::new(kind, message).at(location))
        } else {
            Ok(())
        }
    }
    fn callback_signature(&self, callback: &Callback) -> RuntimeResult<&FunctionSignature> {
        if callback.engine != self.engine
            && !(callback.engine == 0
                && callback.owner.is_none()
                && matches!(callback.target, CallbackTarget::Native(_)))
        {
            return Err(RuntimeError::new(
                ErrorKind::CapabilityDenied,
                "回调属于其他组件 owner",
            ));
        }
        if callback
            .owner
            .is_some_and(|owner| !self.records.contains_key(&owner))
        {
            return Err(RuntimeError::new(ErrorKind::Closed, "回调所属组件已卸载"));
        }
        match &callback.target {
            CallbackTarget::Function(id) => self
                .program
                .functions
                .get(*id)
                .map(|function| &function.signature)
                .ok_or_else(|| invalid("回调函数不存在")),
            CallbackTarget::Native(key) => match self.natives.get(key) {
                Some(NativeBinding::Function { signature, .. }) => Ok(signature),
                _ => Err(RuntimeError::new(
                    ErrorKind::CapabilityDenied,
                    "原生函数未开放",
                )),
            },
        }
    }
    fn invoke(
        &mut self,
        callback: &Arc<Callback>,
        arguments: Vec<Value>,
        allowed: Effect,
    ) -> RuntimeResult<Value> {
        self.step(&Location::default())?;
        let signature = self.callback_signature(callback)?.clone();
        if signature.effect > allowed {
            return Err(RuntimeError::new(
                ErrorKind::CapabilityDenied,
                "只读求值不能调用写状态或外部效果",
            ));
        }
        if arguments.len() < signature.minimum_arguments
            || arguments.len() > signature.parameters.len()
        {
            return Err(RuntimeError::new(ErrorKind::Argument, "函数参数数量不符"));
        }
        for (ty, value) in signature.parameters.iter().zip(&arguments) {
            self.accept(ty, value)?;
        }
        if self.depth >= self.limits.evaluation.depth {
            return Err(RuntimeError::new(ErrorKind::Quota, "组件调用深度耗尽"));
        }
        self.depth += 1;
        let result = self.invoke_body(callback, arguments, &signature);
        self.depth -= 1;
        let value = result?;
        self.accept(&signature.returns, &value)?;
        Ok(value)
    }
    fn invoke_body(
        &mut self,
        callback: &Arc<Callback>,
        arguments: Vec<Value>,
        signature: &FunctionSignature,
    ) -> RuntimeResult<Value> {
        match &callback.target {
            CallbackTarget::Native(key) => {
                let Some(NativeBinding::Function { call, .. }) = self.natives.get(key) else {
                    return Err(invalid("原生绑定缺失"));
                };
                let call = call.clone();
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&arguments)))
                        .map_err(|_| {
                        RuntimeError::new(
                            ErrorKind::HostFailure,
                            "原生函数 panic；外部效果可能已发生",
                        )
                    })??;
                self.step(&Location::default())?;
                Ok(result)
            }
            CallbackTarget::Function(id) => {
                let program = self.program.clone();
                let function = &program.functions[*id];
                if let Some(component) = function.component {
                    if !callback
                        .owner
                        .and_then(|owner| self.records.get(&owner))
                        .is_some_and(|instance| instance.component == component)
                    {
                        return Err(invalid("函数捕获的组件类型不匹配"));
                    }
                }
                let mut frame = Frame {
                    session: self,
                    function: Some(*id),
                    owner: callback.owner,
                    captures: callback.captures.clone(),
                    locals: BTreeMap::new(),
                    effect: signature.effect,
                };
                let result = (|| {
                    let mut arguments = arguments.into_iter();
                    for (index, binding) in function.parameters.iter().enumerate() {
                        let value = if let Some(value) = arguments.next() {
                            value
                        } else {
                            let body = function.defaults[index]
                                .as_ref()
                                .ok_or_else(|| invalid("缺少必需函数参数"))?;
                            frame.body(body)?
                        };
                        frame.session.accept(&signature.parameters[index], &value)?;
                        frame.locals.insert(*binding, value);
                    }
                    frame.body(&function.body)
                })();
                result.map_err(|error| error.at(&function.location))
            }
        }
    }
}
