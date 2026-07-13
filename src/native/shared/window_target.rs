use std::collections::HashMap;

use crate::core::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FocusedSurface {
    surface_id: u32,
    window_id: WindowId,
}

/// Maps native surface identities to application windows and tracks seat focus.
///
/// Wayland pointer and keyboard objects are shared by all windows on a seat, so
/// subsequent events without a surface argument must inherit the most recent
/// matching enter event. Unknown surfaces clear the relevant focus rather than
/// allowing input to leak to the previously focused window.
#[derive(Debug, Default)]
pub(crate) struct SurfaceWindowTargets {
    surfaces: HashMap<u32, WindowId>,
    pointer_focus: Option<FocusedSurface>,
    keyboard_focus: Option<FocusedSurface>,
}

impl SurfaceWindowTargets {
    pub(crate) fn register_surface(&mut self, surface_id: u32, window_id: WindowId) {
        if self.surfaces.insert(surface_id, window_id).is_some() {
            if self
                .pointer_focus
                .is_some_and(|focus| focus.surface_id == surface_id)
            {
                self.pointer_focus = None;
            }
            if self
                .keyboard_focus
                .is_some_and(|focus| focus.surface_id == surface_id)
            {
                self.keyboard_focus = None;
            }
        }
    }

    pub(crate) fn unregister_surface(&mut self, surface_id: u32) -> Option<WindowId> {
        if self
            .pointer_focus
            .is_some_and(|focus| focus.surface_id == surface_id)
        {
            self.pointer_focus = None;
        }
        if self
            .keyboard_focus
            .is_some_and(|focus| focus.surface_id == surface_id)
        {
            self.keyboard_focus = None;
        }
        self.surfaces.remove(&surface_id)
    }

    pub(crate) fn window_for_surface(&self, surface_id: u32) -> Option<WindowId> {
        self.surfaces.get(&surface_id).copied()
    }

    pub(crate) fn pointer_enter(&mut self, surface_id: u32) -> Option<WindowId> {
        let window_id = self.window_for_surface(surface_id);
        self.pointer_focus = window_id.map(|window_id| FocusedSurface {
            surface_id,
            window_id,
        });
        window_id
    }

    pub(crate) fn pointer_leave(&mut self, surface_id: u32) -> bool {
        if self
            .pointer_focus
            .is_some_and(|focus| focus.surface_id == surface_id)
        {
            self.pointer_focus = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn pointer_target(&self) -> Option<WindowId> {
        self.pointer_focus.map(|focus| focus.window_id)
    }

    pub(crate) fn clear_pointer_focus(&mut self) {
        self.pointer_focus = None;
    }

    pub(crate) fn keyboard_enter(&mut self, surface_id: u32) -> Option<WindowId> {
        let window_id = self.window_for_surface(surface_id);
        self.keyboard_focus = window_id.map(|window_id| FocusedSurface {
            surface_id,
            window_id,
        });
        window_id
    }

    pub(crate) fn keyboard_leave(&mut self, surface_id: u32) -> bool {
        if self
            .keyboard_focus
            .is_some_and(|focus| focus.surface_id == surface_id)
        {
            self.keyboard_focus = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn keyboard_target(&self) -> Option<WindowId> {
        self.keyboard_focus.map(|focus| focus.window_id)
    }

    pub(crate) fn clear_keyboard_focus(&mut self) {
        self.keyboard_focus = None;
    }
}
