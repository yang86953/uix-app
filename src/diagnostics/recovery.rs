use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, Weak};

use crate::core::{Errc, Error};

thread_local! {
    static RUNNING_RECOVERY: Cell<bool> = const { Cell::new(false) };
}

/// Decision returned by a registered recovery handler.
pub enum RecoveryAction {
    /// This handler cannot recover the error; try the next matching handler.
    NotHandled,
    /// The handler restored the invariant owned by its domain.
    Recovered,
    /// Recovery itself failed with a typed error.
    Failed(Error),
}

/// Result of running all matching recovery handlers.
pub enum RecoveryOutcome {
    Recovered,
    Unhandled(Error),
    Failed(Error),
}

impl RecoveryOutcome {
    pub fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered)
    }

    /// Returns the unhandled or recovery error when operation could not be
    /// restored.
    pub fn into_error(self) -> Option<Error> {
        match self {
            Self::Recovered => None,
            Self::Unhandled(error) | Self::Failed(error) => Some(error),
        }
    }
}

type RecoveryHandler = dyn Fn(&Error) -> RecoveryAction + Send + Sync + 'static;

struct RecoveryEntry {
    id: u64,
    code: Errc,
    handler: Arc<RecoveryHandler>,
}

struct RecoveryState {
    next_id: u64,
    entries: Vec<RecoveryEntry>,
}

pub(crate) struct RecoveryRegistry {
    state: Mutex<RecoveryState>,
}

impl RecoveryRegistry {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(RecoveryState {
                next_id: 1,
                entries: Vec::new(),
            }),
        }
    }

    pub(crate) fn register<F>(self: &Arc<Self>, code: Errc, handler: F) -> RecoverySubscription
    where
        F: Fn(&Error) -> RecoveryAction + Send + Sync + 'static,
    {
        let mut state = self.lock_state();
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        state.entries.push(RecoveryEntry {
            id,
            code,
            handler: Arc::new(handler),
        });
        RecoverySubscription {
            id,
            registry: Arc::downgrade(self),
        }
    }

    pub(crate) fn attempt(&self, error: Error) -> RecoveryOutcome {
        let Some(_guard) = RecoveryGuard::enter() else {
            return RecoveryOutcome::Unhandled(error);
        };

        let handlers = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .filter(|entry| entry.code == error.code())
                .map(|entry| (entry.id, Arc::clone(&entry.handler)))
                .collect::<Vec<_>>()
        };

        for (handler_id, handler) in handlers {
            match catch_unwind(AssertUnwindSafe(|| handler(&error))) {
                Ok(RecoveryAction::NotHandled) => {}
                Ok(RecoveryAction::Recovered) => {
                    tracing::info!(
                        target: "uix::diagnostics",
                        error_code = %error.code(),
                        recovery_handler_id = handler_id,
                        "registered recovery restored the error invariant"
                    );
                    return RecoveryOutcome::Recovered;
                }
                Ok(RecoveryAction::Failed(recovery_error)) => {
                    let recovery_error = recovery_error.with_appended_source(error);
                    return RecoveryOutcome::Failed(
                        Error::new(recovery_error.code(), "registered error recovery failed")
                            .with_source(recovery_error),
                    );
                }
                Err(_) => {
                    return RecoveryOutcome::Failed(
                        Error::new(Errc::TaskAbandoned, "registered error recovery panicked")
                            .with_source(error),
                    );
                }
            }
        }

        RecoveryOutcome::Unhandled(error)
    }

    fn remove(&self, id: u64) {
        self.lock_state().entries.retain(|entry| entry.id != id);
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RecoveryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// RAII registration for one exact `Errc` recovery handler.
///
/// Dropping it prevents future attempts from selecting the handler. An attempt
/// that already cloned the handler may finish once without holding the
/// registry lock.
pub struct RecoverySubscription {
    id: u64,
    registry: Weak<RecoveryRegistry>,
}

impl Drop for RecoverySubscription {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.remove(self.id);
        }
    }
}

struct RecoveryGuard;

impl RecoveryGuard {
    fn enter() -> Option<Self> {
        RUNNING_RECOVERY.with(|running| {
            if running.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for RecoveryGuard {
    fn drop(&mut self) {
        RUNNING_RECOVERY.with(|running| running.set(false));
    }
}
