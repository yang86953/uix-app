// ============================================================================
// platform/linux/wayland/window.rs — IWindowManager impl（窗口工厂）
//
// 使用 PlatformWindowCore<WaylandWindowOps> + WaylandPresenter
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{Errc, Error, WindowId};
use crate::native::windowing::shared::{PlatformWindowCore, WindowState};
use crate::platform::windowing::WindowSurfaceRole;
use crate::platform::windowing::window::{IWindowManager, PlatformWindow};

use super::WaylandBackend;
// 逐窗 scale owner 把 Wayland output 事实桥接到 presentation。
use super::surface_scale::WaylandWindowScaleState;
use super::window_ops::WaylandWindowOps;

impl IWindowManager for WaylandBackend {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        self.create_window_with_role(title, width, height, &WindowSurfaceRole::Toplevel)
    }

    fn create_window_with_role(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
        surface_role: &WindowSurfaceRole,
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
        // 每个窗口独占 output 进入集合，并共享 backend 的默认 output scale。
        let surface_scale = WaylandWindowScaleState::new(
            // 绑定目标窗口身份，确保 scale resize 不会串窗。
            window_id,
            // 初始 logical width 来自窗口工厂参数。
            width,
            // 初始 logical height 来自窗口工厂参数。
            height,
            // 尚无 Enter 事件时采用 backend 默认 output scale。
            self.output_scales.preferred_scale(),
            // scale 变化进入 backend 已有逐窗事件队列。
            self.events.clone(),
            // callback 错误继续写入 runtime 唯一 failure source。
            self.pending_failures.clone(),
        );

        let mut ops = WaylandWindowOps::new(
            window_id,
            width,
            height,
            self._compositor.clone(),
            self._shm.clone(),
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
            // 所有窗口共享 backend 唯一 output scale registry。
            self.output_scales.clone(),
            // 当前窗口持有自己的 output 进入集合与 surface metrics。
            surface_scale.clone(),
            // 只有 seat 已建立真实 data-device owner 时能力入口才能成功。
            self.data_device.is_some(),
            self._xdg_activation.clone(),
            surface_role.clone(),
        );

        ops.init(
            &self._wm_base,
            self.layer_shell.as_ref(),
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

        // layer-shell 的首个 configure 可以把零维请求解析为真实输出尺寸。
        let (presenter_width, presenter_height) = {
            let configured = state.borrow();
            (configured.width, configured.height)
        };
        if presenter_width <= 0 || presenter_height <= 0 {
            return Err(Error::new(
                Errc::PlatformError,
                "Wayland compositor did not configure a positive surface extent",
            ));
        }

        let presenter = super::presenter::WaylandPresenter::new(
            self._shm.clone(),
            ops.surface.clone(),
            presenter_width,
            presenter_height,
            // CPU presenter 与 EGL descriptor 消费同一逐窗 metrics。
            surface_scale.metrics(),
        );

        let core = PlatformWindowCore::new(state, ops, Box::new(presenter));
        Ok(Box::new(core))
    }
}
