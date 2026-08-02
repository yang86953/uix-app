// ============================================================================
// platform/linux/wayland/window.rs — IWindowManager impl（窗口工厂）
//
// 使用 PlatformWindowCore<WaylandWindowOps> + WaylandPresenter
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{Error, WindowId};
use crate::native::capabilities::*;
use crate::native::platform::*;
use crate::native::present::*;
use crate::native::presentation::*;
use crate::native::windowing::shared::{PlatformWindowCore, WindowState};
use crate::native::windowing::*;

use super::window_ops::WaylandWindowOps;
use super::WaylandBackend;

impl IWindowManager for WaylandBackend {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        self.ensure_seat_and_input();

        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            window_id, width, height,
        )));

        let mut ops = WaylandWindowOps::new(
            window_id,
            self._compositor.clone(),
            self._shm.clone(),
            self.events.clone(),
            self.surface_windows.clone(),
            self.outputs.clone(),
            self._xdg_activation.clone(),
        );

        ops.init(
            &self._wm_base,
            &self._globals,
            &self.display,
            &mut self.event_queue,
            &mut self.dispatch_state,
            &self.pointer,
            title,
            width,
            height,
            Rc::clone(&state),
        )?;

        let presenter = super::presenter::WaylandPresenter::new(
            self._compositor.clone(),
            self._shm.clone(),
            ops.surface.clone(),
            width,
            height,
        );

        let core = PlatformWindowCore::new(state, ops, Box::new(presenter));
        Ok(Box::new(core))
    }
}
