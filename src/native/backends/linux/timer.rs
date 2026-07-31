// ============================================================================
// platform/linux/timer.rs — Linux 定时器（ITimer 实现）
// ============================================================================
//
// 使用单一后台线程通过优先级队列管理所有定时器，
// 取代线程-per-timer 模式以降低线程开销。
// ============================================================================

use crate::native::capabilities::system::ITimer;
use crate::native::windowing::event::UiEvent;
use crate::native::{Errc, Error, Result};
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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

pub struct LinuxTimer {
    next_id: u32,
    cmd_tx: Sender<Cmd>,
    active: Arc<Mutex<HashMap<u32, u32>>>,
}

impl LinuxTimer {
    pub fn new(event_queue: Arc<Mutex<std::collections::VecDeque<UiEvent>>>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let active = Arc::new(Mutex::new(HashMap::new()));
        let active_clone = active.clone();

        // 单一后台线程管理所有定时器
        std::thread::Builder::new()
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
                                    if let Ok(mut map) = active_clone.lock() {
                                        map.remove(&id);
                                    }
                                }
                            },
                            Err(_) => break,
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
                    if let Ok(cmd) = cmd_rx.recv_timeout(wait) {
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
                                if let Ok(mut map) = active_clone.lock() {
                                    map.remove(&id);
                                }
                            }
                        }
                        continue;
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
                        if let Ok(mut q) = event_queue.lock() {
                            q.push_back(UiEvent::timer(id));
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
                            if let Ok(mut map) = active_clone.lock() {
                                map.remove(&id);
                            }
                        }
                    }
                }
            })
            .ok();

        Self {
            next_id: 1,
            cmd_tx,
            active,
        }
    }
}

impl ITimer for LinuxTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if let Ok(mut map) = self.active.lock() {
            map.insert(id, interval_ms);
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
        let _ = self.cmd_tx.send(Cmd::Shutdown);
    }
}
