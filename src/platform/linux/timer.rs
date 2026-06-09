// ============================================================================
// platform/linux/timer.rs — Linux timer implementation (ITimer)
// ============================================================================
//
// Uses std::sync::mpsc + std::thread::spawn for timer dispatching.
// Timers fire UiEvent::timer events through the platform's event queue.
// The platform must route these back to the main event queue.
//
// NOTE: This implementation stores a sender handle. The timers are
// dispatched via a channel. The main event loop must check this channel
// and push the events into its own UiEvent queue.
// ============================================================================

use crate::platform::ITimer;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

// ════════════════════════════════════════════════════════════════════════════
// Timer message
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub struct TimerFired {
    pub id: u32,
}

// ════════════════════════════════════════════════════════════════════════════
// LinuxTimer
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxTimer {
    next_id: u32,
    active: Arc<Mutex<HashMap<u32, TimerState>>>,
    tx: Sender<TimerFired>,
}

struct TimerState {
    _stopper: Sender<()>,
}

impl LinuxTimer {
    pub fn new(tx: Sender<TimerFired>) -> Self {
        Self {
            next_id: 1,
            active: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }
}

impl ITimer for LinuxTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let tx = self.tx.clone();
        let active = self.active.clone();
        let (stopper, stopped) = mpsc::channel::<()>();

        thread::Builder::new()
            .name(format!("uix-timer-{}", id))
            .spawn(move || {
                loop {
                    // Wait for either the interval or a stop signal
                    let start = std::time::Instant::now();
                    if stopped.recv_timeout(std::time::Duration::from_millis(interval_ms as u64))
                        .is_ok()
                    {
                        // Received stop signal
                        break;
                    }
                    // Timer fired
                    if tx.send(TimerFired { id }).is_err() {
                        // Receiver dropped
                        break;
                    }
                    if !repeating {
                        break;
                    }
                    // Adjust for time spent in send
                    let elapsed = start.elapsed();
                    if elapsed.as_millis() as u32 >= interval_ms {
                        continue;
                    }
                }
                // Clean up on exit
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
