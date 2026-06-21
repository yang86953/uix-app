// ============================================================================
// platform/linux/wayland/window.rs — IWindowManager + IWindowProperties impl
// ============================================================================

use uix_core::Point;
use uix_diag::Error;
use crate::*;

use super::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for WaylandBackend {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<(), Error> {
        self.create_window_inner(title, width, height)
    }
    fn destroy_window(&mut self) {
        self.decoration = None;
        self.decoration_manager = None;
        self.xdg_toplevel_decoration = None;
        self.xdg_decoration_manager = None;
        self.input_region = None;
        self.shm_buffers = [None, None];
        self.toplevel = None;
        self.xdg_surface = None;
        self.surface = None;
        self.shown = false;
        self.seat = None;
        *self.pointer.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.keyboard.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    fn set_title(&mut self, title: &str) {
        if let Some(ref t) = self.toplevel { t.set_title(title.to_string()); }
    }
    fn show(&mut self) {
        if !self.shown {
            self.shown = true;
            if let Some(ref s) = self.surface {
                if !self.configured {
                    let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
                }
                s.commit();
            }
        }
    }
    fn hide(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_minimized(); }
    }
    fn is_visible(&self) -> bool { self.shown }
    fn center_on_screen(&mut self) {}
    fn raise(&mut self) {}
    fn lower(&mut self) {}
    fn set_window_icon(&mut self, _: &str) {
        log::warn!("Wayland: set_window_icon not implemented");
    }
    fn flash_window(&mut self) {
        log::warn!("Wayland: flash_window not implemented");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for WaylandBackend {
    fn width(&self) -> i32 { self.width }
    fn height(&self) -> i32 { self.height }

    fn set_size(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        if let Some(ref xs) = self.xdg_surface { xs.set_window_geometry(0, 0, w, h); }
        self.input_region = None;
    }
    fn set_minimum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_min_size(w, h); }
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_max_size(w, h); }
    }
    fn position(&self) -> Point { Point::default() }
    fn set_position(&mut self, _: i32, _: i32) {}
    fn set_resizable(&mut self, _: bool) {}
    fn is_maximized(&self) -> bool {
        self.maximized.lock().map(|m| *m).unwrap_or(false)
    }
    fn is_minimized(&self) -> bool { false }
    fn maximize(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_maximized(); }
    }
    fn minimize(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_minimized(); }
    }
    fn restore(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.unset_maximized();
            t.unset_fullscreen();
        }
    }
    fn set_borderless(&mut self, _: bool) {
        log::warn!("Wayland: set_borderless not directly supported");
    }
    fn set_fullscreen(&mut self, fullscreen: bool) {
        if let Some(ref t) = self.toplevel {
            if fullscreen { t.set_fullscreen(None); } else { t.unset_fullscreen(); }
        }
    }
    fn is_fullscreen(&self) -> bool {
        self.fullscreen.lock().map(|f| *f).unwrap_or(false)
    }
    fn set_always_on_top(&mut self, _: bool) {
        log::warn!("Wayland: set_always_on_top not supported");
    }
    fn set_window_opacity(&mut self, _: f32) {
        log::warn!("Wayland: set_window_opacity not supported");
    }
    fn start_text_input(&mut self) {
        log::warn!("Wayland: text_input protocol not yet implemented");
    }
    fn stop_text_input(&mut self) {}
    fn enable_file_drop(&mut self, _: bool) {
        log::warn!("Wayland: file drop not yet implemented");
    }
}
