// ============================================================================
// platform/linux/wayland/mod.rs — Wayland 后端核心
//
// 本文件：子模块声明、结构体定义、构造方法。
// 各子 trait 的实现在对应的子模块中。
// ============================================================================

// ── 子模块 ──────────────────────────────────────────────────────
pub(crate) mod clipboard;
pub(crate) mod compat;
pub(crate) mod cursor;
// 光标状态 Component 独占期望形状、可见性与 Enter serial。
pub(crate) mod cursor_state;
pub(crate) mod display;
pub(crate) mod event_loop;
// 文件拖放 Component 独占 data-offer、逐窗开关与 URI read 生命周期。
pub(crate) mod file_drop;
// URI 解析 Component 与协议/FD owner 解耦，可执行纯单元测试。
pub(crate) mod file_drop_uri;
// 窗口 Adapter 校验协议能力与逐窗生命周期。
pub(crate) mod file_drop_window;
// 纯逐窗注册表独占启用集合并提供可执行状态单测。
pub(crate) mod file_drop_window_registry;
// 原生 frame callback Component 独占 one-shot 请求消费与事件投递。
pub(crate) mod frame_callback;
// 输入代理生命周期模块把完整 capability 快照收敛为幂等边沿。
pub(crate) mod input_proxy_lifecycle;
// 输入代理 owner Component 检查式读取与提交 pointer/keyboard 槽。
pub(crate) mod input_proxy_owner;
// 键盘焦点 Component 事务化提交 Enter/Leave 与输入状态清理。
pub(crate) mod keyboard_focus_owner;
// 键盘按键 Component 事务化提交事件、serial 与重复状态。
pub(crate) mod keyboard_key_owner;
// 键盘修饰 Component 事务化提交 KeyMod 与重复状态失效。
pub(crate) mod keyboard_modifier_owner;
// 键盘重复配置 Component 事务化提交 compositor rate/delay。
pub(crate) mod keyboard_repeat_config_owner;
pub(crate) mod keycode;
pub(crate) mod output;
// 指针激活注册表独占 Wayland 拖动授权的签发与生命周期。
pub(crate) mod pointer_activation;
// 指针轴帧 Component 把 Wayland 连续/离散协议值归一为平台中立滚轮步长。
pub(crate) mod pointer_axis;
// 指针按钮 Component 事务化提交授权、serial 与 PointerDown/Up。
pub(crate) mod pointer_button_owner;
// 指针焦点 Component 事务化提交 Enter、Motion 与 Leave 的共享 owners。
pub(crate) mod pointer_focus_owner;
pub(crate) mod presenter;
// resize 约束 Component 独占用户约束、有效尺寸状态机与 xdg_toplevel Adapter。
pub(crate) mod resize_constraints;
pub(crate) mod seat;
// 客户端窗口阴影 Component 只在 compositor 提供对应平台协议时启用。
pub(crate) mod shadow;
pub(crate) mod shm_buffer;
// surface registration Component 原子注销窗口的两份共享注册事实。
pub(crate) mod surface_registration;
// output scale registry 与逐窗 surface 状态共同托管 Wayland HiDPI。
pub(crate) mod surface_scale;
pub(crate) mod text_input;
pub(crate) mod window;
// 逐窗 activation Component 独占异步 token 请求与回调生命周期。
pub(crate) mod window_activation;
// 逐窗 callback shutdown Component 独占 xdg-shell 回调注销顺序。
pub(crate) mod window_callback_shutdown;
// 逐窗交互 Component 独占移动/缩放授权消费与 xdg_toplevel 请求映射。
pub(crate) mod window_interaction;
pub(crate) mod window_ops;
// 逐窗 surface helper 隔离 descriptor、装饰映射与注册注销编排。
pub(crate) mod window_surface;
// wake-pipe Module 隔离 owner-thread 的非阻塞排空操作。
pub(crate) mod wake_pipe;

// ── 依赖 ────────────────────────────────────────────────────────
use self::compat::{Main, ProxyContext, WaylandDispatchState};
// 引入 Wayland 私有的一次性指针激活注册表。
use self::pointer_activation::WaylandPointerActivationRegistry;
// 引入 backend 唯一 output scale registry。
use self::surface_scale::WaylandOutputScaleRegistry;
// 引入稳定错误分类，使 wl_output callback 能传播显示状态 owner 损坏。
use crate::core::{Errc, Error, Point, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::shared::ime_events::ImeCompositionState;
use crate::native::windowing::shared::input_serial::InputSerial;
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;
use crate::platform::windowing::event::*;
use crate::platform::windowing::{KeyCode, KeyMod};
use std::collections::{HashSet, VecDeque};
use std::os::unix::io::RawFd;

use std::sync::{Arc, Mutex};

use wayland_client::{
    Connection, EventQueue,
    globals::{GlobalList, registry_queue_init},
    protocol::{
        wl_compositor, wl_data_device_manager, wl_data_source, wl_keyboard, wl_output, wl_pointer,
        wl_seat, wl_shm,
    },
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
};
// 可选 cursor-shape global 为 Wayland 指针提供枚举形状协议。
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::xdg::shell::client::xdg_wm_base;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

// ════════════════════════════════════════════════════════════════════════════
// ShmBuffer — RAII 包装：SHM 池 + 缓冲区 + 后备文件
// ════════════════════════════════════════════════════════════════════════════
// LastPointerState — 追踪鼠标位置（用于 Button 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
pub(crate) struct LastPointerState {
    pub(crate) position: Point,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HeldKeyInfo {
    pub(crate) code: KeyCode,
    pub(crate) mods: KeyMod,
    pub(crate) first_press: std::time::Instant,
    pub(crate) window_id: WindowId,
}

// ════════════════════════════════════════════════════════════════════════════
// WaylandBackend — Wayland 后端主结构体
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct WaylandBackend {
    // ── Wayland 连接 ──────────────────────────────────────────────
    pub(crate) display: Connection,
    pub(crate) event_queue: EventQueue<WaylandDispatchState>,
    pub(crate) dispatch_state: WaylandDispatchState,
    #[allow(dead_code)]
    pub(crate) _globals: GlobalList,

    // ── 协议全局对象 ──────────────────────────────────────────────
    pub(crate) _compositor: Main<wl_compositor::WlCompositor>,
    pub(crate) _wm_base: Main<xdg_wm_base::XdgWmBase>,
    pub(crate) _shm: Main<wl_shm::WlShm>,
    // layer-shell 是可选桌面协议；普通应用不因其缺失而无法启动。
    pub(crate) layer_shell: Option<Main<ZwlrLayerShellV1>>,

    // ── 窗口状态 ──────────────────────────────────────────────────
    pub(crate) closed: bool,
    pub(crate) pending_failures: PendingFailureSource,

    // ── 事件队列（线程安全，供 quick_assign 回调写入）───────────
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,

    // ── Seat 及输入代理 ───────────────────────────────────────────
    pub(crate) seat: Option<Main<wl_seat::WlSeat>>,
    pub(crate) pointer: Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    pub(crate) keyboard: Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
    // 可选协议 global 只由 Wayland cursor Adapter 消费。
    pub(crate) cursor_shape_manager: Option<Main<WpCursorShapeManagerV1>>,
    // 唯一 cursor intent Component 供同步端口与 pointer callback 共享。
    pub(crate) cursor_state: Arc<cursor_state::WaylandCursorState>,
    pub(crate) last_pointer: Arc<Mutex<LastPointerState>>,
    pub(crate) keys_down: Arc<Mutex<HashSet<KeyCode>>>,
    pub(crate) surface_windows: Arc<Mutex<SurfaceWindowTargets>>,
    // 单一注册表关联原生 PointerDown、surface 代次与协议 serial。
    pub(crate) pointer_activations: Arc<Mutex<WaylandPointerActivationRegistry>>,

    // ── 按键重复（客户端侧实现，Wayland 协议要求）──────────
    /// 重复速率（字符/秒），0 = 禁用重复。来自 wl_keyboard.repeat_info。
    pub(crate) repeat_rate: Arc<Mutex<i32>>,
    /// 首次重复前的延迟（毫秒）。
    pub(crate) repeat_delay: Arc<Mutex<i32>>,
    /// 当前按住的键信息：(linux_keycode, KeyCode, 修饰键, 首次按下时间)。
    pub(crate) held_key_info: Arc<Mutex<Option<HeldKeyInfo>>>,
    /// 上次生成重复事件的时间。
    pub(crate) last_repeat_time: Arc<Mutex<Option<std::time::Instant>>>,

    // ── 剪贴板 ────────────────────────────────────────────────────
    pub(crate) data_device_manager: Option<Main<wl_data_device_manager::WlDataDeviceManager>>,
    pub(crate) data_device: Option<Main<wayland_client::protocol::wl_data_device::WlDataDevice>>,
    // 当前本地 selection 的 data-source callback 与协议 handle owner。
    pub(crate) clipboard_source: Option<Main<wl_data_source::WlDataSource>>,
    pub(crate) clipboard_text: Arc<Mutex<String>>,
    pub(crate) owns_clipboard: Arc<Mutex<bool>>,
    pub(crate) clipboard_read: Arc<Mutex<Option<clipboard::ClipboardRead>>>,
    pub(crate) clipboard_writes: Arc<Mutex<Vec<clipboard::ClipboardWrite>>>,
    // 独立 Component 共享逐窗文件拖放、offer 与 read owners。
    pub(crate) file_drop_state: Arc<Mutex<file_drop::WaylandFileDropState>>,
    pub(crate) last_input_serial: Arc<Mutex<InputSerial>>,
    pub(crate) wake_read_fd: RawFd,
    pub(crate) wake_write_fd: RawFd,
    pub(crate) poll_fds: Vec<libc::pollfd>,

    // ── 显示器 ────────────────────────────────────────────────────
    pub(crate) outputs: Arc<Mutex<Vec<output::RawOutput>>>,
    // output identity、动态 scale 与逐窗弱订阅的唯一 owner。
    pub(crate) output_scales: Arc<WaylandOutputScaleRegistry>,
    #[allow(dead_code)]
    pub(crate) _wl_outputs: Vec<Main<wl_output::WlOutput>>,

    // ── 文本输入（IME）──────────────────────────────────────────
    pub(crate) text_input_manager: Option<Main<ZwpTextInputManagerV3>>,
    pub(crate) text_input: Option<Main<ZwpTextInputV3>>,
    pub(crate) text_input_window_id: Option<WindowId>,
    pub(crate) active_text_input_window_id: Option<WindowId>,
    pub(crate) text_input_composition: Arc<Mutex<ImeCompositionState>>,
    pub(crate) text_input_generation: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) text_input_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,

    // ── xdg_activation（窗口提升/聚焦）────────────────────────
    pub(crate) _xdg_activation: Option<Main<XdgActivationV1>>,
    pub(crate) next_window_id: u64,
}

impl WaylandBackend {
    /// 返回事件队列句柄（供定时器子系统使用）。
    pub(crate) fn event_queue_handle(&self) -> Arc<Mutex<VecDeque<UiEvent>>> {
        self.events.clone()
    }

    pub(crate) fn enqueue_failure(&self, error: Error) {
        let _ = self.pending_failures.enqueue(error);
    }

    pub(crate) fn take_pending_failure(&mut self) -> Option<Error> {
        self.pending_failures.take()
    }

    // 确定性注销由 backend global Component 独占的协议回调与显示状态。
    pub(crate) fn shutdown_global_callbacks(&mut self) {
        // 先注销 wm-base ping callback，避免 backend 关闭后继续发送 pong。
        self._wm_base.clear_callback();
        // 再逐个注销 output callback，阻止它们继续改写共享显示状态。
        for output in &self._wl_outputs {
            // 每个兼容代理都必须先脱离注册表，再释放本地 handle。
            output.clear_callback();
        }
        // 所有回调失效后释放 backend 持有的 output handles。
        self._wl_outputs.clear();
        // teardown 位于 owner thread，允许恢复锁所有权后清空不可再观察的状态。
        self.outputs
            // 中毒只表示旧 callback 曾 panic，不改变关闭阶段的独占清理权。
            .lock()
            // 关闭路径不得因遗留 PoisonError 跳过状态释放。
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            // 移除全部过期显示快照。
            .clear();
        // 全局 callback 注销后释放 output scale 与逐窗弱订阅事实。
        self.output_scales.shutdown();
    }

    pub(crate) fn create_wake_pipe() -> Result<(RawFd, RawFd), String> {
        let mut fds = [0; 2];
        let flags = libc::O_CLOEXEC | libc::O_NONBLOCK;
        // SAFETY: fds 指向两个连续且可写的 RawFd 槽，pipe2 成功时会完整初始化二者。
        let ret = unsafe { libc::pipe2(fds.as_mut_ptr(), flags) };
        if ret == 0 {
            Ok((fds[0], fds[1]))
        } else {
            Err(format!(
                "failed to create Wayland wake pipe: {}",
                std::io::Error::last_os_error()
            ))
        }
    }

    pub(crate) fn new(pending_failures: PendingFailureSource) -> Result<Self, String> {
        let display =
            Connection::connect_to_env().map_err(|e| format!("Wayland connect failed: {e}"))?;
        let (globals, mut event_queue) = registry_queue_init::<WaylandDispatchState>(&display)
            .map_err(|e| format!("Wayland registry initialization failed: {e}"))?;
        let queue_handle = event_queue.handle();
        let proxy_context = ProxyContext::new(queue_handle.clone(), pending_failures.clone());
        let mut dispatch_state = WaylandDispatchState::from_context(&proxy_context);

        let _compositor = Main::new(
            globals
                .bind::<wl_compositor::WlCompositor, _, _>(&queue_handle, 1..=4, ())
                .map_err(|e| format!("no wl_compositor: {e}"))?,
            proxy_context.clone(),
        );
        let _wm_base = Main::new(
            globals
                .bind::<xdg_wm_base::XdgWmBase, _, _>(&queue_handle, 1..=1, ())
                .map_err(|e| format!("no xdg_wm_base (need xdg-shell): {e}"))?,
            proxy_context.clone(),
        );
        let _shm = Main::new(
            globals
                .bind::<wl_shm::WlShm, _, _>(&queue_handle, 1..=1, ())
                .map_err(|e| format!("no wl_shm: {e}"))?,
            proxy_context.clone(),
        );
        let layer_shell = globals
            .bind::<ZwlrLayerShellV1, _, _>(&queue_handle, 1..=4, ())
            .ok()
            .map(|proxy| Main::new(proxy, proxy_context.clone()));

        let outputs: Arc<Mutex<Vec<output::RawOutput>>> = Arc::new(Mutex::new(Vec::new()));
        // 建立 backend 唯一 output scale registry，复用 runtime failure source。
        let output_scales = Arc::new(WaylandOutputScaleRegistry::new(pending_failures.clone()));
        let mut wl_output_handles = Vec::new();
        for (index, global) in globals
            .contents()
            .clone_list()
            .into_iter()
            .filter(|global| global.interface == "wl_output")
            .enumerate()
        {
            // 每个 output callback 只克隆同一 backend 的 failure source。
            let output_failures = pending_failures.clone();
            let output = Main::new(
                globals.registry().bind::<wl_output::WlOutput, _, _>(
                    global.name,
                    global.version.min(2),
                    &queue_handle,
                    (),
                ),
                proxy_context.clone(),
            );
            // 保存协议 output identity，供 surface Enter/Leave 与 Scale 事件对齐。
            let output_id = output.id().protocol_id();
            // 首个枚举 output 作为没有 Enter 事实时的稳定 fallback。
            output_scales.register_output(output_id, index == 0);
            let output_list = Arc::clone(&outputs);
            // output callback 只克隆 backend 唯一 scale registry。
            let output_scale_registry = Arc::clone(&output_scales);
            output.quick_assign(move |_, event, _| {
                // 中毒显示状态不得通过 PoisonError 恢复后继续改写。
                let Ok(mut list) = output_list.lock() else {
                    // 将 callback owner 损坏转换为稳定 typed failure。
                    let _ = output_failures.enqueue(Error::new(
                        // 显示状态 owner 已无法安全访问。
                        Errc::InvalidState,
                        // 保留 wl_output callback 的精确投递阶段。
                        "Wayland wl_output callback outputs mutex poisoned",
                    ));
                    // 停止处理本次协议事件，避免写入损坏状态。
                    return;
                };
                while list.len() <= index {
                    list.push(output::RawOutput::default());
                }
                let entry = &mut list[index];
                // Scale 通知在释放显示列表锁后再广播到逐窗状态。
                let mut scale_update = None;
                // 更新同一 output 的显示查询事实。
                match event {
                    wl_output::Event::Geometry { x, y, .. } => {
                        entry.x = x;
                        entry.y = y;
                    }
                    wl_output::Event::Mode {
                        flags,
                        width,
                        height,
                        ..
                    } => {
                        if let wayland_client::WEnum::Value(flags) = flags {
                            if (flags.bits() & 0x1) != 0 {
                                entry.width = width;
                                entry.height = height;
                            }
                        }
                    }
                    wl_output::Event::Scale { factor } => {
                        // DisplayInfo 与 surface registry 消费同一个正 scale 事实。
                        entry.scale = factor.max(1);
                        // 保存锁外广播值。
                        scale_update = Some(entry.scale);
                    }
                    wl_output::Event::Done => entry.is_primary = index == 0,
                    _ => {}
                }
                // 显式释放显示列表锁，避免逐窗通知形成锁顺序耦合。
                drop(list);
                // 动态 Scale 只在实际事件到达时广播。
                if let Some(scale) = scale_update {
                    // registry 更新会逐窗排队 logical resize。
                    output_scale_registry.update_output_scale(output_id, scale);
                }
            });
            wl_output_handles.push(output);
        }

        _wm_base.quick_assign(|wm, event, _| {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm.pong(serial);
            }
        });

        let data_device_manager = globals
            .bind::<wl_data_device_manager::WlDataDeviceManager, _, _>(&queue_handle, 1..=3, ())
            .ok()
            .map(|proxy| Main::new(proxy, proxy_context.clone()));
        // cursor-shape 是可选 staging global，缺失时保持稳定 capability absence。
        let cursor_shape_manager = globals
            // 同时接受协议 v1 与新增兼容形状的 v2。
            .bind::<WpCursorShapeManagerV1, _, _>(&queue_handle, 1..=2, ())
            // bind 失败只表示当前 compositor 不提供该可选能力。
            .ok()
            // 复用 backend callback/child-object 上下文包装代理。
            .map(|proxy| Main::new(proxy, proxy_context.clone()));
        let text_input_manager = globals
            .bind::<ZwpTextInputManagerV3, _, _>(&queue_handle, 1..=1, ())
            .ok()
            .map(|proxy| Main::new(proxy, proxy_context.clone()));
        let xdg_activation = globals
            .bind::<XdgActivationV1, _, _>(&queue_handle, 1..=1, ())
            .ok()
            .map(|proxy| Main::new(proxy, proxy_context.clone()));

        event_queue
            .roundtrip(&mut dispatch_state)
            .map_err(|e| format!("Wayland roundtrip failed: {e}"))?;

        let events: Arc<Mutex<VecDeque<UiEvent>>> = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard_text: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let owns_clipboard: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let clipboard_read = Arc::new(Mutex::new(None));
        // 初始没有启用窗口、协议 offer 或在途 URI read。
        let file_drop_state = Arc::new(Mutex::new(file_drop::WaylandFileDropState::default()));
        let last_pointer: Arc<Mutex<LastPointerState>> =
            Arc::new(Mutex::new(LastPointerState::default()));
        let (wake_read_fd, wake_write_fd) = Self::create_wake_pipe()?;

        Ok(Self {
            display,
            event_queue,
            dispatch_state,
            _globals: globals,
            _compositor,
            _wm_base,
            _shm,
            layer_shell,
            closed: false,
            pending_failures,
            events,
            seat: None,
            pointer: Arc::new(Mutex::new(None)),
            keyboard: Arc::new(Mutex::new(None)),
            // 保存构造期只读发现的可选 cursor-shape global。
            cursor_shape_manager,
            // 默认 intent 为可见箭头且没有 Enter serial。
            cursor_state: Arc::new(cursor_state::WaylandCursorState::default()),
            last_pointer,
            keys_down: Arc::new(Mutex::new(HashSet::new())),
            surface_windows: Arc::new(Mutex::new(SurfaceWindowTargets::default())),
            // 初始化 Wayland 后端唯一的指针激活授权所有者。
            pointer_activations: Arc::new(Mutex::new(
                // 使用非零身份与代次空间建立空注册表。
                WaylandPointerActivationRegistry::new(),
                // 结束共享注册表构造。
            )),
            repeat_rate: Arc::new(Mutex::new(0)),
            repeat_delay: Arc::new(Mutex::new(400)),
            held_key_info: Arc::new(Mutex::new(None)),
            last_repeat_time: Arc::new(Mutex::new(None)),
            data_device_manager,
            data_device: None,
            // 初始尚未向 compositor 发布本地 clipboard source。
            clipboard_source: None,
            clipboard_text,
            owns_clipboard,
            clipboard_read,
            clipboard_writes: Arc::new(Mutex::new(Vec::new())),
            // 发布 backend 唯一的文件拖放状态 owner。
            file_drop_state,
            last_input_serial: Arc::new(Mutex::new(InputSerial::default())),
            wake_read_fd,
            wake_write_fd,
            poll_fds: Vec::with_capacity(4),
            outputs,
            // 发布 backend 唯一 output scale registry。
            output_scales,
            _wl_outputs: wl_output_handles,
            text_input_manager,
            text_input: None,
            text_input_window_id: None,
            active_text_input_window_id: None,
            text_input_composition: Arc::new(Mutex::new(ImeCompositionState::default())),
            text_input_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            text_input_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            _xdg_activation: xdg_activation,
            next_window_id: 1,
        })
    }
}

impl Drop for WaylandBackend {
    fn drop(&mut self) {
        // 先失效独立 text-input callback，避免它借用随后关闭的 seat。
        self.shutdown_text_input();
        // 文件拖放先于 data-device callback 注销释放 offer 与 pipe owners。
        self.shutdown_file_drop();
        // 先拆除 seat 回调与输入代理，打断兼容回调表的强引用环。
        self.shutdown_seat_and_input();
        // 再释放 clipboard 在途 read/write FD 与过期授权状态。
        self.shutdown_clipboard_io();
        // 最后注销 backend 全局 callbacks 并释放显示状态 owner。
        self.shutdown_global_callbacks();
        self.pending_failures.close();
        // SAFETY: 两个描述符由 create_wake_pipe 独占创建，到此尚未关闭且 Drop 只执行一次。
        let _ = unsafe { libc::close(self.wake_read_fd) };
        let _ = unsafe { libc::close(self.wake_write_fd) };
    }
}
