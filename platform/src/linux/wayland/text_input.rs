// ============================================================================
// platform/linux/wayland/text_input.rs — ITextInput impl (zwp_text_input_v3)
//
// 通过 zwp_text_input_v3 协议实现 Linux 下的输入法（IME）支持。
// 当 IME 提交文本时，将 UiEvent::key_press 推入事件队列。
// ============================================================================

use std::sync::atomic::Ordering;
use wayland_protocols::unstable::text_input::v3::client::zwp_text_input_v3;

use crate::event::UiEvent;
use crate::ITextInput;

use super::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// ITextInput for WaylandBackend
// ════════════════════════════════════════════════════════════════════════════

impl ITextInput for WaylandBackend {
    fn start(&mut self) {
        let seat = match self.seat.as_ref() {
            Some(s) => s,
            None => {
                crate::log::warn_fn("Wayland text_input: no seat available, IME not activated");
                return;
            }
        };

        let manager = match self.text_input_manager.as_ref() {
            Some(m) => m,
            None => {
                crate::log::warn_fn("Wayland text_input: no zwp_text_input_manager_v3, compositor may not support IME");
                return;
            }
        };

        // 如果已有活跃的 text_input，先销毁
        self.text_input = None;

        let ti = manager.get_text_input(seat);
        let events = self.events.clone();

        ti.quick_assign(move |_, event, _| {
            if let zwp_text_input_v3::Event::CommitString { text } = event {
                if let Some(ref t) = text {
                    if !t.is_empty() {
                        if let Ok(mut q) = events.lock() {
                            q.push_back(UiEvent::key_press(t.clone()));
                        }
                    }
                }
            }
        });

        ti.enable();
        ti.commit();
        self.text_input = Some(ti);
        self.text_input_enabled.store(true, Ordering::SeqCst);
    }

    fn stop(&mut self) {
        if let Some(ref ti) = self.text_input {
            ti.disable();
            ti.commit();
        }
        self.text_input = None;
        self.text_input_enabled.store(false, Ordering::SeqCst);
    }
}
