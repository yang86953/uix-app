// ============================================================================
// platform/linux/timer.rs — Linux 定时器（ITimer 实现）
// ============================================================================
//
// 使用单一后台线程通过优先级队列管理所有定时器，
// 取代线程-per-timer 模式以降低线程开销。
// ============================================================================

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::diagnostics::PendingFailureSource;
use crate::native::{Errc, Error, Result};
use crate::platform::system::ITimer;
use crate::platform::windowing::event::UiEvent;

/// 定时器控制指令
enum Cmd {
    Register {
        id: u32,
        interval_ms: u32,
        repeating: bool,
    },
    Clear {
        id: u32,
    },
    Shutdown,
}

pub(crate) struct LinuxTimer {
    next_id: u32,
    cmd_tx: Sender<Cmd>,
    active: Arc<Mutex<HashMap<u32, u32>>>,
    pending_failures: PendingFailureSource,
}

impl LinuxTimer {
    pub(crate) fn new(
        event_queue: Arc<Mutex<std::collections::VecDeque<UiEvent>>>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let active = Arc::new(Mutex::new(HashMap::new()));
        let active_clone = active.clone();
        let worker_failures = pending_failures.clone();

        // 单一后台线程管理所有定时器
        if let Err(error) = std::thread::Builder::new()
            .name("uix-timers".into())
            .spawn(move || {
                let mut entries: Vec<(Instant, u32, u32, bool)> = Vec::new();
                loop {
                    if entries.is_empty() {
                        // 无活跃定时器，无限阻塞等待新指令
                        match cmd_rx.recv() {
                            Ok(cmd) => match cmd {
                                Cmd::Shutdown => break,
                                Cmd::Register {
                                    id,
                                    interval_ms,
                                    repeating,
                                } => {
                                    entries.push((
                                        Instant::now()
                                            + std::time::Duration::from_millis(interval_ms as u64),
                                        id,
                                        interval_ms,
                                        repeating,
                                    ));
                                }
                                Cmd::Clear { id } => {
                                    entries.retain(|e| e.1 != id);
                                    match active_clone.lock() {
                                        Ok(mut map) => {
                                            map.remove(&id);
                                        }
                                        Err(poisoned) => {
                                            let mut map = poisoned.into_inner();
                                            map.remove(&id);
                                            let _ = worker_failures.enqueue(Error::new(
                                                Errc::InvalidState,
                                                "LinuxTimer: active timer state lock was poisoned",
                                            ));
                                        }
                                    }
                                }
                            },
                            Err(_) => {
                                let _ = worker_failures.enqueue(Error::new(
                                    Errc::IoError,
                                    "LinuxTimer: timer worker command channel disconnected",
                                ));
                                break;
                            }
                        }
                        continue;
                    }

                    // 有定时器时计算等待时间
                    let now = Instant::now();
                    let next = entries.iter().map(|e| e.0).min().unwrap_or(now);
                    let wait = if next > now {
                        next - now
                    } else {
                        std::time::Duration::ZERO
                    };
                    match cmd_rx.recv_timeout(wait) {
                        Ok(cmd) => {
                            match cmd {
                                Cmd::Shutdown => break,
                                Cmd::Register {
                                    id,
                                    interval_ms,
                                    repeating,
                                } => {
                                    entries.push((
                                        Instant::now()
                                            + std::time::Duration::from_millis(interval_ms as u64),
                                        id,
                                        interval_ms,
                                        repeating,
                                    ));
                                }
                                Cmd::Clear { id } => {
                                    entries.retain(|e| e.1 != id);
                                    match active_clone.lock() {
                                        Ok(mut map) => {
                                            map.remove(&id);
                                        }
                                        Err(poisoned) => {
                                            let mut map = poisoned.into_inner();
                                            map.remove(&id);
                                            let _ = worker_failures.enqueue(Error::new(
                                                Errc::InvalidState,
                                                "LinuxTimer: active timer state lock was poisoned",
                                            ));
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => {
                            let _ = worker_failures.enqueue(Error::new(
                                Errc::IoError,
                                "LinuxTimer: timer worker command channel disconnected",
                            ));
                            break;
                        }
                    }

                    // 触发所有到期的定时器
                    let now = Instant::now();
                    let mut fired = Vec::new();
                    for entry in &entries {
                        if entry.0 <= now {
                            fired.push((entry.1, entry.2, entry.3));
                        }
                    }
                    entries.retain(|e| e.0 > now);

                    for (id, interval_ms, repeating) in fired {
                        match event_queue.lock() {
                            Ok(mut q) => {
                                q.push_back(UiEvent::timer(id));
                            }
                            Err(poisoned) => {
                                let mut q = poisoned.into_inner();
                                q.push_back(UiEvent::timer(id));
                                let _ = worker_failures.enqueue(Error::new(
                                    Errc::InvalidState,
                                    "LinuxTimer: event queue lock was poisoned",
                                ));
                            }
                        }
                        if repeating {
                            entries.push((
                                Instant::now()
                                    + std::time::Duration::from_millis(interval_ms as u64),
                                id,
                                interval_ms,
                                true,
                            ));
                        } else {
                            match active_clone.lock() {
                                Ok(mut map) => {
                                    map.remove(&id);
                                }
                                Err(poisoned) => {
                                    let mut map = poisoned.into_inner();
                                    map.remove(&id);
                                    let _ = worker_failures.enqueue(Error::new(
                                        Errc::InvalidState,
                                        "LinuxTimer: active timer state lock was poisoned",
                                    ));
                                }
                            }
                        }
                    }
                }
            })
        {
            let _ = pending_failures.enqueue(Error::new(
                Errc::IoError,
                format!("LinuxTimer: failed to start timer worker: {error}"),
            ));
        }

        Self {
            next_id: 1,
            cmd_tx,
            active,
            pending_failures,
        }
    }
}

impl ITimer for LinuxTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        match self.active.lock() {
            Ok(mut map) => {
                map.insert(id, interval_ms);
            }
            Err(poisoned) => {
                let mut map = poisoned.into_inner();
                map.insert(id, interval_ms);
                let _ = self.pending_failures.enqueue(Error::new(
                    Errc::InvalidState,
                    "LinuxTimer::set: active timer state lock was poisoned",
                ));
            }
        }
        self.cmd_tx
            .send(Cmd::Register {
                id,
                interval_ms,
                repeating,
            })
            .map_err(|_| {
                Error::new(
                    Errc::IoError,
                    "LinuxTimer::set: timer worker channel closed",
                )
            })?;
        Ok(id)
    }

    fn clear(&mut self, id: u32) -> Result<()> {
        self.cmd_tx.send(Cmd::Clear { id }).map_err(|_| {
            Error::new(
                Errc::IoError,
                "LinuxTimer::clear: timer worker channel closed",
            )
        })
    }
}

impl Drop for LinuxTimer {
    fn drop(&mut self) {
        if self.cmd_tx.send(Cmd::Shutdown).is_err() {
            let _ = self.pending_failures.enqueue(Error::new(
                Errc::IoError,
                "LinuxTimer::drop: timer worker channel closed",
            ));
        }
    }
}
