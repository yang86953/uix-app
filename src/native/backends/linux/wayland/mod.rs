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
pub(crate) mod display;
pub(crate) mod event_loop;
// 原生 frame callback Component 独占 one-shot 请求消费与事件投递。
pub(crate) mod frame_callback;
// 输入代理生命周期模块把完整 capability 快照收敛为幂等边沿。
pub(crate) mod input_proxy_lifecycle;
// 输入代理 owner Component 检查式读取与提交 pointer/keyboard 槽。
pub(crate) mod input_proxy_owner;
pub(crate) mod keyboard;
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
// 指针按钮 Component 事务化提交授权、serial 与 PointerDown/Up。
pub(crate) mod pointer_button_owner;
// 指针焦点 Component 事务化提交 Enter、Motion 与 Leave 的共享 owners。
pub(crate) mod pointer_focus_owner;
pub(crate) mod presenter;
pub(crate) mod seat;
pub(crate) mod shm_buffer;
// surface registration Component 原子注销窗口的两份共享注册事实。
pub(crate) mod surface_registration;
pub(crate) mod text_input;
pub(crate) mod window;
// 逐窗 activation Component 独占异步 token 请求与回调生命周期。
pub(crate) mod window_activation;
// 逐窗 callback shutdown Component 独占 xdg-shell 回调注销顺序。
pub(crate) mod window_callback_shutdown;
pub(crate) mod window_ops;

// ── 依赖 ────────────────────────────────────────────────────────
use self::compat::{Main, ProxyContext, WaylandDispatchState};
// 引入 Wayland 私有的一次性指针激活注册表。
use self::pointer_activation::WaylandPointerActivationRegistry;
// 引入稳定错误分类，使 wl_output callback 能传播显示状态 owner 损坏。
use crate::core::{Errc, Error, Point, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::event::*;
use crate::native::windowing::input::{KeyCode, KeyMod};
use crate::native::windowing::shared::ime_events::ImeCompositionState;
use crate::native::windowing::shared::input_serial::InputSerial;
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;
use std::collections::{HashSet, VecDeque};
use std::os::unix::io::RawFd;

use std::sync::{Arc, Mutex};

use wayland_client::{
    Connection, EventQueue,
    globals::{GlobalList, registry_queue_init},
    protocol::{
        wl_compositor, wl_data_device_manager, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm,
    },
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::xdg::shell::client::xdg_wm_base;

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

    // ── 窗口状态 ──────────────────────────────────────────────────
    pub(crate) closed: bool,
    pub(crate) pending_failures: PendingFailureSource,

    // ── 事件队列（线程安全，供 quick_assign 回调写入）───────────
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,

    // ── Seat 及输入代理 ───────────────────────────────────────────
    pub(crate) seat: Option<Main<wl_seat::WlSeat>>,
    pub(crate) pointer: Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    pub(crate) keyboard: Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
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
    pub(crate) clipboard_text: Arc<Mutex<String>>,
    pub(crate) owns_clipboard: Arc<Mutex<bool>>,
    pub(crate) clipboard_read: Arc<Mutex<Option<clipboard::ClipboardRead>>>,
    pub(crate) clipboard_writes: Arc<Mutex<Vec<clipboard::ClipboardWrite>>>,
    pub(crate) last_input_serial: Arc<Mutex<InputSerial>>,
    pub(crate) wake_read_fd: RawFd,
    pub(crate) wake_write_fd: RawFd,
    pub(crate) poll_fds: Vec<libc::pollfd>,

    // ── 显示器 ────────────────────────────────────────────────────
    pub(crate) outputs: Arc<Mutex<Vec<output::RawOutput>>>,
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

        let outputs: Arc<Mutex<Vec<output::RawOutput>>> = Arc::new(Mutex::new(Vec::new()));
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
            let output_list = Arc::clone(&outputs);
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
                    wl_output::Event::Scale { factor } => entry.scale = factor,
                    wl_output::Event::Done => entry.is_primary = index == 0,
                    _ => {}
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
            closed: false,
            pending_failures,
            events,
            seat: None,
            pointer: Arc::new(Mutex::new(None)),
            keyboard: Arc::new(Mutex::new(None)),
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
            clipboard_text,
            owns_clipboard,
            clipboard_read,
            clipboard_writes: Arc::new(Mutex::new(Vec::new())),
            last_input_serial: Arc::new(Mutex::new(InputSerial::default())),
            wake_read_fd,
            wake_write_fd,
            poll_fds: Vec::with_capacity(4),
            outputs,
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
