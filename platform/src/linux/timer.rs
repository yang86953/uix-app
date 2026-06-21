// ============================================================================
// platform/linux/timer.rs — Linux timer implementation (ITimer)
// ============================================================================
//
// Pushes UiEvent::timer events directly into the Wayland backend's shared
// event queue, so timer events are processed by the main event loop.
// ============================================================================

use crate::event::UiEvent;
use crate::ITimer;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

// ════════════════════════════════════════════════════════════════════════════
// LinuxTimer
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxTimer {
    next_id: u32,
    active: Arc<Mutex<HashMap<u32, TimerState>>>,
    event_queue: Arc<Mutex<std::collections::VecDeque<UiEvent>>>,
}

struct TimerState {
    _stopper: Sender<()>,
}

impl LinuxTimer {
    pub fn new(event_queue: Arc<Mutex<std::collections::VecDeque<UiEvent>>>) -> Self {
        Self {
            next_id: 1,
            active: Arc::new(Mutex::new(HashMap::new())),
            event_queue,
        }
    }
}

impl ITimer for LinuxTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let eq = self.event_queue.clone();
        let active = self.active.clone();
        let (stopper, stopped) = mpsc::channel::<()>();

        thread::Builder::new()
            .name(format!("uix-timer-{}", id))
            .spawn(move || {
                loop {
                    let start = std::time::Instant::now();
                    if stopped.recv_timeout(std::time::Duration::from_millis(interval_ms as u64))
                        .is_ok()
                    {
                        break;
                    }
                    // Push timer event directly into the shared event queue
                    if let Ok(mut q) = eq.lock() {
                        q.push_back(UiEvent::timer(id));
                    }
                    if !repeating {
                        break;
                    }
                    let elapsed = start.elapsed();
                    if elapsed.as_millis() as u32 >= interval_ms {
                        continue;
                    }
                }
                if let Ok(mut map) = active.lock() {
                    map.remove(&id);
                }
            })
            .ok();

        if let Ok(mut map) = self.active.lock() {
            map.insert(id, TimerState { _stopper: stopper });
        }

        id
    }

    fn clear(&mut self, id: u32) {
        if let Ok(mut map) = self.active.lock() {
            map.remove(&id);
        }
    }
}

impl Drop for LinuxTimer {
    fn drop(&mut self) {
        if let Ok(mut map) = self.active.lock() {
            map.clear();
        }
    }
}
