// ============================================================================
// platform/linux/wayland/window.rs — IWindowManager impl（窗口工厂）
//
// 使用 PlatformWindowCore<WaylandWindowOps> + WaylandPresenter
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{Errc, Error, WindowId};
use crate::native::windowing::shared::{PlatformWindowCore, WindowState};
use crate::native::windowing::*;

use super::WaylandBackend;
use super::window_ops::WaylandWindowOps;

impl IWindowManager for WaylandBackend {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        // closed 事实必须成为窗口工厂访问任何 seat 或协议 owner 前的首个决策。
        if self.closed {
            // 致命关闭后的创建请求返回稳定生命周期错误，禁止复活 backend。
            return Err(Error::new(
                // 使用 InvalidState 区分关闭生命周期与能力缺失。
                Errc::InvalidState,
                // 保留窗口工厂与 backend shutdown 上下文。
                "Wayland create_window requested after backend shutdown",
            ));
        }
        self.ensure_seat_and_input();

        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            window_id, width, height,
        )));

        let mut ops = WaylandWindowOps::new(
            window_id,
            self._compositor.clone(),
            self.events.clone(),
            // 所有窗口 callback 复用 backend 已绑定的 owner-thread failure source。
            self.pending_failures.clone(),
            self.surface_windows.clone(),
            // 传入已绑定 seat 的协议引用，不复制任何输入 serial。
            self.seat.clone(),
            // 所有窗口共享后端唯一的一次性激活注册表。
            self.pointer_activations.clone(),
            // 所有窗口共享 backend 唯一的文件拖放 Component。
            self.file_drop_state.clone(),
            // 只有 seat 已建立真实 data-device owner 时能力入口才能成功。
            self.data_device.is_some(),
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
            self._shm.clone(),
            ops.surface.clone(),
            width,
            height,
        );

        let core = PlatformWindowCore::new(state, ops, Box::new(presenter));
        Ok(Box::new(core))
    }
}
