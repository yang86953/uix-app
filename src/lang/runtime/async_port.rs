//! 显式宿主异步端口与一次性结果槽；槽位不持有模块、状态或 continuation。

use crate::lang::runtime::value::{quota_error, type_error};
use crate::lang::runtime::*;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicUsize, Ordering},
};

/// 进程内唯一操作身份；不作为重启恢复或外部幂等的持久标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationId {
    pub instance: u64,
    pub generation: u64,
    pub request: u64,
    pub await_ordinal: u64,
}

/// 宿主接管的拥有型请求。启动回调返回不等于外部业务完成。
pub struct AsyncCall {
    pub operation: OperationId,
    pub arguments: Vec<Value>,
    pub cancellation: Cancellation,
    pub completion: AsyncCompletion,
}

/// 启动回调必须短时返回；实际 I/O、外部资源取消与 join 由宿主负责。
#[derive(Clone)]
pub struct AsyncHostPort {
    pub signature: Signature,
    pub start: Arc<dyn Fn(AsyncCall) -> RuntimeResult<()> + Send + Sync>,
}
pub type AsyncHostPorts = BTreeMap<String, AsyncHostPort>;

enum State {
    Pending,
    Ready(RuntimeResult<Value>),
    Consumed,
    Closed,
}

pub(crate) struct CompletionSlot {
    state: Mutex<State>,
    producers: AtomicUsize,
    cancellation: Cancellation,
    returns: Type,
    limits: Limits,
}
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

impl CompletionSlot {
    pub(crate) fn new(
        cancellation: Cancellation,
        returns: Type,
        limits: Limits,
    ) -> (Arc<Self>, AsyncCompletion) {
        let slot = Arc::new(Self {
            state: Mutex::new(State::Pending),
            producers: AtomicUsize::new(1),
            cancellation,
            returns,
            limits,
        });
        (slot.clone(), AsyncCompletion { slot })
    }
    pub(crate) fn is_ready(&self) -> bool {
        matches!(*lock(&self.state), State::Ready(_))
    }
    pub(crate) fn take(&self) -> Option<RuntimeResult<Value>> {
        let mut state = lock(&self.state);
        if !matches!(*state, State::Ready(_)) {
            return None;
        }
        match std::mem::replace(&mut *state, State::Consumed) {
            State::Ready(result) => Some(result),
            _ => None,
        }
    }
    pub(crate) fn close(&self) {
        *lock(&self.state) = State::Closed;
    }
    fn complete(&self, result: RuntimeResult<Value>) -> RuntimeResult<()> {
        let mut state = lock(&self.state);
        if self.cancellation.is_cancelled() || matches!(*state, State::Closed) {
            return Err(RuntimeError::new(
                ErrorKind::Cancelled,
                "异步操作已终止，不接受迟到完成",
            ));
        }
        if !matches!(*state, State::Pending) {
            return Err(RuntimeError::new(ErrorKind::Conflict, "异步操作已经完成"));
        }
        let result = result
            .and_then(|value| {
                value.validate_budget(self.limits.value_bytes, self.limits.value_items)?;
                if !self.returns.accepts(&value) {
                    return Err(type_error("异步端口返回类型不符"));
                }
                Ok(value)
            })
            .map_err(|error| {
                if error
                    .message
                    .len()
                    .saturating_add(error.location.source.len())
                    > self.limits.value_bytes
                {
                    quota_error()
                } else {
                    error
                }
            });
        *state = State::Ready(result);
        drop(state);
        self.cancellation.wake();
        Ok(())
    }
}

/// 可以转交或克隆，但只接受一次完成。Ok 仅确认结果交回，不确认恢复提交。
pub struct AsyncCompletion {
    slot: Arc<CompletionSlot>,
}
impl AsyncCompletion {
    pub fn complete(&self, result: RuntimeResult<Value>) -> RuntimeResult<()> {
        self.slot.complete(result)
    }
}
impl Clone for AsyncCompletion {
    fn clone(&self) -> Self {
        self.slot.producers.fetch_add(1, Ordering::Relaxed);
        Self {
            slot: self.slot.clone(),
        }
    }
}
impl Drop for AsyncCompletion {
    fn drop(&mut self) {
        if self.slot.producers.fetch_sub(1, Ordering::AcqRel) == 1 {
            // 最后一位生产者放弃回执也是终态，不能把责任留给无限等待。
            drop(self.slot.complete(Err(RuntimeError::new(
                ErrorKind::HostFailure,
                "异步端口放弃了完成回执",
            ))));
        }
    }
}
