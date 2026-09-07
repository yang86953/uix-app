//! 有界后台命令队列。运行实例只由 worker 持有，UI 出口只读取拥有型快照。

use crate::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, mpsc, atomic::{AtomicU8, Ordering}};
use std::time::Duration;

/// 完整提交快照。sequence 覆盖已处理的请求，失败保留原界面并携带错误。
#[derive(Debug, Clone)]
pub struct ModuleSnapshot {
    pub name: String,
    pub generation: u64,
    pub revision: u64,
    pub sequence: u64,
    pub state: BTreeMap<String, Value>,
    pub view: Option<ViewSnapshot>,
    pub error: Option<RuntimeError>,
    pub active: bool,
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    // 序号与拥有型快照没有用户回调；中毒时仍允许关闭与读取最后提交。
    value.lock().unwrap_or_else(|error| error.into_inner())
}

struct Shared {
    snapshot: Mutex<ModuleSnapshot>,
    next: Mutex<u64>,
    lifecycle: AtomicU8,
    cancellation: Cancellation,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Shared {
    fn notify(&self) {
        // 唤醒失败不会改变已提交业务结果；调用方仍可主动读取快照。
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.wake)())).is_err() {
            lock(&self.snapshot).error = Some(RuntimeError::new(ErrorKind::HostFailure, "宿主唤醒回调 panic"));
        }
    }
    fn check(&self) -> RuntimeResult<()> {
        match self.lifecycle.load(Ordering::Acquire) {
            0 => Ok(()), 1 => Err(RuntimeError::new(ErrorKind::CapabilityDenied, "实例已撤权")),
            _ => Err(RuntimeError::new(ErrorKind::Closed, "实例已关闭")),
        }
    }
}

enum Operation {
    Call(String, Vec<Value>), Event(String, u64, Vec<Value>), Replace(Module, u64),
}
struct Request {
    sequence: u64,
    operation: Operation,
    cancellation: Cancellation,
    reply: mpsc::SyncSender<RuntimeResult<Value>>,
}

/// 非阻塞投递的回执。等待仅供业务线程使用；UI 通过 snapshot 观察完成。
pub struct ModuleRequest {
    pub sequence: u64,
    pub cancellation: Cancellation,
    reply: mpsc::Receiver<RuntimeResult<Value>>,
}
impl ModuleRequest {
    pub fn wait(self, timeout: Duration) -> RuntimeResult<Value> {
        self.reply.recv_timeout(timeout).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => RuntimeError::new(ErrorKind::Timeout, "等待超时；请求仍可能执行，可显式取消"),
            mpsc::RecvTimeoutError::Disconnected => RuntimeError::new(ErrorKind::Closed, "执行队列已关闭"),
        })?
    }
}

/// 可克隆的操作出口，不拥有实例的关闭责任。
#[derive(Clone)]
pub struct ModuleHandle { sender: mpsc::SyncSender<Option<Request>>, shared: Arc<Shared> }
impl ModuleHandle {
    pub fn snapshot(&self) -> ModuleSnapshot { lock(&self.shared.snapshot).clone() }
    pub fn call(&self, name: &str, arguments: Vec<Value>) -> RuntimeResult<ModuleRequest> {
        self.submit(Operation::Call(name.into(), arguments))
    }
    pub fn event(&self, key: &str, generation: u64, arguments: Vec<Value>) -> RuntimeResult<ModuleRequest> {
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
        { let mut snapshot = lock(&self.shared.snapshot);
          snapshot.active = false; snapshot.view = None; snapshot.state.clear();
          snapshot.error = self.shared.check().err(); }
        self.shared.notify();
        // 满队列已经能唤醒接收者；关闭通道也无需另一次停止信号。
        drop(self.sender.try_send(None));
    }
    fn submit(&self, operation: Operation) -> RuntimeResult<ModuleRequest> {
        let result: RuntimeResult<ModuleRequest> = (|| {
            self.shared.check()?;
            let mut next = lock(&self.shared.next);
            let sequence = next.checked_add(1).ok_or_else(|| RuntimeError::new(ErrorKind::Quota, "请求序号耗尽"))?;
            let (reply, receiver) = mpsc::sync_channel(1);
            let cancellation = self.shared.cancellation.child();
            self.sender.try_send(Some(Request { sequence, operation, cancellation: cancellation.clone(), reply }))
                .map_err(|error| match error { mpsc::TrySendError::Full(_) => RuntimeError::new(ErrorKind::Quota, "模块命令队列已满"),
                    mpsc::TrySendError::Disconnected(_) => RuntimeError::new(ErrorKind::Closed, "模块命令队列已关闭") })?;
            *next = sequence;
            Ok(ModuleRequest { sequence, cancellation, reply: receiver })
        })();
        if let Err(error) = &result { lock(&self.shared.snapshot).error = Some(error.clone()); self.shared.notify(); }
        result
    }
}

/// 后台执行租约。Drop 请求取消；close 可在非 UI 线程等待真实退出。
pub struct ModuleWorker {
    handle: ModuleHandle,
    done: mpsc::Receiver<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl ModuleWorker {
    pub fn spawn(instance: Instance, capacity: usize, wake: impl Fn() + Send + Sync + 'static) -> RuntimeResult<Self> {
        if !(1..=1024).contains(&capacity) { return Err(RuntimeError::new(ErrorKind::Argument, "队列容量必须在 1 到 1024 之间")); }
        let snapshot = ModuleSnapshot { name: instance.module().name.clone(), generation: instance.generation(),
            revision: instance.revision(), sequence: 0, state: instance.state(), view: instance.view()?, error: None, active: true };
        let shared = Arc::new(Shared { snapshot: Mutex::new(snapshot), next: Mutex::new(0), lifecycle: AtomicU8::new(0),
            cancellation: Cancellation::default(), wake: Arc::new(wake) });
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let (done_sender, done) = mpsc::sync_channel(1);
        let owner = shared.clone();
        let thread = std::thread::Builder::new().name("uix-module".into()).spawn(move || {
            run(instance, receiver, &owner);
            // 接收者可能已放弃等待；退出已完成，无需另建错误通道。
            drop(done_sender.send(()));
        }).map_err(|e| RuntimeError::new(ErrorKind::HostFailure, format!("模块线程启动失败：{e}")))?;
        Ok(Self { handle: ModuleHandle { sender, shared }, done, thread: Some(thread) })
    }
    pub fn handle(&self) -> ModuleHandle { self.handle.clone() }
    pub fn close(mut self, timeout: Duration) -> RuntimeResult<()> {
        self.handle.close();
        self.done.recv_timeout(timeout).map_err(|_| RuntimeError::new(ErrorKind::Timeout, "模块线程尚未退出；已禁止发布结果"))?;
        if let Some(thread) = self.thread.take() { thread.join().map_err(|_| RuntimeError::new(ErrorKind::HostFailure, "模块线程 panic"))?; }
        Ok(())
    }
}
impl Drop for ModuleWorker { fn drop(&mut self) { self.handle.close(); } }

fn run(mut instance: Instance, receiver: mpsc::Receiver<Option<Request>>, shared: &Shared) {
    loop {
        if shared.check().is_err() { break; }
        let request = match receiver.recv() {
            Ok(Some(request)) => request, Ok(None) | Err(_) => break,
        };
        let mut result = shared.check().and_then(|()| {
            if request.cancellation.is_cancelled() { return Err(RuntimeError::new(ErrorKind::Cancelled, "请求已取消")); }
            match request.operation {
                Operation::Call(name, args) => instance.call_with_cancellation(&name, &args, &request.cancellation),
                Operation::Event(key, generation, args) => instance.event(&key, generation, &args, &request.cancellation),
                Operation::Replace(candidate, generation) => instance.replace(candidate, generation).map(|()| Value::Unit),
            }
        });
        { let mut snapshot = lock(&shared.snapshot);
          if let Err(error) = shared.check() { result = Err(error); }
          else {
              snapshot.sequence = request.sequence;
              snapshot.generation = instance.generation();
              snapshot.revision = instance.revision();
              snapshot.state = instance.state();
              match instance.view() { Ok(view) => snapshot.view = view, Err(error) => result = Err(error) }
              snapshot.error = result.as_ref().err().cloned();
          }
        }
        // 回执允许主动丢弃；结果与失败均已进入公开快照。
        drop(request.reply.send(result));
        shared.notify();
    }
    instance.close();
    for request in receiver.try_iter().flatten() {
        drop(request.reply.send(Err(shared.check().err().unwrap_or_else(|| RuntimeError::new(ErrorKind::Closed, "实例已关闭")))));
    }
}
