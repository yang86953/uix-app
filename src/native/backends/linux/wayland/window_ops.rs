// platform/linux/wayland/window_ops.rs — Wayland 平台窗口操作
// WaylandWindowOps 实现 WindowOps trait，封装 Wayland 协议窗口调用。
// 与 PlatformWindowCore<WaylandWindowOps> 组合使用。
// SHM 像素呈现由独立的 WaylandPresenter 处理。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::compat::Main;
use wayland_client::backend::WaylandError;
// 引入窗口协议对象以及发起交互移动所需的 seat 类型。
use wayland_client::protocol::{
    // frame callback 保持既有呈现节拍职责。
    wl_callback,
    // compositor 继续创建窗口 surface 与输入区域。
    wl_compositor,
    // region 继续限定客户区输入范围。
    wl_region,
    // seat 只作为 xdg_toplevel.move 的协议参数使用。
    wl_seat,
    // surface 继续标识当前原生窗口表面。
    wl_surface,
    // 结束 Wayland 核心协议类型导入。
};
use wayland_client::{Connection, EventQueue, globals::GlobalList};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
// 引入 Wayland 顶层窗口装饰对象及客户端/服务端装饰模式。
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{
    // 使用短别名表达系统标题栏可见性对应的协议模式。
    Mode as XdgDecoMode,
    // 持有装饰对象直到顶层窗口关闭。
    ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::core::WindowId;
use crate::core::error::{Errc, Error, Result};
// 引入 runtime-scoped callback failure source，禁止窗口对象创建第二队列。
use crate::diagnostics::PendingFailureSource;
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle;
// 引入帧事件、UI 事件与不可解释的指针激活身份。
use crate::native::windowing::event::{FrameRequestToken, PointerActivationId, UiEvent};
use crate::native::windowing::shared::window_mode::{
    NativeMaximizeTransition, NativeWindowModeState,
};
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;
// 直接从共享窗口模块引入 Wayland 需要的未实现操作，避免其他目标产生未使用重导出。
use crate::native::windowing::shared::window::{WindowOps, unimpl};
// 引入跨平台共享的窗口状态。
use crate::native::windowing::shared::WindowState;
use crate::native::windowing::window::{NativeFrameRequest, NativeFrameRequestPhase};

use super::compat::WaylandDispatchState;
// 引入私有 frame callback Component，保持 WindowOps 只编排协议生命周期。
use super::frame_callback::deliver_frame_opportunity;
// 引入 Wayland 私有授权注册表及其非致命消费结果。
use super::pointer_activation::{
    // 消费结果区分可提交 serial 与正常竞态忽略。
    PointerActivationOutcome,
    // 注册表独占 raw serial、surface 代次与 pointer 代次。
    WaylandPointerActivationRegistry,
    // 结束指针激活私有类型导入。
};
// 引入跨注册表原子登记与注销 Component，窗口 owner 只编排协议生命周期。
use super::surface_registration::{register_window_surface, unregister_window_surface};

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
    pub(crate) input_region: Option<Main<wl_region::WlRegion>>,
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,
    // 所有窗口 callback 复用所属 Wayland backend 的同一 failure source。
    pub(super) pending_failures: PendingFailureSource,
    surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
    // seat 代理只用于提交已经通过注册表校验的交互移动请求。
    seat: Option<Main<wl_seat::WlSeat>>,
    // 共享注册表是 raw pointer press serial 的唯一所有者。
    pointer_activations: Arc<Mutex<WaylandPointerActivationRegistry>>,
    frame_request: Arc<Mutex<Option<NativeFrameRequest>>>,
    configured_modes: Arc<Mutex<NativeWindowModeState>>,
    /// xdg-decoration 装饰对象（需维持生命周期以避免装饰被撤销）
    pub(crate) xdg_decoration: Option<Main<ZxdgToplevelDecorationV1>>,
    /// xdg_activation 协议，用于请求窗口激活（raise）
    pub(crate) xdg_activation: Option<Main<XdgActivationV1>>,
}

impl WaylandWindowOps {
    // 将统一的系统标题栏可见性映射为 Wayland 装饰模式。
    fn title_bar_decoration_mode(visible: bool) -> XdgDecoMode {
        // 可见系统标题栏请求 compositor 绘制服装饰。
        if visible {
            // 服务端装饰对应原生标题栏。
            XdgDecoMode::ServerSide
        } else {
            // 客户端装饰把标题栏区域交给 UIX 自己绘制。
            XdgDecoMode::ClientSide
        }
    }

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
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        // 注入所属 Wayland backend 已有的 callback failure source。
        pending_failures: PendingFailureSource,
        surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
        // 注入后端已绑定的 seat 代理引用。
        seat: Option<Main<wl_seat::WlSeat>>,
        // 注入后端唯一的指针激活注册表。
        pointer_activations: Arc<Mutex<WaylandPointerActivationRegistry>>,
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
            input_region: None,
            events,
            // 保存同一 source 的廉价 clone，不建立新的 failure owner。
            pending_failures,
            surface_windows,
            // 保存只用于协议提交的 seat 代理引用。
            seat,
            // 保存授权注册表共享句柄而不复制任何 raw serial。
            pointer_activations,
            frame_request: Arc::new(Mutex::new(None)),
            configured_modes: Arc::new(Mutex::new(NativeWindowModeState::default())),
            xdg_decoration: None,
            xdg_activation,
        }
    }

    // 检查式注销当前 surface，并只在事务成功后清除可重试身份。
    pub(super) fn unregister_surface(&mut self) -> Result<()> {
        // 重复注销保持幂等，不触碰任何共享注册表。
        let Some(surface_id) = self.surface_id else {
            // 当前窗口已经没有活动 surface 注册事实。
            return Ok(());
        };
        // 私有 Component 在任一修改前取得两份健康注册表 guard。
        unregister_window_surface(
            // 传入 raw pointer activation 的唯一共享 owner。
            &self.pointer_activations,
            // 传入 surface 到窗口身份的唯一共享 owner。
            &self.surface_windows,
            // 注销当前协议 surface 编号。
            surface_id,
            // 限定只能删除仍属于当前窗口的授权注册。
            self.window_id,
        )?;
        // 两份注册表均成功注销后才消费窗口持有的可重试身份。
        self.surface_id = None;
        // 显式报告注销事务成功。
        Ok(())
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
        // 初始化阶段只需临时绑定装饰管理器，装饰模式使用模块级统一映射。
        use wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;

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
        // toplevel callback 复用窗口所属 backend 的同一 failure source。
        let toplevel_failures = self.pending_failures.clone();
        tl.quick_assign(move |_, event, _| match event {
            xdg_toplevel::Event::Close => {
                // close 事实只在健康事件队列中提交。
                let Ok(mut queued) = tl_events.lock() else {
                    // 事件队列损坏必须交给 owner-thread failure source。
                    let _ = toplevel_failures.enqueue(Error::new(
                        // callback 无法提交 close 属于稳定 owner 状态错误。
                        Errc::InvalidState,
                        // 保留 xdg_toplevel Close 的精确失败阶段。
                        "Wayland xdg_toplevel Close event queue mutex poisoned",
                    ));
                    // 停止本次 close 提交，禁止访问 recovered 队列。
                    return;
                };
                // 健康队列接收当前窗口唯一 close 事实。
                queued.push_back(UiEvent::close().for_window(window_id));
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
                // 先检查模式 owner，失败时不得开始 Configure 状态事务。
                let Ok(mut modes) = configured_modes.lock() else {
                    // 模式状态损坏必须交给 owner-thread failure source。
                    let _ = toplevel_failures.enqueue(Error::new(
                        // callback 无法访问唯一模式 owner。
                        Errc::InvalidState,
                        // 保留 xdg_toplevel Configure 模式阶段。
                        "Wayland xdg_toplevel Configure mode state mutex poisoned",
                    ));
                    // 停止本次 Configure，禁止访问 recovered 模式状态。
                    return;
                };
                // 再检查 WindowState 借用，冲突不得以 panic 越过 callback adapter。
                let Ok(mut state) = window_state.try_borrow_mut() else {
                    // 可重入状态借用冲突必须交给 owner-thread failure source。
                    let _ = toplevel_failures.enqueue(Error::new(
                        // callback 无法取得唯一窗口状态写权限。
                        Errc::InvalidState,
                        // 保留 xdg_toplevel Configure 状态阶段。
                        "Wayland xdg_toplevel Configure WindowState already borrowed",
                    ));
                    // 尚未改写 modes，安全结束本次 Configure 事务。
                    return;
                };
                // 最后检查事件队列，失败时 modes 与 WindowState 仍保持原值。
                let Ok(mut queued) = tl_events.lock() else {
                    // 事件队列损坏必须交给 owner-thread failure source。
                    let _ = toplevel_failures.enqueue(Error::new(
                        // callback 无法提交同源窗口事件。
                        Errc::InvalidState,
                        // 保留 xdg_toplevel Configure 投递阶段。
                        "Wayland xdg_toplevel Configure event queue mutex poisoned",
                    ));
                    // 尚未改写 modes 或 WindowState，安全结束本次事务。
                    return;
                };
                // 全部 owner 均可用后才提交模式转换。
                let transition = modes.apply_configure(is_max, is_full);
                // 读取刚提交的同源模式快照。
                let (maximized, fullscreen) = modes.snapshot();
                // 同步提交窗口最大化事实。
                state.maximized = maximized;
                // 同步提交窗口全屏事实。
                state.fullscreen = fullscreen;
                // 有效客户区尺寸继续生成 resize 事实。
                if w > 0 && h > 0 {
                    // resize 与本次模式快照进入同一事件事务。
                    queued.push_back(UiEvent::resize(w, h).for_window(window_id));
                }
                // 保持既有最大化/恢复边沿事件语义。
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
        // 登记成功前由局部 RAII owner 持有可选装饰对象。
        let xdg_decoration = if let Ok(dm) =
            globals.bind::<ZxdgDecorationManagerV1, _, _>(&queue_handle, 1..=1, ())
        {
            let dm = Main::new(dm, self.compositor.context());
            let d = dm.get_toplevel_decoration(&tl);
            d.set_mode(XdgDecoMode::ServerSide);
            // 保留既有健康扩展观测信息。
            tracing::info!("[Wayland] xdg-decoration ServerSide mode requested");
            // 健康扩展把装饰对象交给局部 owner。
            Some(d)
        } else {
            tracing::warn!("[Wayland] no available window decoration protocol");
            // 缺少装饰扩展保持既有无对象语义。
            None
        };

        xdg_surf.set_window_geometry(0, 0, width, height);
        // 登记成功前由局部 RAII owner 持有输入区域。
        let input_region = {
            let region = self.compositor.create_region();
            region.add(0, 0, width, height);
            surface.set_input_region(Some(&region));
            // 返回已设置到 surface 的局部区域 owner。
            region
        };
        // 私有 Component 在任何改写前取得两份健康注册表 guard。
        register_window_surface(
            // 传入 raw pointer activation 的唯一共享 owner。
            &self.pointer_activations,
            // 传入 surface 到窗口身份的唯一共享 owner。
            &self.surface_windows,
            // 登记当前协议 surface 编号。
            surface_id,
            // 两份注册事实绑定同一稳定窗口身份。
            window_id,
        )?;
        // 两份注册事实提交后才发布可注销的 surface identity。
        self.surface_id = Some(surface_id);
        // 登记成功后把装饰协议对象转交窗口生命周期 owner。
        self.xdg_decoration = xdg_decoration;
        // 登记成功后把输入区域协议对象转交窗口生命周期 owner。
        self.input_region = Some(input_region);
        // surface identity 与协议 owner 均已准备后再提交。
        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surf);
        self.toplevel = Some(tl);
        self.native_surface =
            WaylandSurfaceHandle::new(display.backend().display_ptr().cast(), self.surface_c_ptr());

        event_queue
            .dispatch_pending(dispatch_state)
            .map_err(|error| {
                Error::new(
                    Errc::PlatformError,
                    format!("Wayland window initialization dispatch failed: {error}"),
                )
            })?;
        for _ in 0..5 {
            let has_ptr = pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_ptr {
                break;
            }
            if let Err(error) = display.flush() {
                if !matches!(
                    error,
                    WaylandError::Io(ref error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                ) {
                    return Err(Error::new(
                        Errc::IoError,
                        format!("Wayland window initialization flush failed: {error}"),
                    ));
                }
            }
            event_queue
                .dispatch_pending(dispatch_state)
                .map_err(|error| {
                    Error::new(
                        Errc::PlatformError,
                        format!("Wayland window initialization dispatch failed: {error}"),
                    )
                })?;
        }
        if let Some(ref s) = self.surface {
            s.commit();
        }
        if let Err(error) = display.flush() {
            if !matches!(
                error,
                WaylandError::Io(ref error) if error.kind() == std::io::ErrorKind::WouldBlock
            ) {
                return Err(Error::new(
                    Errc::IoError,
                    format!("Wayland window initialization flush failed: {error}"),
                ));
            }
        }
        Ok(())
    }
}

impl WindowOps for WaylandWindowOps {
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
        // frame request owner 损坏时不得继续释放窗口协议对象。
        *self.frame_request.lock().map_err(|_| {
            // 构造稳定的窗口关闭阶段错误。
            Error::new(
                // 共享请求状态已无法安全访问。
                Errc::InvalidState,
                // 保留 frame request 与窗口关闭阶段。
                "Wayland frame request mutex poisoned during window close",
            )
        })? = None;
        // 注册表失败同步传播，并保留 surface identity 与协议对象供显式重试。
        self.unregister_surface()?;
        // 装饰对象依赖 xdg_toplevel，必须先于顶层窗口释放。
        self.xdg_decoration = None;
        self.toplevel = None;
        self.xdg_surface = None;
        self.surface = None;
        self.input_region = None;
        Ok(())
    }

    // 应用主动关闭与 compositor 关闭共用同一逐窗事实，由 Application System 执行 teardown。
    fn os_request_close(&mut self) -> Result<()> {
        // 同步窄端口必须把损坏的事件队列作为 typed failure 返回调用方。
        let mut events = self.events.lock().map_err(|_| {
            // 构造稳定的主动关闭队列错误。
            Error::new(
                // 队列 owner 已无法安全访问。
                Errc::InvalidState,
                // 保留 Wayland 主动关闭与逐窗事件队列阶段。
                "Wayland request-close event queue mutex poisoned",
            )
        })?;
        // 健康队列继续接收唯一的逐窗关闭事实。
        events.push_back(UiEvent::close().for_window(self.window_id));
        // 成功表示关闭意图已交付，不表示 surface 已被销毁。
        Ok(())
    }

    // 将当前 PointerDown 的一次性授权提交给 xdg_toplevel 交互移动协议。
    fn os_begin_move_drag(
        // 窗口操作只在本次同步调用期间借用自身状态。
        &mut self,
        // app 仅转交不可解释身份，raw serial 始终留在 Wayland 注册表。
        pointer_activation: Option<PointerActivationId>,
        // 正常竞态统一安全忽略，协议连接失败仍由既有 pending failure 报告。
    ) -> Result<()> {
        // 没有原生 PointerDown 身份的动作不得猜测或复用任何 serial。
        let Some(pointer_activation) = pointer_activation else {
            // 提供稳定调试诊断且不终止主窗口或副窗口事件循环。
            tracing::debug!(
                // 记录动作所属窗口用于多窗口排查。
                window_id = self.window_id.raw(),
                // 使用固定原因文本支持定向日志检索。
                reason = "missing_pointer_activation",
                // 说明本次 Wayland 移动请求被安全忽略。
                "Wayland interactive move ignored" // 结束无身份诊断参数。
            );
            // 正常竞态或非原生动作不构成平台错误。
            return Ok(());
            // 结束无激活身份分支。
        };
        // 已关闭或尚未登记 surface 的窗口不能消费拖动授权。
        let Some(surface_id) = self.surface_id else {
            // 记录稳定的 surface 生命周期拒绝原因。
            tracing::debug!(
                // 记录动作所属窗口。
                window_id = self.window_id.raw(),
                // 标明原生 surface 当前不可用。
                reason = "surface_unavailable",
                // 说明本次请求被安全忽略。
                "Wayland interactive move ignored" // 结束 surface 缺失诊断参数。
            );
            // surface 缺失是关闭竞态而非致命失败。
            return Ok(());
            // 结束 surface 缺失分支。
        };
        // Component 检查共享 owner 后原子校验并消费一次性授权。
        let outcome = WaylandPointerActivationRegistry::consume_checked(
            // 传入 raw serial 的唯一共享 owner。
            &self.pointer_activations,
            // 校验当前原生 PointerDown 身份。
            pointer_activation,
            // 校验动作所属的稳定窗口身份。
            self.window_id,
            // 校验当前 surface 协议身份与代次。
            surface_id,
        )?;
        // 根据私有消费结果决定提交或安全忽略。
        match outcome {
            // 只有完整匹配的一次性授权可以获得 raw serial。
            PointerActivationOutcome::Authorized { serial } => {
                // capability 初始化或关闭竞态可能使 seat 不可用。
                let Some(seat) = self.seat.as_ref() else {
                    // serial 已消费，缺少 seat 时绝不回填或伪造。
                    tracing::debug!(
                        // 记录动作所属窗口。
                        window_id = self.window_id.raw(),
                        // 标明 seat 协议对象不可用。
                        reason = "seat_unavailable",
                        // 说明请求被安全忽略。
                        "Wayland interactive move ignored" // 结束 seat 缺失诊断参数。
                    );
                    // seat 缺失在交互竞态中保持非致命。
                    return Ok(());
                    // 结束 seat 缺失分支。
                };
                // 窗口关闭竞态可能已经释放 xdg_toplevel。
                let Some(toplevel) = self.toplevel.as_ref() else {
                    // serial 已消费，缺少顶层对象时不允许重试。
                    tracing::debug!(
                        // 记录动作所属窗口。
                        window_id = self.window_id.raw(),
                        // 标明顶层协议对象不可用。
                        reason = "toplevel_unavailable",
                        // 说明请求被安全忽略。
                        "Wayland interactive move ignored" // 结束顶层对象缺失诊断参数。
                    );
                    // 关闭竞态不应结束应用事件循环。
                    return Ok(());
                    // 结束顶层对象缺失分支。
                };
                // 锁已释放，此处只提交同一 press 的 seat 与 serial。
                toplevel._move(seat.as_ref(), serial);
                // 记录协议请求已排队，不宣称 compositor 已实际移动窗口。
                tracing::debug!(
                    // 记录提交请求的窗口身份。
                    window_id = self.window_id.raw(),
                    // 记录授权绑定的 surface 协议身份。
                    surface_id,
                    // 说明请求已交付给 Wayland 代理队列。
                    "Wayland interactive move request submitted" // 结束提交诊断参数。
                );
                // 结束授权成功分支。
            }
            // 缺失、过期或身份不匹配都是正常的输入生命周期结果。
            PointerActivationOutcome::Ignored(reason) => {
                // 使用结构化原因支持定向回归与现场排查。
                tracing::debug!(
                    // 记录动作所属窗口。
                    window_id = self.window_id.raw(),
                    // 记录当前窗口 surface 身份。
                    surface_id,
                    // 记录稳定的私有拒绝原因枚举。
                    ?reason,
                    // 说明本次请求被安全忽略。
                    "Wayland interactive move ignored" // 结束拒绝诊断参数。
                );
                // 结束安全忽略分支。
            } // 结束授权消费结果分派。
        }
        // 协议请求已排队或正常竞态已安全降级。
        Ok(())
        // 结束 Wayland 交互移动实现。
    }

    fn os_set_title(&mut self, title: &str) -> Result<()> {
        let toplevel = self
            .toplevel
            .as_ref()
            .ok_or_else(|| Self::missing_proxy("os_set_title", "xdg_toplevel"))?;
        toplevel.set_title(title.to_string());
        Ok(())
    }

    // 在服务端标题栏与 UIX 客户端标题栏之间切换。
    fn os_set_system_title_bar_visible(&mut self, visible: bool) -> Result<()> {
        // 优先通过 xdg-decoration 协议提交明确的模式请求。
        if let Some(decoration) = self.xdg_decoration.as_ref() {
            // 将公共布尔契约映射为唯一协议模式。
            let mode = Self::title_bar_decoration_mode(visible);
            // 模式请求由后续 surface commit 与 compositor configure 完成协商。
            decoration.set_mode(mode);
            // 记录请求方向，便于 Linux 真窗验收定位 compositor 行为。
            tracing::info!(visible, "[Wayland] xdg-decoration mode requested");
            // 已成功把请求交付给 Wayland 协议对象。
            return Ok(());
        }
        // 缺少协议时 Wayland 默认由客户端负责装饰，因此隐藏系统标题栏可直接满足。
        if !visible {
            // 记录无扩展协议时采用的客户端装饰语义。
            tracing::info!("[Wayland] using client-side decorations without xdg-decoration");
            // UIX 可以继续显示自己的标题栏。
            return Ok(());
        }
        // 无装饰协议时无法保证 compositor 提供系统标题栏。
        Err(Error::new(
            // 使用未实现分类保留能力缺失语义。
            Errc::NotImplemented,
            // 给调用方稳定说明缺失的 Wayland 扩展。
            "xdg-decoration is unavailable for server-side title bar",
        ))
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
            // request owner 损坏必须向同步调用方返回 typed failure。
            let mut active = self.frame_request.lock().map_err(|_| {
                // 构造稳定的 request 登记阶段错误。
                Error::new(
                    // active request owner 已无法安全访问。
                    Errc::InvalidState,
                    // 保留原生 frame 请求登记阶段。
                    "Wayland frame request mutex poisoned during registration",
                )
            })?;
            if active.as_ref() == Some(&request) {
                return Ok(true);
            }
            *active = Some(request);
        }

        let callback = surface.frame();
        let active = Arc::clone(&self.frame_request);
        let events = Arc::clone(&self.events);
        // callback 复用窗口所属 backend 的同一 owner-thread failure source。
        let frame_failures = self.pending_failures.clone();
        let window_id = self.window_id;
        callback.quick_assign(move |_, event, _| {
            if !matches!(event, wl_callback::Event::Done { .. }) {
                return;
            }
            // 私有 Component 检查式消费 request 并投递逐窗事件。
            if let Err(error) = deliver_frame_opportunity(
                // 传入单窗口唯一 active request owner。
                &active,
                // 传入 callback 与 App owner 之间的唯一事件队列。
                &events,
                // 传入本底层 callback 对应的精确 token。
                request,
                // 绑定本次 frame opportunity 的窗口身份。
                window_id,
                // 在 callback 到达点记录单调时间。
                std::time::Instant::now(),
            ) {
                // callback 只入队 typed failure，不执行恢复、报告或用户代码。
                let _ = frame_failures.enqueue(error);
            }
        });
        Ok(true)
    }

    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
        // request owner 损坏必须向同步调用方返回 typed failure。
        let mut active = self.frame_request.lock().map_err(|_| {
            // 构造稳定的 request 取消阶段错误。
            Error::new(
                // active request owner 已无法安全访问。
                Errc::InvalidState,
                // 保留原生 frame 请求取消阶段。
                "Wayland frame request mutex poisoned during cancellation",
            )
        })?;
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
