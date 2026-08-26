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
    // SHM 用于平台客户端阴影纹理。
    wl_shm,
    // surface 继续标识当前原生窗口表面。
    wl_surface,
    // 结束 Wayland 核心协议类型导入。
};
use wayland_client::{Connection, EventQueue, globals::GlobalList};
use wayland_protocols::xdg::activation::v1::client::{
    // pending token handle 由逐窗 activation Component 持有直到 Done 或关闭。
    xdg_activation_token_v1::XdgActivationTokenV1,
    // activation global 继续由 backend 注入逐窗 owner。
    xdg_activation_v1::XdgActivationV1,
    // 结束 activation 协议类型导入。
};
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
use crate::native::windowing::shared::window_mode::{
    NativeMaximizeTransition, NativeWindowModeState,
};
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;
use crate::platform::windowing::event::{FrameRequestToken, PointerActivationId, UiEvent};
// 引入所有后端均须显式实现的共享窗口操作端口。
use crate::native::windowing::shared::window::WindowOps;
// 引入跨平台共享的窗口状态。
use crate::native::windowing::shared::WindowState;
use crate::platform::windowing::window::{
    NativeFrameRequest, NativeFrameRequestPhase, WindowOcclusionState,
};
// 引入平台中立的窗口缩放方向供 Wayland adapter 转交。
use crate::platform::windowing::{WindowCapabilities, WindowCapability, WindowResizeEdge};

use super::compat::WaylandDispatchState;
// 引入私有 frame callback Component，保持 WindowOps 只编排协议生命周期。
use super::frame_callback::{FrameCallbackOwner, deliver_frame_opportunity};
// 文件拖放 Component 保存逐窗能力与在途协议 owner。
use super::file_drop::WaylandFileDropState;
// 引入 Wayland 私有授权注册表及其非致命消费结果。
use super::pointer_activation::WaylandPointerActivationRegistry;
// resize 约束 Component 独占用户约束、有效尺寸转换与协议请求编码。
use super::resize_constraints::{
    WaylandResizeConstraintAdapter, WaylandResizeConstraintRequest, WaylandResizeConstraintState,
};
// 引入跨注册表原子登记与注销 Component，窗口 owner 只编排协议生命周期。
use super::surface_registration::register_window_surface;
// 可选平台阴影协议由独立逐窗 Component 托管。
use super::shadow::WaylandClientShadow;
// Wayland HiDPI Component 独占 output 订阅、buffer scale 与共享 surface metrics。
use super::surface_scale::{
    WaylandOutputScaleRegistry, WaylandWindowScaleState, bind_surface_scale_events,
};

const WAYLAND_WINDOW_CAPABILITIES: WindowCapabilities = WindowCapabilities::from_slice(&[
    WindowCapability::RequestClose,
    WindowCapability::BeginMoveDrag,
    WindowCapability::BeginResizeDrag,
    WindowCapability::ResizeNotify,
    WindowCapability::SetMinimumSize,
    WindowCapability::SetMaximumSize,
    WindowCapability::SetResizable,
    WindowCapability::Maximize,
    WindowCapability::Minimize,
    WindowCapability::Restore,
    WindowCapability::HideSystemTitleBar,
    WindowCapability::SetFullscreen,
    WindowCapability::RequestNativeFrame,
    WindowCapability::NativeFramePresented,
    WindowCapability::CancelNativeFrame,
    WindowCapability::NativeSurface,
]);

/// Wayland 平台窗口操作句柄。
///
/// 持有 Wayland 协议窗口对象（surface/toplevel/xdg_surface），
/// 所有 `os_*` 方法通过 Wayland 协议操作窗口。
pub(crate) struct WaylandWindowOps {
    pub(crate) window_id: WindowId,
    pub(super) native_surface: WaylandSurfaceHandle,
    pub(crate) surface: Option<Main<wl_surface::WlSurface>>,
    pub(super) surface_id: Option<u32>,
    pub(crate) xdg_surface: Option<Main<xdg_surface::XdgSurface>>,
    pub(crate) toplevel: Option<Main<xdg_toplevel::XdgToplevel>>,
    pub(crate) compositor: Main<wl_compositor::WlCompositor>,
    // 后端唯一 SHM global 的逐窗引用，仅用于创建客户端阴影 buffer。
    pub(crate) shm: Main<wl_shm::WlShm>,
    pub(crate) input_region: Option<Main<wl_region::WlRegion>>,
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,
    // 所有窗口 callback 复用所属 Wayland backend 的同一 failure source。
    pub(super) pending_failures: PendingFailureSource,
    pub(super) surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
    // seat 代理只用于提交已经通过注册表校验的交互移动或缩放请求。
    pub(super) seat: Option<Main<wl_seat::WlSeat>>,
    // 共享注册表是 raw pointer press serial 的唯一所有者。
    pub(super) pointer_activations: Arc<Mutex<WaylandPointerActivationRegistry>>,
    // backend 级共享 Component 允许窗口同步切换自身接收资格。
    pub(super) file_drop_state: Arc<Mutex<WaylandFileDropState>>,
    // backend output registry 只通过弱引用订阅当前 surface。
    output_scales: Arc<WaylandOutputScaleRegistry>,
    // 当前窗口唯一的 output 集合、有效 scale 与 metrics owner。
    surface_scale: Arc<WaylandWindowScaleState>,
    // 实际 data-device owner 缺失时不得伪造逐窗启用成功。
    file_drop_available: bool,
    // 单一 Component 同时拥有 active request 与在途 wl_callback handle。
    pub(super) frame_callback: FrameCallbackOwner,
    configured_modes: Arc<Mutex<NativeWindowModeState>>,
    // 单窗口 resize 约束状态由同步入口与 compositor configure 共享。
    resize_constraints: Rc<RefCell<WaylandResizeConstraintState>>,
    /// xdg-decoration 装饰对象（需维持生命周期以避免装饰被撤销）
    pub(crate) xdg_decoration: Option<Main<ZxdgToplevelDecorationV1>>,
    /// xdg_activation 协议，用于请求窗口激活（raise）
    pub(crate) xdg_activation: Option<Main<XdgActivationV1>>,
    // 单个 pending activation token 允许新请求与窗口关闭注销旧 callback。
    pub(crate) activation_token: Option<Main<XdgActivationTokenV1>>,
    // 可选客户端阴影 owner；上层始终只操作统一标题栏外观契约。
    client_shadow: Option<WaylandClientShadow>,
}

impl WaylandWindowOps {
    pub(crate) fn new(
        window_id: WindowId,
        width: i32,
        height: i32,
        compositor: Main<wl_compositor::WlCompositor>,
        shm: Main<wl_shm::WlShm>,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        // 注入所属 Wayland backend 已有的 callback failure source。
        pending_failures: PendingFailureSource,
        surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
        // 注入后端已绑定的 seat 代理引用。
        seat: Option<Main<wl_seat::WlSeat>>,
        // 注入后端唯一的指针激活注册表。
        pointer_activations: Arc<Mutex<WaylandPointerActivationRegistry>>,
        // 注入 backend 唯一的文件拖放状态 owner。
        file_drop_state: Arc<Mutex<WaylandFileDropState>>,
        // 注入 backend 唯一 output scale registry。
        output_scales: Arc<WaylandOutputScaleRegistry>,
        // 注入当前窗口唯一 surface scale owner。
        surface_scale: Arc<WaylandWindowScaleState>,
        // 注入 seat 绑定后实际建立的 data-device 能力事实。
        file_drop_available: bool,
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
            // 保存同一 source 的廉价 clone，不建立新的 failure owner。
            pending_failures,
            surface_windows,
            // 保存只用于协议提交的 seat 代理引用。
            seat,
            // 保存授权注册表共享句柄而不复制任何 raw serial。
            pointer_activations,
            // 保存共享文件拖放 Component。
            file_drop_state,
            // 保存 backend output scale registry 的共享句柄。
            output_scales,
            // 保存当前窗口唯一 surface scale owner。
            surface_scale,
            // 保存不可变 data-device 能力事实。
            file_drop_available,
            // 新窗口尚未登记原生 frame request 或协议 callback。
            frame_callback: FrameCallbackOwner::new(),
            configured_modes: Arc::new(Mutex::new(NativeWindowModeState::default())),
            // 只有正的初始 logical 客户区尺寸能成为后续锁定依据。
            resize_constraints: Rc::new(RefCell::new(WaylandResizeConstraintState::new(
                width, height,
            ))),
            xdg_decoration: None,
            xdg_activation,
            // 新窗口尚未建立异步 activation token 请求。
            activation_token: None,
            client_shadow: None,
        }
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
        use crate::platform::windowing::event::{UiEventPayload, UiEventType};
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
        // configure 与同步 WindowOps 共享单窗口 resize 约束权威。
        let resize_constraints = Rc::clone(&self.resize_constraints);
        // configure 回调先发布 logical extent，再排队同一 resize 事实。
        let toplevel_surface_scale = Arc::clone(&self.surface_scale);
        // toplevel callback 复用窗口所属 backend 的同一 failure source。
        let toplevel_failures = self.pending_failures.clone();
        tl.quick_assign(move |toplevel, event, _| match event {
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
                // resize 约束 owner 必须在任何窗口事实提交前取得唯一写权限。
                let Ok(mut constraints) = resize_constraints.try_borrow_mut() else {
                    let _ = toplevel_failures.enqueue(Error::new(
                        Errc::InvalidState,
                        "Wayland xdg_toplevel Configure resize constraints already borrowed",
                    ));
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
                // 正尺寸 configure 同时规划有效尺寸与锁定约束同步。
                let resize_transition = constraints.plan_effective_extent(w, h);
                if let Some(resize_transition) = resize_transition {
                    // callback 参数自身就是仍存活的 xdg_toplevel proxy。
                    let adapter = WaylandResizeConstraintAdapter::from_proxy(&toplevel);
                    if let Err(error) = constraints
                        .commit_after(resize_transition, |request| adapter.apply(request))
                    {
                        let _ = toplevel_failures.enqueue(error);
                        return;
                    }
                }
                // 全部 owner 均可用后才提交模式转换。
                let transition = modes.apply_configure(is_max, is_full);
                // 读取刚提交的同源模式快照。
                let (maximized, fullscreen) = modes.snapshot();
                // 同一 surface appearance owner 决定贴边窗口是否关闭圆角。
                if let Err(error) = toplevel_surface_scale
                    .metrics()
                    .set_maximized(maximized || fullscreen)
                {
                    let _ = toplevel_failures.enqueue(error);
                    return;
                }
                // 同步提交窗口最大化事实。
                state.maximized = maximized;
                // 同步提交窗口全屏事实。
                state.fullscreen = fullscreen;
                // 有效客户区尺寸继续生成 resize 事实。
                if w > 0 && h > 0 {
                    // 初始或重复 configure 可能只确认已提交尺寸，不能重复重建同代 swapchain。
                    let extent_changed = state.width != w || state.height != h;
                    // compositor 已确认的 logical 尺寸立即同步共享窗口事实。
                    state.width = w;
                    state.height = h;
                    // presentation 消费事件时可读取同代 drawable 与 DPR。
                    toplevel_surface_scale.set_logical_extent(w, h);
                    // 只有真实逻辑尺寸变化才排队 resize；scale 变化由独立 owner 自行投递。
                    if extent_changed {
                        queued.push_back(UiEvent::resize(w, h).for_window(window_id));
                    }
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
        // 平台扩展存在时准备逐窗阴影资源；缺失时保持标准 Wayland 客户端装饰。
        self.client_shadow = WaylandClientShadow::try_create(
            globals,
            &queue_handle,
            self.compositor.context(),
            &self.shm,
            &surface,
            self.window_id,
        );
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
        // 初次提交前设置 fallback output 对应的正整数 buffer scale。
        surface.set_buffer_scale(self.surface_scale.current_scale());
        // 同一 surface callback 跟踪后续 output Enter/Leave 与动态 scale。
        bind_surface_scale_events(
            // callback 绑定到尚未首次提交的目标 surface。
            &surface,
            // 共享 backend 唯一 output scale registry。
            Arc::clone(&self.output_scales),
            // 共享当前窗口唯一 scale owner。
            Arc::clone(&self.surface_scale),
        );
        // surface identity 与协议 owner 均已准备后再提交。
        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surf);
        self.toplevel = Some(tl);
        self.native_surface = WaylandSurfaceHandle::new(
            // descriptor 保存当前 Wayland display 连接。
            display.backend().display_ptr().cast(),
            // descriptor 保存当前窗口 wl_surface 指针。
            self.surface_c_ptr(),
            // graphics 与 CPU presenter 共用同一逐窗 metrics owner。
            self.surface_scale.metrics(),
        );

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
    fn capabilities(&self) -> WindowCapabilities {
        let mut capabilities = WAYLAND_WINDOW_CAPABILITIES;
        if self.xdg_activation.is_some() {
            capabilities = capabilities.with(WindowCapability::Raise);
        }
        if self.xdg_decoration.is_some() {
            capabilities = capabilities.with(WindowCapability::ShowSystemTitleBar);
        }
        if self.file_drop_available {
            capabilities = capabilities
                .with(WindowCapability::EnableFileDrop)
                .with(WindowCapability::DisableFileDrop);
        }
        capabilities
    }

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
        // frame callback Component 先检查式消费 request 与协议 callback owner。
        self.frame_callback.close_checked()?;
        // 注册表失败同步传播，并保留 surface identity 与协议对象供显式重试。
        self.unregister_surface()?;
        // surface 注销后撤销本窗口的拖放接受资格与在途传输。
        super::file_drop_window::disable_window(&self.file_drop_state, self.window_id)?;
        // 装饰对象依赖 xdg_toplevel，必须先于顶层窗口释放。
        self.xdg_decoration = None;
        // 阴影依赖 wl_surface，必须在 surface owner 释放前结束生命周期。
        if let Some(shadow) = self.client_shadow.as_mut() {
            shadow.set_enabled(false);
        }
        self.client_shadow = None;
        // 先注销 xdg-shell callbacks，再释放两个逐窗协议 handles。
        self.shutdown_window_callbacks();
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
        // 由窄 Component 统一消费授权并提交移动请求。
        super::window_interaction::begin_move_drag(self, pointer_activation)
    }

    // 将当前 PointerDown 的一次性授权提交给 xdg_toplevel 交互缩放协议。
    fn os_begin_resize_drag(
        // 窗口操作只在本次同步调用期间借用自身状态。
        &mut self,
        // 保留 UI 热区声明的精确调整大小方向。
        edge: WindowResizeEdge,
        // app 仅转交不可解释身份，raw serial 始终留在 Wayland 注册表。
        pointer_activation: Option<PointerActivationId>,
        // 正常竞态统一安全忽略，协议连接失败仍由既有 pending failure 报告。
    ) -> Result<()> {
        // 由窄 Component 统一消费授权并提交带方向的缩放请求。
        super::window_interaction::begin_resize_drag(self, edge, pointer_activation)
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
            // 协议能力确认后再提交共享外观事实，避免失败请求留下部分状态。
            self.surface_scale
                .metrics()
                .set_client_decorated(!visible)?;
            // 将公共布尔契约映射为唯一协议模式。
            let mode = Self::title_bar_decoration_mode(visible);
            // 模式请求由后续 surface commit 与 compositor configure 完成协商。
            decoration.set_mode(mode);
            // 阴影启停与同一个客户端装饰事实同步，不建立第二套上层开关。
            if let Some(shadow) = self.client_shadow.as_mut() {
                shadow.set_enabled(!visible);
            }
            // 记录请求方向，便于 Linux 真窗验收定位 compositor 行为。
            tracing::info!(visible, "[Wayland] xdg-decoration mode requested");
            // 已成功把请求交付给 Wayland 协议对象。
            return Ok(());
        }
        // 缺少协议时 Wayland 默认由客户端负责装饰，因此隐藏系统标题栏可直接满足。
        if !visible {
            // 无协议时 ClientSide 是 Wayland 默认值，仍提交同一共享外观事实。
            self.surface_scale.metrics().set_client_decorated(true)?;
            if let Some(shadow) = self.client_shadow.as_mut() {
                shadow.set_enabled(true);
            }
            // 记录无扩展协议时采用的客户端装饰语义。
            tracing::info!("[Wayland] using client-side decorations without xdg-decoration");
            // UIX 可以继续显示自己的标题栏。
            return Ok(());
        }
        // 无装饰协议时无法保证 compositor 提供系统标题栏。
        Err(WindowCapability::ShowSystemTitleBar.unsupported_error())
    }

    fn os_center_on_screen(&mut self) -> Result<()> {
        Err(WindowCapability::CenterOnScreen.unsupported_error())
    }

    fn os_raise(&mut self) -> Result<()> {
        if self.xdg_activation.is_none() {
            return Err(WindowCapability::Raise.unsupported_error());
        }
        // 同步入口只建立异步请求，不伪造 compositor 已授予焦点。
        self.request_activation()
    }

    fn os_lower(&mut self) -> Result<()> {
        Err(WindowCapability::Lower.unsupported_error())
    }

    fn os_set_icon(&mut self, _path: &str) -> Result<()> {
        Err(WindowCapability::SetWindowIcon.unsupported_error())
    }

    fn os_flash(&mut self) -> Result<()> {
        Err(WindowCapability::FlashWindow.unsupported_error())
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
        // 共享层已验证正尺寸；这里仍禁止无效值污染私有有效尺寸权威。
        let mut constraints = self.resize_constraints.try_borrow_mut().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "os_set_size: Wayland resize constraints already borrowed",
            )
        })?;
        let transition = constraints.plan_effective_extent(w, h).ok_or_else(|| {
            Error::invalid_arg(format!(
                "os_set_size requires a positive extent, got {w}x{h}"
            ))
        })?;
        // 只有锁定尺寸实际变化时 set_size 才额外需要 toplevel proxy。
        let adapter = match transition.request() {
            WaylandResizeConstraintRequest::Unchanged => None,
            _ => Some(WaylandResizeConstraintAdapter::require(
                self.toplevel.as_ref(),
                "os_set_size",
            )?),
        };
        let region = self.compositor.create_region();
        // 所有 fallible preflight 完成后才提交协议输出与私有状态。
        constraints.commit_after(transition, |request| {
            xdg_surface.set_window_geometry(0, 0, w, h);
            region.add(0, 0, w, h);
            surface.set_input_region(Some(&region));
            if let Some(adapter) = adapter.as_ref() {
                adapter.apply(request)?;
            }
            Ok(())
        })?;
        drop(constraints);
        // 新 region 只在整个协议事务成功后替换旧 owner。
        self.input_region = Some(region);
        // Wayland 没有服务端 set-size 请求；客户端必须先更新布局与 buffer，再随 present commit 新尺寸。
        self.surface_scale.publish_programmatic_resize(w, h)?;
        Ok(())
    }

    fn os_set_min_size(&mut self, w: i32, h: i32) -> Result<()> {
        let adapter =
            WaylandResizeConstraintAdapter::require(self.toplevel.as_ref(), "os_set_min_size")?;
        let mut constraints = self.resize_constraints.try_borrow_mut().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "os_set_min_size: Wayland resize constraints already borrowed",
            )
        })?;
        let transition = constraints.plan_set_user_minimum(w, h);
        constraints.commit_after(transition, |request| adapter.apply(request))
    }

    fn os_set_max_size(&mut self, w: i32, h: i32) -> Result<()> {
        let adapter =
            WaylandResizeConstraintAdapter::require(self.toplevel.as_ref(), "os_set_max_size")?;
        let mut constraints = self.resize_constraints.try_borrow_mut().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "os_set_max_size: Wayland resize constraints already borrowed",
            )
        })?;
        let transition = constraints.plan_set_user_maximum(w, h);
        constraints.commit_after(transition, |request| adapter.apply(request))
    }

    fn os_set_position(&mut self, _x: i32, _y: i32) -> Result<()> {
        Err(WindowCapability::SetPosition.unsupported_error())
    }

    /// 窗口尺寸变化通知。更新 xdg_surface 窗口几何和输入区域，
    /// 确保 compositor（如 niri）的布局与窗口实际尺寸一致。
    fn os_resize_notify(&mut self, w: i32, h: i32) -> Result<()> {
        // 先更新窗口几何与输入区域。
        self.os_set_size(w, h)?;
        // 协议与 resize 约束状态提交后才发布最新 logical extent。
        self.surface_scale.set_logical_extent(w, h);
        // 再把当前有效 scale 保持在下一次 surface commit 上。
        self.surface
            // 活动窗口必须仍持有 wl_surface。
            .as_ref()
            // 缺失 surface 返回稳定生命周期错误。
            .ok_or_else(|| Self::missing_proxy("os_resize_notify", "wl_surface"))?
            // Wayland core buffer scale 必须是正整数。
            .set_buffer_scale(self.surface_scale.current_scale());
        // logical resize 通知成功。
        Ok(())
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
        // Component 完成 request 去重与旧 callback owner 替换。
        if !self.frame_callback.prepare_request(request)? {
            // 相同 request 已有在途 callback，保持既有能力成功形状。
            return Ok(true);
        }

        let callback = surface.frame();
        // callback 闭包共享 Component 内唯一 active request owner。
        let active = self.frame_callback.active_source();
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
        // callback 接线完成后由同一 Component 接管协议 handle。
        self.frame_callback.attach_callback(callback);
        Ok(true)
    }

    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
        // Component 只消费匹配 token 的 request 与 callback owner。
        self.frame_callback.cancel_checked(token)
    }

    fn os_native_frame_presented(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }

    // ── 窗口状态 ──────────────────────────────────────────

    fn os_set_resizable(&mut self, resizable: bool) -> Result<()> {
        // 即使本次为幂等调用，也必须先确认窗口仍持有活动 toplevel owner。
        let adapter =
            WaylandResizeConstraintAdapter::require(self.toplevel.as_ref(), "os_set_resizable")?;
        let mut constraints = self.resize_constraints.try_borrow_mut().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "os_set_resizable: Wayland resize constraints already borrowed",
            )
        })?;
        let transition = constraints.plan_set_resizable("os_set_resizable", resizable)?;
        constraints.commit_after(transition, |request| adapter.apply(request))
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
        Err(WindowCapability::SetBorderless.unsupported_error())
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
        Err(WindowCapability::SetAlwaysOnTop.unsupported_error())
    }

    fn os_set_opacity(&mut self, _opacity: f32) -> Result<()> {
        Err(WindowCapability::SetWindowOpacity.unsupported_error())
    }

    // ── 特性开关 ──────────────────────────────────────────

    fn os_start_text_input(&mut self) -> Result<()> {
        Err(WindowCapability::StartTextInput.unsupported_error())
    }

    fn os_stop_text_input(&mut self) -> Result<()> {
        Err(WindowCapability::StopTextInput.unsupported_error())
    }

    fn os_enable_file_drop(&mut self, enable: bool) -> Result<()> {
        if !self.file_drop_available {
            let capability = if enable {
                WindowCapability::EnableFileDrop
            } else {
                WindowCapability::DisableFileDrop
            };
            return Err(capability.unsupported_error());
        }
        // Adapter 同时校验 surface、协议能力与共享状态健康度。
        super::file_drop_window::set_window_capability(
            // 传入 backend 唯一拖放 Component。
            &self.file_drop_state,
            // 更新当前稳定窗口身份。
            self.window_id,
            // 只有活动 surface 可以发布接收资格。
            self.surface_id.is_some(),
            // 使用构造期协议能力事实。
            self.file_drop_available,
            // 传递调用方期望开关。
            enable,
        )
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.surface_c_ptr()
    }

    fn client_logical_extent(&self, fallback_width: i32, fallback_height: i32) -> (i32, i32) {
        (fallback_width, fallback_height)
    }

    fn os_show_system_menu(&mut self) -> Result<()> {
        Err(WindowCapability::ShowSystemMenu.unsupported_error())
    }

    fn os_occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }
}
