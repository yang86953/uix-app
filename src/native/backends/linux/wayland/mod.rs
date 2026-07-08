// ============================================================================
// platform/linux/wayland/mod.rs — Wayland 后端核心
//
// 本文件：子模块声明、结构体定义、构造方法。
// 各子 trait 的实现在对应的子模块中。
// ============================================================================

// ── 子模块 ──────────────────────────────────────────────────────
pub(crate) mod clipboard;
pub(crate) mod cursor;
pub(crate) mod display;
pub(crate) mod event_loop;
pub(crate) mod gpu_presenter;
pub(crate) mod keyboard;
pub(crate) mod keycode;
pub(crate) mod output;
pub(crate) mod presenter;
pub(crate) mod seat;
pub(crate) mod shm_buffer;
pub(crate) mod text_input;
pub(crate) mod window;
pub(crate) mod window_ops;

// ── 依赖 ────────────────────────────────────────────────────────
use self::shm_buffer::ShmBuffer;
use crate::core::Point;
use crate::native::traits::event::*;
use crate::native::traits::input::{KeyCode, KeyMod};
use std::collections::{HashSet, VecDeque};
use std::os::unix::io::RawFd;

use std::sync::{Arc, Mutex};

use wayland_client::{
    protocol::{
        wl_compositor, wl_data_device_manager, wl_keyboard, wl_output, wl_pointer, wl_region,
        wl_seat, wl_shm, wl_surface,
    },
    Display, EventQueue, GlobalManager, Main,
};
use wayland_protocols::staging::xdg_activation::v1::client::xdg_activation_v1::XdgActivationV1;
use wayland_protocols::unstable::text_input::v3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

// ════════════════════════════════════════════════════════════════════════════
// ShmBuffer — RAII 包装：SHM 池 + 缓冲区 + 后备文件
// ════════════════════════════════════════════════════════════════════════════
// LastPointerState — 追踪鼠标位置（用于 Button 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
pub(crate) struct LastPointerState {
    pub(crate) position: Point,
}

// ════════════════════════════════════════════════════════════════════════════
// WaylandBackend — Wayland 后端主结构体
// ════════════════════════════════════════════════════════════════════════════

pub struct WaylandBackend {
    // ── Wayland 连接 ──────────────────────────────────────────────
    pub(crate) display: Display,
    pub(crate) event_queue: EventQueue,
    #[allow(dead_code)]
    pub(crate) _globals: GlobalManager,

    // ── 协议全局对象 ──────────────────────────────────────────────
    pub(crate) _compositor: Main<wl_compositor::WlCompositor>,
    pub(crate) _wm_base: Main<xdg_wm_base::XdgWmBase>,
    pub(crate) _shm: Main<wl_shm::WlShm>,

    // ── 窗口对象 ──────────────────────────────────────────────────
    pub(crate) surface: Option<Main<wl_surface::WlSurface>>,
    pub(crate) xdg_surface: Option<Main<xdg_surface::XdgSurface>>,
    pub(crate) toplevel: Option<Main<xdg_toplevel::XdgToplevel>>,

    // ── 窗口状态 ──────────────────────────────────────────────────
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) shown: bool,
    pub(crate) closed: bool,
    pub(crate) configured: bool,
    pub(crate) maximized: Arc<Mutex<bool>>,
    pub(crate) fullscreen: Arc<Mutex<bool>>,

    // ── 事件队列（线程安全，供 quick_assign 回调写入）───────────
    pub(crate) events: Arc<Mutex<VecDeque<UiEvent>>>,

    // ── SHM 双缓冲 ────────────────────────────────────────────────
    pub(crate) shm_buffers: [Option<ShmBuffer>; 2],
    pub(crate) active_buffer: usize,

    // ── Seat 及输入代理 ───────────────────────────────────────────
    pub(crate) seat: Option<Main<wl_seat::WlSeat>>,
    pub(crate) pointer: Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    pub(crate) keyboard: Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
    pub(crate) last_pointer: Arc<Mutex<LastPointerState>>,
    pub(crate) keys_down: Arc<Mutex<HashSet<KeyCode>>>,
    pub(crate) input_region: Option<Main<wl_region::WlRegion>>,

    // ── 按键重复（客户端侧实现，Wayland 协议要求）──────────
    /// 重复速率（字符/秒），0 = 禁用重复。来自 wl_keyboard.repeat_info。
    pub(crate) repeat_rate: Arc<Mutex<i32>>,
    /// 首次重复前的延迟（毫秒）。
    pub(crate) repeat_delay: Arc<Mutex<i32>>,
    /// 当前按住的键信息：(linux_keycode, KeyCode, 修饰键, 首次按下时间)。
    pub(crate) held_key_info: Arc<Mutex<Option<(u32, KeyCode, KeyMod, std::time::Instant)>>>,
    /// 上次生成重复事件的时间。
    pub(crate) last_repeat_time: Arc<Mutex<Option<std::time::Instant>>>,

    // ── 剪贴板 ────────────────────────────────────────────────────
    pub(crate) data_device_manager: Option<Main<wl_data_device_manager::WlDataDeviceManager>>,
    pub(crate) data_device: Option<Main<wayland_client::protocol::wl_data_device::WlDataDevice>>,
    pub(crate) clipboard_text: Arc<Mutex<String>>,
    pub(crate) owns_clipboard: Arc<Mutex<bool>>,
    pub(crate) clipboard_read_fd: Arc<Mutex<Option<RawFd>>>,
    pub(crate) wake_read_fd: RawFd,
    pub(crate) wake_write_fd: RawFd,

    // ── 显示器 ────────────────────────────────────────────────────
    pub(crate) outputs: Arc<Mutex<Vec<output::RawOutput>>>,
    #[allow(dead_code)]
    pub(crate) _wl_outputs: Vec<Main<wl_output::WlOutput>>,

    // ── 文本输入（IME）──────────────────────────────────────────
    pub(crate) text_input_manager: Option<Main<ZwpTextInputManagerV3>>,
    pub(crate) text_input: Option<Main<ZwpTextInputV3>>,
    pub(crate) text_input_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,

    // ── xdg_activation（窗口提升/聚焦）────────────────────────
    pub(crate) _xdg_activation: Option<Main<XdgActivationV1>>,
    pub(crate) next_window_id: u64,
}

impl WaylandBackend {
    /// 返回事件队列句柄（供定时器子系统使用）。
    pub fn event_queue_handle(&self) -> Arc<Mutex<VecDeque<UiEvent>>> {
        self.events.clone()
    }

    pub(crate) fn create_wake_pipe() -> Result<(RawFd, RawFd), String> {
        let mut fds = [0; 2];
        let flags = libc::O_CLOEXEC | libc::O_NONBLOCK;
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

    pub fn new() -> Result<Self, String> {
        let display =
            Display::connect_to_env().map_err(|e| format!("Wayland connect failed: {}", e))?;

        let mut event_queue = display.create_event_queue();
        let attached = (*display).clone().attach(event_queue.token());

        // ── 使用 Arc<Mutex> 捕获 global_filter! 回调中绑定的单例对象 ──
        // 必须用 Arc（非 Rc）因为回调要求 'static 生命周期。
        let compositor_cell: Arc<Mutex<Option<Main<wl_compositor::WlCompositor>>>> =
            Arc::new(Mutex::new(None));

        let wm_base_cell: Arc<Mutex<Option<Main<xdg_wm_base::XdgWmBase>>>> =
            Arc::new(Mutex::new(None));

        let shm_cell: Arc<Mutex<Option<Main<wl_shm::WlShm>>>> = Arc::new(Mutex::new(None));

        // ── wl_output 多实例收集 ──────────────────────────────────
        let outputs: Arc<Mutex<Vec<output::RawOutput>>> = Arc::new(Mutex::new(Vec::new()));
        let wl_output_handles: Arc<Mutex<Vec<Main<wl_output::WlOutput>>>> =
            Arc::new(Mutex::new(Vec::new()));

        // 防止多个 wl_output 的 done 事件竞态（仅第一个标记为 primary）
        let is_first_output = Arc::new(Mutex::new(true));

        // text_input_manager 绑定
        let text_input_manager_cell: Arc<Mutex<Option<Main<ZwpTextInputManagerV3>>>> =
            Arc::new(Mutex::new(None));
        let tim_for_cb = text_input_manager_cell.clone();

        // xdg_activation 绑定
        let xdg_activation_cell: Arc<Mutex<Option<Main<XdgActivationV1>>>> =
            Arc::new(Mutex::new(None));
        let xa_for_cb = xdg_activation_cell.clone();

        // ── 手动回调绑定所有全局（避免 global_filter! 宏的高阶生命周期问题）──
        let globals = GlobalManager::new_with_cb(&attached, {
            let compositor_for_cb = compositor_cell.clone();
            let wm_base_for_cb = wm_base_cell.clone();
            let shm_for_cb = shm_cell.clone();
            let outputs_for_cb = outputs.clone();
            let wl_output_handles_for_cb = wl_output_handles.clone();
            let is_first_for_cb = is_first_output.clone();
            move |event: wayland_client::GlobalEvent,
                  registry: wayland_client::Attached<
                wayland_client::protocol::wl_registry::WlRegistry,
            >,
                  _ddata: wayland_client::DispatchData<'_>| {
                use wayland_client::GlobalEvent;
                if let GlobalEvent::New {
                    id,
                    interface,
                    version,
                } = event
                {
                    match interface.as_str() {
                        "wl_compositor" => {
                            let proxy: Main<wl_compositor::WlCompositor> =
                                registry.bind(version.min(4), id);
                            *compositor_for_cb.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(proxy);
                        }
                        "xdg_wm_base" => {
                            let proxy: Main<xdg_wm_base::XdgWmBase> =
                                registry.bind(version.min(1), id);
                            *wm_base_for_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
                        }
                        "wl_shm" => {
                            let proxy: Main<wl_shm::WlShm> = registry.bind(version.min(1), id);
                            *shm_for_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
                        }
                        "wl_output" => {
                            let out_list = outputs_for_cb.clone();
                            let first_flag = is_first_for_cb.clone();
                            let mut handles = wl_output_handles_for_cb
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            let idx = handles.len();
                            let proxy: Main<wl_output::WlOutput> =
                                registry.bind(version.min(2), id);
                            proxy.quick_assign(move |_, event, _| {
                                let mut list = out_list.lock().unwrap_or_else(|e| e.into_inner());
                                while list.len() <= idx {
                                    list.push(output::RawOutput::default());
                                }
                                let entry = &mut list[idx];
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
                                        const WL_OUTPUT_MODE_CURRENT: u32 = 0x1;
                                        if (flags.to_raw() & WL_OUTPUT_MODE_CURRENT) != 0 {
                                            entry.width = width;
                                            entry.height = height;
                                        }
                                    }
                                    wl_output::Event::Scale { factor } => {
                                        entry.scale = factor;
                                    }
                                    wl_output::Event::Done => {
                                        if let Ok(mut first) = first_flag.lock() {
                                            if *first {
                                                *first = false;
                                                entry.is_primary = true;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            });
                            handles.push(proxy);
                        }
                        "zwp_text_input_manager_v3" => {
                            let proxy: Main<ZwpTextInputManagerV3> =
                                registry.bind(version.min(1), id);
                            *tim_for_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
                        }
                        "xdg_activation_v1" => {
                            let proxy: Main<XdgActivationV1> = registry.bind(version.min(1), id);
                            *xa_for_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
                        }
                        _ => {}
                    }
                }
            }
        });

        // ── 初始 roundtrip：接收所有全局广告事件 ──────────────────
        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;

        // ── 提取单例对象 ──────────────────────────────────────────
        let _compositor = compositor_cell
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or("no wl_compositor".to_string())?;
        let _wm_base = wm_base_cell
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or("no xdg_wm_base (need xdg-shell)".to_string())?;
        let _shm = shm_cell
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or("no wl_shm".to_string())?;

        _wm_base.quick_assign(|wm, event, _| {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm.pong(serial);
            }
        });

        // ── 第二次 roundtrip：收集 wl_output 的 geometry/mode/scale/done ──
        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .map_err(|e| format!("Wayland output roundtrip failed: {}", e))?;

        let _wl_outputs = {
            let mut handles = wl_output_handles.lock().unwrap_or_else(|e| e.into_inner());
            handles.drain(..).collect::<Vec<_>>()
        };

        // ── 其余共享状态初始化 ────────────────────────────────────
        let events: Arc<Mutex<VecDeque<UiEvent>>> = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard_text: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let owns_clipboard: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let clipboard_read_fd: Arc<Mutex<Option<RawFd>>> = Arc::new(Mutex::new(None));
        let last_pointer: Arc<Mutex<LastPointerState>> =
            Arc::new(Mutex::new(LastPointerState::default()));

        // data_device_manager 需要通过 globals 绑定
        // 注意：由于使用了 new_with_cb，globals 内部没有跟踪 wl_data_device_manager，
        // 我们需要手动使用 instantiate_exact。
        // 但实际上 GlobalManager::new_with_cb 仍会跟踪所有全局，
        // 所以 instantiate_exact 仍然可用。
        let data_device_manager = globals
            .instantiate_exact::<wl_data_device_manager::WlDataDeviceManager>(3)
            .ok();

        let text_input_manager = text_input_manager_cell
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();

        // 提取 xdg_activation（先求值再用于 struct init，避免 MutexGuard 生命周期问题）
        let xdg_activation = xdg_activation_cell
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();

        let (wake_read_fd, wake_write_fd) = Self::create_wake_pipe()?;

        Ok(Self {
            display,
            event_queue,
            _globals: globals,
            _compositor,
            _wm_base,
            _shm,
            surface: None,
            xdg_surface: None,
            toplevel: None,
            width: 800,
            height: 600,
            shown: false,
            closed: false,
            configured: false,
            maximized: Arc::new(Mutex::new(false)),
            fullscreen: Arc::new(Mutex::new(false)),
            shm_buffers: [None, None],
            active_buffer: 0,
            events,
            seat: None,
            pointer: Arc::new(Mutex::new(None)),
            keyboard: Arc::new(Mutex::new(None)),
            last_pointer,
            keys_down: Arc::new(Mutex::new(HashSet::new())),
            repeat_rate: Arc::new(Mutex::new(0)),
            repeat_delay: Arc::new(Mutex::new(400)),
            held_key_info: Arc::new(Mutex::new(None)),
            last_repeat_time: Arc::new(Mutex::new(None)),
            input_region: None,
            data_device_manager,
            data_device: None,
            clipboard_text,
            owns_clipboard,
            clipboard_read_fd,
            wake_read_fd,
            wake_write_fd,
            outputs,
            _wl_outputs,
            text_input_manager,
            text_input: None,
            text_input_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            _xdg_activation: xdg_activation,
            next_window_id: 1,
        })
    }
}

impl Drop for WaylandBackend {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.wake_read_fd) };
        let _ = unsafe { libc::close(self.wake_write_fd) };
    }
}
