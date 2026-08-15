use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, Weak};

use crate::core::ComponentId;
use crate::ui::component::app_state::{AppState, AppStateInner, FocusRequest};

/// 应用持有、由声明式 View 绑定的编程式焦点句柄。
///
/// 句柄只记录目标节点与所属 [`AppState`] 的弱引用，不持有 `WidgetTree`。
/// 同一个句柄同时绑定多个存活节点时，命令会返回明确的歧义错误。
#[derive(Clone, Default)]
pub struct FocusHandle {
    inner: Arc<Mutex<FocusHandleInner>>,
}

#[derive(Default)]
struct FocusHandleInner {
    bindings: HashMap<ComponentId, Weak<Mutex<AppStateInner>>>,
}

/// [`FocusHandle`] 无法登记焦点命令的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusHandleError {
    /// 句柄尚未绑定，或原目标已经从 View 树移除。
    Unbound,
    /// 同一个句柄被同时用于多个存活节点，目标不唯一。
    AmbiguousTarget {
        /// 当前仍存活的绑定目标数量。
        count: usize,
    },
    /// 目标绑定仍存在，但节点当前不在所属窗口的可用注册表中。
    TargetUnavailable,
}

impl fmt::Display for FocusHandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound => formatter.write_str("focus handle is not bound to a live view"),
            Self::AmbiguousTarget { count } => {
                write!(formatter, "focus handle has {count} live targets")
            }
            Self::TargetUnavailable => {
                formatter.write_str("focus handle target is not currently available")
            }
        }
    }
}

impl std::error::Error for FocusHandleError {}

impl FocusHandle {
    /// 创建尚未绑定任何声明式节点的焦点句柄。
    pub fn new() -> Self {
        Self::default()
    }

    /// 将焦点请求登记到目标节点所属窗口的 UI 轮次。
    pub fn focus(&self) -> Result<(), FocusHandleError> {
        self.request(FocusRequest::Focus)
    }

    /// 将焦点请求登记到目标窗口，并请求祖先 viewport 显露目标节点。
    pub fn focus_and_reveal(&self) -> Result<(), FocusHandleError> {
        self.request(FocusRequest::FocusAndReveal)
    }

    /// 仅当目标节点当前持有焦点时，在所属窗口清除焦点。
    pub fn blur(&self) -> Result<(), FocusHandleError> {
        self.request(FocusRequest::Blur)
    }

    /// 句柄是否恰好绑定一个仍存活的窗口节点。
    pub fn is_bound(&self) -> bool {
        self.live_binding_count() == 1
    }

    pub(crate) fn same_handle(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn bind(&self, id: ComponentId, app_state: &AppState) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .bindings
            .insert(id, Arc::downgrade(&app_state.inner));
    }

    pub(crate) fn unbind(&self, id: ComponentId) {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .bindings
            .remove(&id);
    }

    fn live_binding_count(&self) -> usize {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        inner
            .bindings
            .retain(|_, app_state| app_state.strong_count() > 0);
        inner.bindings.len()
    }

    fn request(&self, request: FocusRequest) -> Result<(), FocusHandleError> {
        let binding = {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            inner
                .bindings
                .retain(|_, app_state| app_state.strong_count() > 0);
            match inner.bindings.len() {
                0 => return Err(FocusHandleError::Unbound),
                1 => inner
                    .bindings
                    .iter()
                    .next()
                    .map(|(&id, app_state)| (id, app_state.clone())),
                count => return Err(FocusHandleError::AmbiguousTarget { count }),
            }
        };

        let Some((id, app_state)) = binding else {
            return Err(FocusHandleError::Unbound);
        };
        let Some(app_state) = app_state.upgrade() else {
            return Err(FocusHandleError::Unbound);
        };
        let waker = app_state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .enqueue_focus_request(id, request)
            .ok_or(FocusHandleError::TargetUnavailable)?;
        waker.wake();
        Ok(())
    }
}
