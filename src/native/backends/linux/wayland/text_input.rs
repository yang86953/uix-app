// ============================================================================
// platform/linux/wayland/text_input.rs — ITextInput impl (zwp_text_input_v3)
//
// 通过 zwp_text_input_v3 协议实现 Linux 下的输入法（IME）支持。
// 当 IME 提交文本时，将 UiEvent::text_input 推入事件队列。
// ============================================================================

use std::sync::atomic::Ordering;
use wayland_protocols::unstable::text_input::v3::client::zwp_text_input_v3;

use crate::core::{Errc, Error, Rect, Result, WindowId};
use crate::native::shared::ime_events::{on_unmark_text_for_window, PendingImeBatch};
use crate::native::traits::input::ITextInput;

use super::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// ITextInput for WaylandBackend
// ════════════════════════════════════════════════════════════════════════════

impl ITextInput for WaylandBackend {
    fn set_target_window(
        &mut self,
        window_id: WindowId,
        _native_window: *mut std::ffi::c_void,
    ) -> Result<()> {
        self.text_input_window_id = Some(window_id);
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        let window_id = self.text_input_window_id.ok_or_else(|| {
            Error::new(
                Errc::InvalidOperation,
                "Wayland text_input: no target window selected",
            )
        })?;
        if self.text_input.is_some() || self.active_text_input_window_id.is_some() {
            self.stop()?;
        }
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
        let surface_windows = self.surface_windows.clone();
        let composition = self.text_input_composition.clone();
        let active_generation = self.text_input_generation.clone();
        let generation = active_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        let mut focused = false;
        let mut pending = PendingImeBatch::default();

        ti.quick_assign(move |_, event, _| {
            if active_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            match event {
                zwp_text_input_v3::Event::Enter { surface } => {
                    let owns_surface = surface_windows
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .window_for_surface(surface.as_ref().id())
                        == Some(window_id);
                    if focused && !owns_surface {
                        let mut state = composition
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        on_unmark_text_for_window(&events, &mut state, window_id);
                    }
                    focused = owns_surface;
                    pending = PendingImeBatch::default();
                }
                zwp_text_input_v3::Event::Leave { .. } => {
                    if focused {
                        let mut state = composition
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        on_unmark_text_for_window(&events, &mut state, window_id);
                    }
                    focused = false;
                    pending = PendingImeBatch::default();
                }
                zwp_text_input_v3::Event::PreeditString { text, .. } if focused => {
                    pending.set_preedit(text);
                }
                zwp_text_input_v3::Event::CommitString { text } if focused => {
                    pending.set_commit(text);
                }
                zwp_text_input_v3::Event::Done { .. } if focused => {
                    let mut state = composition
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    pending.apply_for_window(&events, &mut state, window_id);
                }
                _ => {}
            }
        });

        ti.enable();
        ti.commit();
        self.text_input = Some(ti);
        self.active_text_input_window_id = Some(window_id);
        self.text_input_enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.text_input_generation.fetch_add(1, Ordering::SeqCst);
        if let Some(ref ti) = self.text_input {
            ti.disable();
            ti.commit();
        }
        if let Some(window_id) = self.active_text_input_window_id.take() {
            let mut state = self
                .text_input_composition
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            on_unmark_text_for_window(&self.events, &mut state, window_id);
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
