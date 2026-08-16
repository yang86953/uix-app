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

    // 返回当前 pointer focus 的原生 surface 与稳定窗口身份快照。
    pub(crate) fn pointer_target_identity(&self) -> Option<(u32, WindowId)> {
        // 同一次不可变读取保证 surface 与窗口身份来自同一 focus。
        self.pointer_focus.map(|focus| {
            // 仅投影平台路由需要的两个稳定值。
            (focus.surface_id, focus.window_id)
            // 结束 focus 身份投影。
        })
        // 结束 pointer focus 身份读取。
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

    // 返回当前 keyboard focus 的原生 surface 与稳定窗口身份快照。
    pub(crate) fn keyboard_target_identity(&self) -> Option<(u32, WindowId)> {
        // 同一次不可变读取保证 surface 与窗口身份来自同一 focus。
        self.keyboard_focus.map(|focus| {
            // 仅投影平台路由需要的两个稳定值。
            (focus.surface_id, focus.window_id)
            // 结束 focus 身份投影。
        })
        // 结束 keyboard focus 身份读取。
    }

    pub(crate) fn clear_keyboard_focus(&mut self) {
        self.keyboard_focus = None;
    }
}
