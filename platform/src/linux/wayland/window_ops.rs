// ============================================================================
// platform/linux/wayland/window_ops.rs — Wayland 平台窗口操作
//
// WaylandWindowOps 实现 WindowOps trait，封装 Wayland 协议窗口调用。
// 与 PlatformWindowCore<WaylandWindowOps> 组合使用。
//
// SHM 像素呈现由独立的 WaylandPresenter 处理。
// ============================================================================

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

use wayland_client::protocol::{wl_compositor, wl_region, wl_shm, wl_surface};
use wayland_client::Main;
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols::misc::server_decoration::client::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration;
use wayland_protocols::staging::xdg_activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::unstable::xdg_decoration::v1::client::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1;

use crate::core::WindowOps;
use crate::core::unimpl;
use crate::event::UiEvent;

/// Wayland 平台窗口操作句柄。
///
/// 持有 Wayland 协议窗口对象（surface/toplevel/xdg_surface），
/// 所有 `os_*` 方法通过 Wayland 协议操作窗口。
pub(crate) struct WaylandWindowOps {
    pub(crate) surface: Option<Main<wl_surface::WlSurface>>,
    pub(crate) xdg_surface: Option<Main<xdg_surface::XdgSurface>>,
    pub(crate) toplevel: Option<Main<xdg_toplevel::XdgToplevel>>,
    pub(crate) compositor: Main<wl_compositor::WlCompositor>,
    pub(crate) shm: Main<wl_shm::WlShm>,
    pub(crate) input_region: Option<Main<wl_region::WlRegion>>,
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,
    /// KDE 服务器端装饰对象（需维持生命周期以避免装饰被撤销）
    pub(crate) kde_decoration: Option<Main<OrgKdeKwinServerDecoration>>,
    /// xdg-decoration 装饰对象（需维持生命周期以避免装饰被撤销）
    pub(crate) xdg_decoration: Option<Main<ZxdgToplevelDecorationV1>>,
    /// 显示器信息，用于计算居中位置
    pub(crate) outputs: Arc<Mutex<Vec<super::output::RawOutput>>>,
    /// xdg_activation 协议，用于请求窗口激活（raise）
    pub(crate) xdg_activation: Option<Main<XdgActivationV1>>,
}

impl WaylandWindowOps {
    /// 获取 wl_surface 的原始 C 指针（供 EGL wl_egl_window_create 使用）。
    /// 此指针仅在窗口生命周期内有效。
    pub(crate) fn surface_c_ptr(&self) -> *mut std::ffi::c_void {
        self.surface.as_ref().map_or(std::ptr::null_mut(), |s| {
            // Main<WlSurface> → Attached<Proxy<WlSurface>> → Proxy<WlSurface>
            // Proxy 内的 inner (ProxyInner) 持有 *mut wl_proxy
            // 我们通过 id() 对应的方式获取指针：
            // 实际上 wayland 协议中 wl_proxy 指针就是 surface 指针
            let s_ref: &Main<wl_surface::WlSurface> = s;
            let ptr = s_ref as *const Main<wl_surface::WlSurface> as *const *const std::ffi::c_void;
            unsafe { *ptr as *mut std::ffi::c_void }
        })
    }

    pub(crate) fn new(
        compositor: Main<wl_compositor::WlCompositor>,
        shm: Main<wl_shm::WlShm>,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        outputs: Arc<Mutex<Vec<super::output::RawOutput>>>,
        xdg_activation: Option<Main<XdgActivationV1>>,
    ) -> Self {
        Self {
            surface: None,
            xdg_surface: None,
            toplevel: None,
            compositor,
            shm,
            input_region: None,
            events,
            kde_decoration: None,
            xdg_decoration: None,
            outputs,
            xdg_activation,
        }
    }

    /// 初始化 Wayland 窗口：创建 surface/toplevel、设置装饰、绑定事件回调。
    pub(crate) fn init(
        &mut self,
        wm_base: &Main<xdg_wm_base::XdgWmBase>,
        globals: &wayland_client::GlobalManager,
        display: &wayland_client::Display,
        event_queue: &mut wayland_client::EventQueue,
        pointer: &Arc<Mutex<Option<Main<wayland_client::protocol::wl_pointer::WlPointer>>>>,
        title: &str,
        width: i32,
        height: i32,
        maximized: Arc<Mutex<bool>>,
        fullscreen: Arc<Mutex<bool>>,
    ) -> Result<(), crate::Error> {
        use wayland_protocols::misc::server_decoration::client::{
            org_kde_kwin_server_decoration::Mode,
            org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager,
        };
        use wayland_protocols::unstable::xdg_decoration::v1::client::{
            zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
            zxdg_toplevel_decoration_v1::Mode as XdgDecoMode,
        };
        use crate::event::{UiEventType, UiEventPayload};

        let events = self.events.clone();

        let surface = self.compositor.create_surface();
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
        let max_state = maximized;
        let fs_state = fullscreen;
        tl.quick_assign(move |_, event, _| {
            match event {
                xdg_toplevel::Event::Close => {
                    let _ = tl_events.lock().unwrap_or_else(|e| e.into_inner())
                        .push_back(UiEvent::close());
                }
                xdg_toplevel::Event::Configure { width: w, height: h, states } => {
                    let is_max = states.chunks_exact(4).any(|c|
                        c.len() == 4 && u32::from_ne_bytes([c[0],c[1],c[2],c[3]]) == 1);
                    let is_full = states.chunks_exact(4).any(|c|
                        c.len() == 4 && u32::from_ne_bytes([c[0],c[1],c[2],c[3]]) == 2);
                    let was_max = max_state.lock().map(|m| *m).unwrap_or(false);
                    if let Ok(mut m) = max_state.lock() { *m = is_max; }
                    if let Ok(mut f) = fs_state.lock() { *f = is_full; }
                    if w > 0 && h > 0 {
                        let _ = tl_events.lock().unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent::resize(w, h));
                    }
                    if is_max && !was_max {
                        let _ = tl_events.lock().unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent { type_: UiEventType::WindowMaximize, payload: UiEventPayload::None });
                    } else if !is_max && was_max {
                        let _ = tl_events.lock().unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent { type_: UiEventType::WindowRestore, payload: UiEventPayload::None });
                    }
                }
                _ => {}
            }
        });

        // 窗口装饰 — 维持装饰对象生命周期，防止过早销毁导致装饰被撤销
        let got_kde = globals
            .instantiate_exact::<OrgKdeKwinServerDecorationManager>(1)
            .map(|dm| {
                let d = dm.create(&surface);
                d.request_mode(Mode::Server);
                self.kde_decoration = Some(d);
                log::info!("[Wayland] KDE server-side decoration requested");
                true
            })
            .unwrap_or(false);
        if !got_kde {
            if let Ok(dm) = globals
                .instantiate_exact::<ZxdgDecorationManagerV1>(1)
            {
                let d = dm.get_toplevel_decoration(&tl);
                d.set_mode(XdgDecoMode::ServerSide);
                self.xdg_decoration = Some(d);
                log::info!("[Wayland] xdg-decoration ServerSide mode requested");
            } else {
                log::warn!("[Wayland] 无可用的窗口装饰协议，窗口可能无标题栏");
            }
        }

        xdg_surf.set_window_geometry(0, 0, width, height);
        {
            let region = self.compositor.create_region();
            region.add(0, 0, width, height);
            surface.set_input_region(Some(&region));
            self.input_region = Some(region);
        }
        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surf);
        self.toplevel = Some(tl);

        let _ = event_queue.dispatch(&mut (), |_, _, _| {});
        for _ in 0..5 {
            let has_ptr = pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_ptr { break; }
            let _ = display.flush();
            let _ = event_queue.dispatch(&mut (), |_, _, _| {});
        }
        if let Some(ref s) = self.surface { s.commit(); }
        let _ = display.flush();
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps — Wayland 协议实现
// ════════════════════════════════════════════════════════════════════════════

impl WindowOps for WaylandWindowOps {
    // ── 窗口生命周期 ──────────────────────────────────────

    fn os_show(&mut self) {
        if let Some(ref s) = self.surface { s.commit(); }
    }

    fn os_hide(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_minimized(); }
    }

    fn os_close(&mut self) {
        self.toplevel = None;
        self.xdg_surface = None;
        self.surface = None;
        self.input_region = None;
    }

    // ── 窗口外观 ──────────────────────────────────────────

    fn os_set_title(&mut self, title: &str) {
        if let Some(ref t) = self.toplevel { t.set_title(title.to_string()); }
    }

    fn os_center_on_screen(&mut self) {
        // Wayland 不支持客户端设置窗口位置，compositor 自行决定放置。
        // 但我们可以读取显示器几何信息，供调试和日志参考。
        let outputs = self.outputs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(primary) = outputs.iter().find(|o| o.is_primary).or_else(|| outputs.first())
        {
            let cx = primary.x + primary.width / 2;
            let cy = primary.y + primary.height / 2;
            log::info!(
                "窗口居中计算完成: 显示器 {}x{} @({},{}), 中心 ({},{})",
                primary.width, primary.height, primary.x, primary.y, cx, cy,
            );
        } else {
            log::info!("窗口居中: 未检测到显示器信息，由 compositor 自行放置");
        }
    }

    fn os_raise(&mut self) {
        // 通过 xdg_activation_v1 请求窗口激活（提升聚焦）。
        // 注意：此协议需要异步 done 事件获取 token 字符串，在同步上下文中
        // 无法等待；携带空 token 的 activate 请求部分 compositor 仍会处理。
        if let (Some(xa), Some(surface)) = (self.xdg_activation.as_ref(), self.surface.as_ref()) {
            let token = xa.get_activation_token();
            token.set_surface(surface);
            token.set_app_id("belldandy".to_string());
            token.commit();
            xa.activate(String::new(), surface);
            log::info!("已请求窗口激活 (xdg_activation_v1)");
        } else {
            log::info!("请求窗口提升: xdg_activation_v1 不可用，由 compositor 自行决定");
        }
    }

    fn os_lower(&mut self) {
        unimpl("os_lower"); // Wayland 不支持程序化窗口层级
    }

    fn os_set_icon(&mut self, _path: &str) {
        unimpl("os_set_icon");
    }

    fn os_flash(&mut self) {
        unimpl("os_flash");
    }

    // ── 尺寸/位置 ─────────────────────────────────────────

    fn os_set_size(&mut self, w: i32, h: i32) {
        if let Some(ref xs) = self.xdg_surface {
            xs.set_window_geometry(0, 0, w, h);
        }
        self.input_region = None;
        if let Some(ref s) = self.surface {
            let region = self.compositor.create_region();
            region.add(0, 0, w, h);
            s.set_input_region(Some(&region));
            self.input_region = Some(region);
        }
    }

    fn os_set_min_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_min_size(w, h); }
    }

    fn os_set_max_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_max_size(w, h); }
    }

    fn os_set_position(&mut self, _x: i32, _y: i32) {
        unimpl("os_set_position"); // Wayland 不允许客户端设置窗口位置
    }

    /// 窗口尺寸变化通知。更新 xdg_surface 窗口几何和输入区域，
    /// 确保 compositor（如 niri）的布局与窗口实际尺寸一致。
    fn os_resize_notify(&mut self, w: i32, h: i32) {
        self.os_set_size(w, h);
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.surface_c_ptr()
    }

    // ── 窗口状态 ──────────────────────────────────────────

    fn os_set_resizable(&mut self, _resizable: bool) {
        unimpl("os_set_resizable"); // compositor 控制
    }

    fn os_maximize(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_maximized(); }
    }

    fn os_minimize(&mut self) {
        if let Some(ref t) = self.toplevel { t.set_minimized(); }
    }

    fn os_restore(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.unset_maximized();
            t.unset_fullscreen();
        }
    }

    fn os_set_borderless(&mut self, _borderless: bool) {
        unimpl("os_set_borderless");
    }

    fn os_set_fullscreen(&mut self, fullscreen: bool) {
        if let Some(ref t) = self.toplevel {
            if fullscreen { t.set_fullscreen(None); } else { t.unset_fullscreen(); }
        }
    }

    fn os_set_always_on_top(&mut self, _on: bool) {
        unimpl("os_set_always_on_top");
    }

    fn os_set_opacity(&mut self, _opacity: f32) {
        unimpl("os_set_opacity");
    }

    // ── 特性开关 ──────────────────────────────────────────

    fn os_start_text_input(&mut self) {
        // 文本输入通过 WaylandBackend 的 text_input_manager 管理
    }

    fn os_stop_text_input(&mut self) {
        // 文本输入通过 WaylandBackend 的 text_input_manager 管理
    }

    fn os_enable_file_drop(&mut self, _enable: bool) {
        unimpl("os_enable_file_drop");
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.surface.as_ref().map_or(std::ptr::null_mut(), |s| {
            s as *const Main<wl_surface::WlSurface> as *mut std::ffi::c_void
        })
    }
}
