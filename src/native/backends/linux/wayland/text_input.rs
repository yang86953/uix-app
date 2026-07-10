// ============================================================================
// platform/linux/wayland/text_input.rs — ITextInput impl (zwp_text_input_v3)
//
// 通过 zwp_text_input_v3 协议实现 Linux 下的输入法（IME）支持。
// 当 IME 提交文本时，将 UiEvent::text_input 推入事件队列。
// ============================================================================

use std::sync::atomic::Ordering;
use wayland_protocols::unstable::text_input::v3::client::zwp_text_input_v3;

use crate::core::{Errc, Error, Rect, Result};
use crate::native::traits::event::UiEvent;
use crate::native::traits::input::ITextInput;

use super::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// ITextInput for WaylandBackend
// ════════════════════════════════════════════════════════════════════════════

impl ITextInput for WaylandBackend {
    fn start(&mut self) -> Result<()> {
        let seat = match self.seat.as_ref() {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "Wayland text_input: no seat available",
                ));
            }
        };

        let manager = match self.text_input_manager.as_ref() {
            Some(m) => m,
            None => {
                return Err(Error::new(
                    Errc::NotImplemented,
                    "Wayland text_input: compositor has no zwp_text_input_manager_v3",
                ));
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
                            q.push_back(UiEvent::text_input(t.clone()));
                        }
                    }
                }
            }
        });

        ti.enable();
        ti.commit();
        self.text_input = Some(ti);
        self.text_input_enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(ref ti) = self.text_input {
            ti.disable();
            ti.commit();
        }
        self.text_input = None;
        self.text_input_enabled.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()> {
        let Some(text_input) = self.text_input.as_ref() else {
            return Err(Error::new(
                Errc::InvalidOperation,
                "Wayland text_input: no active IME session",
            ));
        };
        text_input.set_cursor_rectangle(
            rect.x.round() as i32,
            rect.y.round() as i32,
            rect.w.max(0.0).round() as i32,
            rect.h.max(0.0).round() as i32,
        );
        text_input.commit();
        Ok(())
    }
}
