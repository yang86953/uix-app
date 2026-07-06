// ============================================================================
// platform/linux/wayland/clipboard.rs — IClipboard impl for WaylandBackend
// ============================================================================

use std::fs::File;
use std::os::unix::io::FromRawFd;

use wayland_client::protocol::wl_data_source;

use crate::native::traits::input::IClipboard;

use super::WaylandBackend;

impl IClipboard for WaylandBackend {
    fn text(&self) -> String {
        self.clipboard_text
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default()
    }
    fn set_text(&mut self, text: &str) {
        if let Ok(mut t) = self.clipboard_text.lock() {
            *t = text.to_string();
        }
        if let Ok(mut o) = self.owns_clipboard.lock() {
            *o = true;
        }
        if let Some(ref dm) = self.data_device_manager {
            if let Some(ref dd) = self.data_device {
                let source = dm.create_data_source();
                source.offer("text/plain;charset=utf-8".to_string());
                let ct = self.clipboard_text.clone();
                source.quick_assign(move |_, event, _| {
                    if let wl_data_source::Event::Send { mime_type: _, fd } = event {
                        if let Ok(text) = ct.lock() {
                            let bytes = text.as_bytes();
                            let file = unsafe { File::from_raw_fd(fd) };
                            use std::io::Write;
                            let _ = (&file).write_all(bytes);
                            let _ = (&file).flush();
                        }
                    }
                });
                dd.set_selection(Some(&source), 0);
            }
        }
    }
    fn has_text(&self) -> bool {
        self.clipboard_text
            .lock()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    }
}
