//! 有界命令与异步续体。实例由一个 worker 持有；完成/取消唤醒不依赖空闲轮询。

use crate::async_port::CompletionSlot;
use crate::execute::TaskExecution;
use crate::value::quota_error;
use crate::*;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicU8, AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

/// sequence 是已终止请求的最大序号；pending_sequences 明确排除仍在途请求。
#[derive(Debug, Clone)]
pub struct ModuleSnapshot {
    pub name: String,
    pub generation: u64,
    pub revision: u64,
    pub sequence: u64,
    pub pending_sequences: Vec<u64>,
    pub state: BTreeMap<String, Value>,
    pub view: Option<ViewSnapshot>,
    pub error: Option<RuntimeError>,
    pub active: bool,
}
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

struct Shared {
    snapshot: Mutex<ModuleSnapshot>,
    next: Mutex<u64>,
    lifecycle: AtomicU8,
    cancellation: Cancellation,
    wake: Arc<dyn Fn() + Send + Sync>,
    limits: Limits,
    instance_id: u64,
}
impl Shared {
    fn notify(&self) {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.wake)())).is_err() {
            let mut snapshot = lock(&self.snapshot);
            if snapshot.active {
                snapshot.error = Some(RuntimeError::new(
                    ErrorKind::HostFailure,
                    "宿主唤醒回调 panic",
                ));
            }
        }
    }
    fn check(&self) -> RuntimeResult<()> {
        match self.lifecycle.load(Ordering::Acquire) {
            0 => Ok(()),
            1 => Err(RuntimeError::new(ErrorKind::CapabilityDenied, "实例已撤权")),
            _ => Err(RuntimeError::new(ErrorKind::Closed, "实例已关闭")),
        }
    }
    fn publish(
        &self,
        instance: &Instance,
        pending: &BTreeMap<u64, Pending>,
        sequence: Option<u64>,
        error: Option<RuntimeError>,
    ) {
        {
            let mut snapshot = lock(&self.snapshot);
            if self.check().is_err() {
                return;
            }
            if let Some(sequence) = sequence {
                snapshot.sequence = snapshot.sequence.max(sequence);
            }
            snapshot.pending_sequences = pending.keys().copied().collect();
            snapshot.generation = instance.generation();
            snapshot.revision = instance.revision();
            snapshot.state = instance.state();
            match instance.view() {
                Ok(view) => {
                    snapshot.view = view;
                    snapshot.error = error;
                }
                Err(error) => snapshot.error = Some(error),
            }
        }
        self.notify();
    }
    fn begin_await(&self, instance: &Instance, sequence: u64) -> RuntimeResult<()> {
        {
            let mut snapshot = lock(&self.snapshot);
            self.check()?;
            snapshot.generation = instance.generation();
            snapshot.revision = instance.revision();
            snapshot.state = instance.state();
            snapshot.view = instance.view()?;
            snapshot.error = None;
            if !snapshot.pending_sequences.contains(&sequence) {
                snapshot.pending_sequences.push(sequence);
                snapshot.pending_sequences.sort_unstable();
            }
        }
        self.notify();
        Ok(())
    }
}
enum Operation {
    Call(String, Vec<Value>),
    Event(String, u64, Vec<Value>),
    Replace(Module, u64),
}
struct Receipt {
    sequence: u64,
    cancellation: Cancellation,
    reply: mpsc::SyncSender<RuntimeResult<Value>>,
}
struct Request {
    operation: Operation,
    receipt: Receipt,
}

/// 丢弃回执不取消请求，wait 超时也不冒充业务取消。
pub struct ModuleRequest {
    pub sequence: u64,
    pub cancellation: Cancellation,
    reply: mpsc::Receiver<RuntimeResult<Value>>,
}
impl ModuleRequest {
    pub fn wait(self, timeout: Duration) -> RuntimeResult<Value> {
        self.reply
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => {
                    RuntimeError::new(ErrorKind::Timeout, "等待超时；请求仍可能执行，可显式取消")
                }
                mpsc::RecvTimeoutError::Disconnected => {
                    RuntimeError::new(ErrorKind::Closed, "执行队列已关闭")
                }
            })?
    }
}

#[derive(Clone)]
pub struct ModuleHandle {
    sender: mpsc::SyncSender<Request>,
    shared: Arc<Shared>,
}
impl ModuleHandle {
    pub fn snapshot(&self) -> ModuleSnapshot {
        lock(&self.shared.snapshot).clone()
    }
    pub fn call(&self, name: &str, arguments: Vec<Value>) -> RuntimeResult<ModuleRequest> {
        self.submit(Operation::Call(name.into(), arguments))
    }
    pub fn event(
        &self,
        key: &str,
        generation: u64,
        arguments: Vec<Value>,
    ) -> RuntimeResult<ModuleRequest> {
        self.submit(Operation::Event(key.into(), generation, arguments))
    }
    pub fn replace(&self, candidate: Module, generation: u64) -> RuntimeResult<ModuleRequest> {
        self.submit(Operation::Replace(candidate, generation))
    }
    pub fn revoke(&self) {
        self.shared.lifecycle.fetch_max(1, Ordering::AcqRel);
        self.shared.cancellation.cancel();
        self.mark_terminal();
    }
    pub fn close(&self) {
        self.shared.lifecycle.store(2, Ordering::Release);
        self.shared.cancellation.cancel();
        self.mark_terminal();
    }
    fn mark_terminal(&self) {
        {
            let mut snapshot = lock(&self.shared.snapshot);
            snapshot.active = false;
            snapshot.view = None;
            snapshot.state.clear();
            snapshot.pending_sequences.clear();
            snapshot.error = self.shared.check().err();
        }
        self.shared.cancellation.wake();
        self.shared.notify();
    }
    fn submit(&self, operation: Operation) -> RuntimeResult<ModuleRequest> {
        let result: RuntimeResult<ModuleRequest> = (|| {
            self.shared.check()?;
            if let Operation::Call(_, arguments) | Operation::Event(_, _, arguments) = &operation {
                let mut bytes = 0;
                let mut items = 0;
                for value in arguments {
                    let (used_bytes, used_items) = value.budget_usage(
                        self.shared.limits.value_bytes.saturating_sub(bytes),
                        self.shared.limits.value_items.saturating_sub(items),
                    )?;
                    bytes += used_bytes;
                    items += used_items;
                }
            }
            let mut next = lock(&self.shared.next);
            let sequence = next.checked_add(1).ok_or_else(quota_error)?;
            let (reply, receiver) = mpsc::sync_channel(1);
            let cancellation = self.shared.cancellation.child();
            self.sender
                .try_send(Request {
                    operation,
                    receipt: Receipt {
                        sequence,
                        cancellation: cancellation.clone(),
                        reply,
                    },
                })
                .map_err(|error| match error {
                    mpsc::TrySendError::Full(_) => {
                        RuntimeError::new(ErrorKind::Quota, "模块命令队列已满")
                    }
                    mpsc::TrySendError::Disconnected(_) => {
                        RuntimeError::new(ErrorKind::Closed, "模块命令队列已关闭")
                    }
                })?;
            *next = sequence;
            self.shared.cancellation.wake();
            Ok(ModuleRequest {
                sequence,
                cancellation,
                reply: receiver,
            })
        })();
        if let Err(error) = &result {
            lock(&self.shared.snapshot).error = Some(error.clone());
            self.shared.notify();
        }
        result
    }
}

/// capacity 分别限制等待请求和挂起任务；Drop 只请求停止，close 等待真实退出。
pub struct ModuleWorker {
    handle: ModuleHandle,
    done: mpsc::Receiver<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl ModuleWorker {
    pub fn spawn(
        instance: Instance,
        capacity: usize,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> RuntimeResult<Self> {
        if !(1..=1024).contains(&capacity) {
            return Err(RuntimeError::new(
                ErrorKind::Argument,
                "队列容量必须在 1 到 1024 之间",
            ));
        }
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_INSTANCE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| quota_error())?;
        let snapshot = ModuleSnapshot {
            name: instance.module().name.clone(),
            generation: instance.generation(),
            revision: instance.revision(),
            sequence: 0,
            pending_sequences: vec![],
            state: instance.state(),
            view: instance.view()?,
            error: None,
            active: true,
        };
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(snapshot),
            next: Mutex::new(0),
            lifecycle: AtomicU8::new(0),
            cancellation: Cancellation::default(),
            wake: Arc::new(wake),
            limits: instance.limits().clone(),
            instance_id,
        });
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let (done_sender, done) = mpsc::sync_channel(1);
        let owner = shared.clone();
        let thread = std::thread::Builder::new()
            .name("uix-module".into())
            .spawn(move || {
                owner.cancellation.attach_worker();
                run(instance, receiver, &owner, capacity);
                // 等待者可能已经离开；实际退出不依赖通知被接收。
                let _ = done_sender.send(());
            })
            .map_err(|error| {
                RuntimeError::new(ErrorKind::HostFailure, format!("模块线程启动失败：{error}"))
            })?;
        Ok(Self {
            handle: ModuleHandle { sender, shared },
            done,
            thread: Some(thread),
        })
    }
    pub fn handle(&self) -> ModuleHandle {
        self.handle.clone()
    }
    pub fn close(mut self, timeout: Duration) -> RuntimeResult<()> {
        self.handle.close();
        self.done.recv_timeout(timeout).map_err(|_| {
            RuntimeError::new(ErrorKind::Timeout, "模块线程尚未退出；已禁止发布结果")
        })?;
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| RuntimeError::new(ErrorKind::HostFailure, "模块线程 panic"))?;
        }
        Ok(())
    }
}
impl Drop for ModuleWorker {
    fn drop(&mut self) {
        self.handle.close();
    }
}

struct Waiting {
    slot: Arc<CompletionSlot>,
    next: usize,
    local: usize,
}
struct Pending {
    receipt: Receipt,
    execution: TaskExecution,
    waiting: Option<Waiting>,
}
impl Pending {
    fn terminal(&self, instance: &Instance) -> Option<RuntimeError> {
        let kind = if self.receipt.cancellation.is_cancelled() {
            Some((ErrorKind::Cancelled, "异步任务已取消"))
        } else if self.execution.generation != instance.generation() {
            Some((ErrorKind::Conflict, "异步任务所属模块已替换"))
        } else if self.execution.started.elapsed() >= instance.limits().timeout {
            Some((ErrorKind::Timeout, "异步任务总等待超时"))
        } else {
            None
        };
        kind.map(|(kind, message)| RuntimeError::new(kind, message).at(&self.execution.location))
    }
    fn stop(&mut self) {
        self.receipt.cancellation.cancel();
        if let Some(waiting) = self.waiting.take() {
            waiting.slot.close();
        }
    }
}

fn finish(
    instance: &Instance,
    shared: &Shared,
    pending: &BTreeMap<u64, Pending>,
    receipt: Receipt,
    result: RuntimeResult<Value>,
) {
    let result = shared.check().and(result);
    shared.publish(
        instance,
        pending,
        Some(receipt.sequence),
        result.as_ref().err().cloned(),
    );
    drop(receipt.reply.send(result));
}

fn drive(
    instance: &mut Instance,
    shared: &Shared,
    task: &mut Pending,
) -> RuntimeResult<Option<Value>> {
    if let Some(error) = task.terminal(instance) {
        return Err(error);
    }
    if let Some(waiting) = task.waiting.take() {
        let result = waiting
            .slot
            .take()
            .ok_or_else(|| RuntimeError::new(ErrorKind::HostFailure, "异步完成事实缺失"))?;
        task.execution.resume(
            waiting.local,
            waiting.next,
            result.map_err(|error| error.at(&task.execution.location))?,
        )?;
    }
    match instance.advance_task(&mut task.execution, &task.receipt.cancellation)? {
        TaskStep::Complete(value) => Ok(Some(value)),
        TaskStep::Await {
            name,
            arguments,
            next,
            slot,
        } => {
            shared.check()?;
            if let Some(error) = task.terminal(instance) {
                return Err(error);
            }
            let port = instance.async_port(&name)?;
            task.execution.await_ordinal = task
                .execution
                .await_ordinal
                .checked_add(1)
                .ok_or_else(quota_error)?;
            let (completion, handle) = CompletionSlot::new(
                task.receipt.cancellation.clone(),
                port.signature.returns.clone(),
                instance.limits().clone(),
            );
            task.waiting = Some(Waiting {
                slot: completion,
                next,
                local: slot,
            });
            shared.begin_await(instance, task.receipt.sequence)?;
            shared.check()?;
            if let Some(error) = task.terminal(instance) {
                return Err(error);
            }
            let call = AsyncCall {
                operation: OperationId {
                    instance: shared.instance_id,
                    generation: task.execution.generation,
                    request: task.receipt.sequence,
                    await_ordinal: task.execution.await_ordinal,
                },
                arguments,
                cancellation: task.receipt.cancellation.clone(),
                completion: handle,
            };
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (port.start)(call)))
                .map_err(|_| {
                    RuntimeError::new(
                        ErrorKind::HostFailure,
                        "异步端口启动 panic，外部效果可能已发生",
                    )
                })??;
            Ok(None)
        }
        _ => Err(RuntimeError::new(
            ErrorKind::InvalidModule,
            "异步片段没有返回可观察边界",
        )),
    }
    .map_err(|error| error.at(&task.execution.location))
}

fn process(
    instance: &mut Instance,
    shared: &Shared,
    pending: &mut BTreeMap<u64, Pending>,
    capacity: usize,
    request: Request,
) {
    let Request { operation, receipt } = request;
    let prepared = shared.check().and_then(|()| {
        if receipt.cancellation.is_cancelled() {
            return Err(RuntimeError::new(ErrorKind::Cancelled, "请求已取消"));
        }
        match operation {
            Operation::Call(name, arguments) => Ok(Some((name, arguments, true))),
            Operation::Event(key, generation, arguments) => instance
                .resolve_event(&key, generation, &arguments)
                .map(|(name, args)| Some((name, args, false))),
            Operation::Replace(candidate, generation) => {
                instance.replace_cancelled(candidate, generation, &receipt.cancellation)?;
                // 替换回执交出前撤销全部旧代续体；不能逐轮留下仍接受完成的旧槽。
                let mut stopped = Vec::new();
                for (_, mut task) in std::mem::take(pending) {
                    task.stop();
                    let error = shared.check().err().unwrap_or_else(|| {
                        RuntimeError::new(ErrorKind::Conflict, "异步任务所属模块已替换")
                            .at(&task.execution.location)
                    });
                    stopped.push((task.receipt, error));
                }
                // 先发布整批终态，再发各回执，保持 wait 后快照可见性的合同。
                if let Some((last, error)) = stopped.last() {
                    shared.publish(instance, pending, Some(last.sequence), Some(error.clone()));
                }
                for (receipt, error) in stopped {
                    drop(
                        receipt
                            .reply
                            .send(Err(shared.check().err().unwrap_or(error))),
                    );
                }
                Ok(None)
            }
        }
    });
    let (name, arguments, exported) = match prepared {
        Ok(Some(values)) => values,
        Ok(None) => {
            finish(instance, shared, pending, receipt, Ok(Value::Unit));
            return;
        }
        Err(error) => {
            finish(instance, shared, pending, receipt, Err(error));
            return;
        }
    };
    if !instance.is_async(&name) {
        let result = if exported {
            instance.call_with_cancellation(&name, &arguments, &receipt.cancellation)
        } else {
            instance.invoke(&name, &arguments, &receipt.cancellation)
        };
        finish(instance, shared, pending, receipt, result);
        return;
    }
    if pending.len() >= capacity {
        finish(
            instance,
            shared,
            pending,
            receipt,
            Err(RuntimeError::new(ErrorKind::Quota, "模块在途异步任务已满")),
        );
        return;
    }
    let execution = match instance.start_task(&name, arguments, exported) {
        Ok(execution) => execution,
        Err(error) => {
            finish(instance, shared, pending, receipt, Err(error));
            return;
        }
    };
    let mut task = Pending {
        receipt,
        execution,
        waiting: None,
    };
    match drive(instance, shared, &mut task) {
        Ok(None) => {
            pending.insert(task.receipt.sequence, task);
            shared.publish(instance, pending, None, None);
        }
        result => {
            if result.is_err() {
                task.stop();
            }
            finish(
                instance,
                shared,
                pending,
                task.receipt,
                result.map(|value| value.unwrap_or(Value::Unit)),
            );
        }
    }
}

fn run(
    mut instance: Instance,
    receiver: mpsc::Receiver<Request>,
    shared: &Shared,
    capacity: usize,
) {
    let mut pending: BTreeMap<u64, Pending> = BTreeMap::new();
    loop {
        if shared.check().is_err() {
            break;
        }
        let mut progressed = false;
        match receiver.try_recv() {
            Ok(request) => {
                process(&mut instance, shared, &mut pending, capacity, request);
                progressed = true;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let ready = pending.iter().find_map(|(sequence, task)| {
            (task.terminal(&instance).is_some()
                || task
                    .waiting
                    .as_ref()
                    .is_some_and(|waiting| waiting.slot.is_ready()))
            .then_some(*sequence)
        });
        if let Some(sequence) = ready {
            let mut task = pending.remove(&sequence).expect("已定位的在途任务");
            match drive(&mut instance, shared, &mut task) {
                Ok(None) => {
                    pending.insert(sequence, task);
                    shared.publish(&instance, &pending, None, None);
                }
                result => {
                    if result.is_err() {
                        task.stop();
                    }
                    finish(
                        &instance,
                        shared,
                        &pending,
                        task.receipt,
                        result.map(|value| value.unwrap_or(Value::Unit)),
                    );
                }
            }
            progressed = true;
        }
        if !progressed {
            let timeout = pending
                .values()
                .map(|task| {
                    instance
                        .limits()
                        .timeout
                        .saturating_sub(task.execution.started.elapsed())
                })
                .min();
            if let Some(timeout) = timeout {
                std::thread::park_timeout(timeout);
            } else {
                std::thread::park();
            }
        }
    }
    instance.close();
    let error = shared
        .check()
        .err()
        .unwrap_or_else(|| RuntimeError::new(ErrorKind::Closed, "实例已关闭"));
    for (_, mut task) in pending {
        task.stop();
        drop(task.receipt.reply.send(Err(error.clone())));
    }
    for request in receiver.try_iter() {
        drop(request.receipt.reply.send(Err(error.clone())));
    }
}
