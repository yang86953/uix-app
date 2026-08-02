// ============================================================================
// platform/linux/wayland/window_ops.rs — Wayland 平台窗口操作
//
// WaylandWindowOps 实现 WindowOps trait，封装 Wayland 协议窗口调用。
// 与 PlatformWindowCore<WaylandWindowOps> 组合使用。
//
// SHM 像素呈现由独立的 WaylandPresenter 处理。
// ============================================================================

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::compat::Main;
use wayland_client::protocol::{wl_callback, wl_compositor, wl_region, wl_shm, wl_surface};
use wayland_client::{globals::GlobalList, Connection, EventQueue};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::core::error::{Errc, Error, Result};
use crate::core::WindowId;
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle;
use crate::native::windowing::event::{FrameRequestToken, UiEvent};
use crate::native::windowing::shared::window_mode::{
    NativeMaximizeTransition, NativeWindowModeState,
};
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;
use crate::native::windowing::shared::{unimpl, WindowOps, WindowState};
use crate::native::windowing::window::{NativeFrameRequest, NativeFrameRequestPhase};

use super::compat::WaylandDispatchState;

/// Wayland 平台窗口操作句柄。
///
/// 持有 Wayland 协议窗口对象（surface/toplevel/xdg_surface），
/// 所有 `os_*` 方法通过 Wayland 协议操作窗口。
pub(crate) struct WaylandWindowOps {
    pub(crate) window_id: WindowId,
    native_surface: WaylandSurfaceHandle,
    pub(crate) surface: Option<Main<wl_surface::WlSurface>>,
    surface_id: Option<u32>,
    pub(crate) xdg_surface: Option<Main<xdg_surface::XdgSurface>>,
    pub(crate) toplevel: Option<Main<xdg_toplevel::XdgToplevel>>,
    pub(crate) compositor: Main<wl_compositor::WlCompositor>,
    pub(crate) shm: Main<wl_shm::WlShm>,
    pub(crate) input_region: Option<Main<wl_region::WlRegion>>,
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,
    surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
    frame_request: Arc<Mutex<Option<NativeFrameRequest>>>,
    configured_modes: Arc<Mutex<NativeWindowModeState>>,
    /// xdg-decoration 装饰对象（需维持生命周期以避免装饰被撤销）
    pub(crate) xdg_decoration: Option<Main<ZxdgToplevelDecorationV1>>,
    /// 显示器信息，用于计算居中位置
    pub(crate) outputs: Arc<Mutex<Vec<super::output::RawOutput>>>,
    /// xdg_activation 协议，用于请求窗口激活（raise）
    pub(crate) xdg_activation: Option<Main<XdgActivationV1>>,
}

impl WaylandWindowOps {
    fn missing_proxy(operation: &str, proxy: &str) -> Error {
        Error::new(
            Errc::InvalidState,
            format!("{operation}: Wayland {proxy} is unavailable"),
        )
    }

    /// 获取 wl_surface 的原始 C 指针（供 EGL wl_egl_window_create 使用）。
    /// 此指针仅在窗口生命周期内有效。
    pub(crate) fn surface_c_ptr(&self) -> *mut std::ffi::c_void {
        self.surface.as_ref().map_or(std::ptr::null_mut(), |s| {
            // Main<WlSurface> → Attached<Proxy<WlSurface>> → Proxy<WlSurface>
            // Proxy 内的 inner (ProxyInner) 持有 *mut wl_proxy
            // 我们通过 id() 对应的方式获取指针：
            // 实际上 wayland 协议中 wl_proxy 指针就是 surface 指针
            s.c_ptr()
        })
    }

    fn native_surface_descriptor_ptr(&self) -> *mut std::ffi::c_void {
        if self.native_surface.is_valid() {
            &self.native_surface as *const WaylandSurfaceHandle as *mut std::ffi::c_void
        } else {
            std::ptr::null_mut()
        }
    }

    pub(crate) fn new(
        window_id: WindowId,
        compositor: Main<wl_compositor::WlCompositor>,
        shm: Main<wl_shm::WlShm>,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
        outputs: Arc<Mutex<Vec<super::output::RawOutput>>>,
        xdg_activation: Option<Main<XdgActivationV1>>,
    ) -> Self {
        Self {
            window_id,
            native_surface: WaylandSurfaceHandle::default(),
            surface: None,
            surface_id: None,
            xdg_surface: None,
            toplevel: None,
            compositor,
            shm,
            input_region: None,
            events,
            surface_windows,
            frame_request: Arc::new(Mutex::new(None)),
            configured_modes: Arc::new(Mutex::new(NativeWindowModeState::default())),
            xdg_decoration: None,
            outputs,
            xdg_activation,
        }
    }

    fn unregister_surface(&mut self) {
        let Some(surface_id) = self.surface_id.take() else {
            return;
        };
        self.surface_windows
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .unregister_surface(surface_id);
    }

    /// 初始化 Wayland 窗口：创建 surface/toplevel、设置装饰、绑定事件回调。
    pub(crate) fn init(
        &mut self,
        wm_base: &Main<xdg_wm_base::XdgWmBase>,
        globals: &GlobalList,
        display: &Connection,
        event_queue: &mut EventQueue<WaylandDispatchState>,
        dispatch_state: &mut WaylandDispatchState,
        pointer: &Arc<Mutex<Option<Main<wayland_client::protocol::wl_pointer::WlPointer>>>>,
        title: &str,
        width: i32,
        height: i32,
        window_state: Rc<RefCell<WindowState>>,
    ) -> Result<(), Error> {
        use crate::native::windowing::event::{UiEventPayload, UiEventType};
        use wayland_protocols::xdg::decoration::zv1::client::{
            zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
            zxdg_toplevel_decoration_v1::Mode as XdgDecoMode,
        };

        let events = self.events.clone();
        let window_id = self.window_id;

        let surface = self.compositor.create_surface();
        let surface_id = surface.id().protocol_id();
        let xdg_surf = wm_base.get_xdg_surface(&surface);
        let tl = xdg_surf.get_toplevel();
        tl.set_title(title.to_string());
        tl.set_app_id("uix-app".to_string());

        xdg_surf.quick_assign(move |xs, event, _| {
            if let xdg_surface::Event::Configure { serial } = event {
                xs.ack_configure(serial);
            }
        });

        let tl_events = events.clone();
        let configured_modes = Arc::clone(&self.configured_modes);
        tl.quick_assign(move |_, event, _| match event {
            xdg_toplevel::Event::Close => {
                let _ = tl_events
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push_back(UiEvent::close().for_window(window_id));
            }
            xdg_toplevel::Event::Configure {
                width: w,
                height: h,
                states,
            } => {
                let is_max = states
                    .chunks_exact(4)
                    .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 1);
                let is_full = states
                    .chunks_exact(4)
                    .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 2);
                let (transition, maximized, fullscreen) = {
                    let mut modes = configured_modes
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    let transition = modes.apply_configure(is_max, is_full);
                    let (maximized, fullscreen) = modes.snapshot();
                    (transition, maximized, fullscreen)
                };
                {
                    let mut state = window_state.borrow_mut();
                    state.maximized = maximized;
                    state.fullscreen = fullscreen;
                }
                let mut queued = tl_events.lock().unwrap_or_else(|error| error.into_inner());
                if w > 0 && h > 0 {
                    queued.push_back(UiEvent::resize(w, h).for_window(window_id));
                }
                match transition {
                    NativeMaximizeTransition::Maximized => {
                        queued.push_back(UiEvent {
                            window_id: Some(window_id),
                            type_: UiEventType::WindowMaximize,
                            payload: UiEventPayload::None,
                        });
                    }
                    NativeMaximizeTransition::Restored => {
                        queued.push_back(UiEvent {
                            window_id: Some(window_id),
                            type_: UiEventType::WindowRestore,
                            payload: UiEventPayload::None,
                        });
                    }
                    NativeMaximizeTransition::Unchanged => {}
                }
            }
            _ => {}
        });

        // 窗口装饰 — 维持装饰对象生命周期，防止过早销毁导致装饰被撤销
        let queue_handle = self.compositor.queue_handle();
        if let Ok(dm) = globals.bind::<ZxdgDecorationManagerV1, _, _>(&queue_handle, 1..=1, ()) {
            let dm = Main::new(dm, self.compositor.context());
            let d = dm.get_toplevel_decoration(&tl);
            d.set_mode(XdgDecoMode::ServerSide);
            self.xdg_decoration = Some(d);
            tracing::info!("[Wayland] xdg-decoration ServerSide mode requested");
        } else {
            tracing::warn!("[Wayland] no available window decoration protocol");
        }

        xdg_surf.set_window_geometry(0, 0, width, height);
        {
            let region = self.compositor.create_region();
            region.add(0, 0, width, height);
            surface.set_input_region(Some(&region));
            self.input_region = Some(region);
        }
        self.surface_windows
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .register_surface(surface_id, window_id);
        self.surface_id = Some(surface_id);
        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surf);
        self.toplevel = Some(tl);
        self.native_surface =
            WaylandSurfaceHandle::new(display.backend().display_ptr().cast(), self.surface_c_ptr());

        let _ = event_queue.dispatch_pending(dispatch_state);
        for _ in 0..5 {
            let has_ptr = pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_ptr {
                break;
            }
            let _ = display.flush();
            let _ = event_queue.dispatch_pending(dispatch_state);
        }
        if let Some(ref s) = self.surface {
            s.commit();
        }
        let _ = display.flush();
        Ok(())
    }
}

impl Drop for WaylandWindowOps {
    fn drop(&mut self) {
        self.unregister_surface();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps — Wayland 协议实现
// ════════════════════════════════════════════════════════════════════════════

impl WindowOps for WaylandWindowOps {
    // ── 窗口生命周期 ──────────────────────────────────────

    fn os_show(&mut self) -> Result<()> {
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_show", "wl_surface"))?;
        surface.commit();
        Ok(())
    }

    fn os_hide(&mut self) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_hide", "xdg_toplevel"))?;
        toplevel.set_minimized();
        Ok(())
    }

    fn os_close(&mut self) -> Result<()> {
        *self
            .frame_request
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        self.unregister_surface();
        self.toplevel = None;
        self.xdg_surface = None;
        self.surface = None;
        self.input_region = None;
        Ok(())
    }

    // ── 窗口外观 ──────────────────────────────────────────

    fn os_set_title(&mut self, title: &str) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_title", "xdg_toplevel"))?;
        toplevel.set_title(title.to_string());
        Ok(())
    }

    fn os_center_on_screen(&mut self) -> Result<()> {
        unimpl("os_center_on_screen")
    }

    fn os_raise(&mut self) -> Result<()> {
        // 通过 xdg_activation_v1 请求窗口激活（提升聚焦）。
        // 注意：此协议需要异步 done 事件获取 token 字符串，在同步上下文中
        // 无法等待；携带空 token 的 activate 请求部分 compositor 仍会处理。
        let xa = self
            .xdg_activation
            .as_ref()
            .ok_or_else(|| Error::new(Errc::NotImplemented, "xdg_activation_v1 is unavailable"))?;
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_raise", "wl_surface"))?;
        let token = xa.get_activation_token();
        token.set_surface(surface);
        token.set_app_id("belldandy".to_string());
        token.commit();
        xa.activate(String::new(), surface);
        Ok(())
    }

    fn os_lower(&mut self) -> Result<()> {
        unimpl("os_lower") // Wayland 不支持程序化窗口层级
    }

    fn os_set_icon(&mut self, _path: &str) -> Result<()> {
        unimpl("os_set_icon")
    }

    fn os_flash(&mut self) -> Result<()> {
        unimpl("os_flash")
    }

    // ── 尺寸/位置 ─────────────────────────────────────────

    fn os_set_size(&mut self, w: i32, h: i32) -> Result<()> {
        let xdg_surface = self
            .xdg_surface
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_size", "xdg_surface"))?;
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_size", "wl_surface"))?;
        xdg_surface.set_window_geometry(0, 0, w, h);
        self.input_region = None;
        let region = self.compositor.create_region();
        region.add(0, 0, w, h);
        surface.set_input_region(Some(&region));
        self.input_region = Some(region);
        Ok(())
    }

    fn os_set_min_size(&mut self, w: i32, h: i32) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_min_size", "xdg_toplevel"))?;
        toplevel.set_min_size(w, h);
        Ok(())
    }

    fn os_set_max_size(&mut self, w: i32, h: i32) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_max_size", "xdg_toplevel"))?;
        toplevel.set_max_size(w, h);
        Ok(())
    }

    fn os_set_position(&mut self, _x: i32, _y: i32) -> Result<()> {
        unimpl("os_set_position") // Wayland 不允许客户端设置窗口位置
    }

    /// 窗口尺寸变化通知。更新 xdg_surface 窗口几何和输入区域，
    /// 确保 compositor（如 niri）的布局与窗口实际尺寸一致。
    fn os_resize_notify(&mut self, w: i32, h: i32) -> Result<()> {
        self.os_set_size(w, h)
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.native_surface_descriptor_ptr()
    }

    fn os_request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool> {
        if request.phase != NativeFrameRequestPhase::AfterPresent {
            return Ok(false);
        }
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_request_native_frame", "wl_surface"))?;
        {
            let mut active = self
                .frame_request
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if active.as_ref() == Some(&request) {
                return Ok(true);
            }
            *active = Some(request);
        }

        let callback = surface.frame();
        let active = Arc::clone(&self.frame_request);
        let events = Arc::clone(&self.events);
        let window_id = self.window_id;
        callback.quick_assign(move |_, event, _| {
            if !matches!(event, wl_callback::Event::Done { .. }) {
                return;
            }
            let should_deliver = {
                let mut current = active.lock().unwrap_or_else(|error| error.into_inner());
                if current.as_ref() == Some(&request) {
                    *current = None;
                    true
                } else {
                    false
                }
            };
            if should_deliver {
                events
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push_back(
                        UiEvent::frame_opportunity(request.token, Instant::now(), None)
                            .for_window(window_id),
                    );
            }
        });
        Ok(true)
    }

    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
        let mut active = self
            .frame_request
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active
            .as_ref()
            .is_some_and(|request| request.token == token)
        {
            *active = None;
        }
        Ok(())
    }

    // ── 窗口状态 ──────────────────────────────────────────

    fn os_set_resizable(&mut self, _resizable: bool) -> Result<()> {
        unimpl("os_set_resizable") // compositor 控制
    }

    fn os_maximize(&mut self) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_maximize", "xdg_toplevel"))?;
        toplevel.set_maximized();
        Ok(())
    }

    fn os_minimize(&mut self) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_minimize", "xdg_toplevel"))?;
        toplevel.set_minimized();
        Ok(())
    }

    fn os_restore(&mut self) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_restore", "xdg_toplevel"))?;
        toplevel.unset_maximized();
        toplevel.unset_fullscreen();
        Ok(())
    }

    fn os_set_borderless(&mut self, _borderless: bool) -> Result<()> {
        unimpl("os_set_borderless")
    }

    fn os_set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_fullscreen", "xdg_toplevel"))?;
        if fullscreen {
            toplevel.set_fullscreen(None);
        } else {
            toplevel.unset_fullscreen();
        }
        Ok(())
    }

    fn os_set_always_on_top(&mut self, _on: bool) -> Result<()> {
        unimpl("os_set_always_on_top")
    }

    fn os_set_opacity(&mut self, _opacity: f32) -> Result<()> {
        unimpl("os_set_opacity")
    }

    // ── 特性开关 ──────────────────────────────────────────

    fn os_start_text_input(&mut self) -> Result<()> {
        // 文本输入通过 WaylandBackend 的 text_input_manager 管理
        Ok(())
    }

    fn os_stop_text_input(&mut self) -> Result<()> {
        // 文本输入通过 WaylandBackend 的 text_input_manager 管理
        Ok(())
    }

    fn os_enable_file_drop(&mut self, _enable: bool) -> Result<()> {
        unimpl("os_enable_file_drop")
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.surface_c_ptr()
    }
}
