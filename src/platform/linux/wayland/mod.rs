// ============================================================================
// platform/linux/wayland/mod.rs — Wayland 后端核心
//
// 本文件：子模块声明、结构体定义、构造方法。
// 各子 trait 的实现在对应的子模块中。
// ============================================================================

// ── 子模块 ──────────────────────────────────────────────────────
pub(crate) mod clipboard;
pub(crate) mod creation;
pub(crate) mod cursor;
pub(crate) mod display;
pub(crate) mod event_loop;
pub(crate) mod keyboard;
pub(crate) mod keycode;
pub(crate) mod presenter;
pub(crate) mod window;

// ── 依赖 ────────────────────────────────────────────────────────
use crate::base::{KeyCode, KeyMod, MouseButton, Point};
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::*;

use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};

use libc::{poll, pollfd, POLLIN};
use std::sync::{Arc, Mutex};

use wayland_client::{
    protocol::{
        wl_buffer, wl_compositor, wl_data_device, wl_data_device_manager, wl_keyboard, wl_pointer,
        wl_region, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Display, EventQueue, GlobalManager, Main,
};
use wayland_protocols::misc::server_decoration::client::{
    org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration,
    org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager,
};
use wayland_protocols::unstable::xdg_decoration::v1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
    zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

// ════════════════════════════════════════════════════════════════════════════
// ShmBuffer — RAII 包装：SHM 池 + 缓冲区 + 后备文件
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct ShmBuffer {
    pub(crate) file: File,
    pub(crate) size: usize,
    #[allow(dead_code)]
    pub(crate) pool: Main<wl_shm_pool::WlShmPool>,
    pub(crate) buffer: Main<wl_buffer::WlBuffer>,
}

impl ShmBuffer {
    /// 将 BGRA 像素数据写入共享内存缓冲区。
    pub(crate) fn write_pixels(&mut self, pixels: &[u32]) -> std::io::Result<usize> {
        use std::io::{Seek, Write};
        let byte_len = pixels.len().min(self.size / 4) * 4;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // SAFETY: &[u32] → &[u8] 的安全转换，长度对齐确保不越界。
        let bytes = unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len) };
        self.file.write(bytes)
    }
}

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

    // ── 窗口装饰 ──────────────────────────────────────────────────
    pub(crate) decoration_manager: Option<Main<OrgKdeKwinServerDecorationManager>>,
    pub(crate) decoration: Option<Main<OrgKdeKwinServerDecoration>>,
    pub(crate) xdg_decoration_manager: Option<Main<ZxdgDecorationManagerV1>>,
    pub(crate) xdg_toplevel_decoration: Option<Main<ZxdgToplevelDecorationV1>>,

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

    // ── 剪贴板 ────────────────────────────────────────────────────
    pub(crate) data_device_manager: Option<Main<wl_data_device_manager::WlDataDeviceManager>>,
    pub(crate) data_device: Option<Main<wayland_client::protocol::wl_data_device::WlDataDevice>>,
    pub(crate) clipboard_text: Arc<Mutex<String>>,
    pub(crate) owns_clipboard: Arc<Mutex<bool>>,
    pub(crate) clipboard_read_fd: Arc<Mutex<Option<RawFd>>>,
}

impl WaylandBackend {
    /// 返回事件队列句柄（供定时器子系统使用）。
    pub fn event_queue_handle(&self) -> Arc<Mutex<VecDeque<UiEvent>>> {
        self.events.clone()
    }

    pub fn new() -> Result<Self, String> {
        let display =
            Display::connect_to_env().map_err(|e| format!("Wayland connect failed: {}", e))?;

        let mut event_queue = display.create_event_queue();
        let attached = (*display).clone().attach(event_queue.token());
        let globals = GlobalManager::new(&attached);

        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;

        let _compositor = globals
            .instantiate_exact::<wl_compositor::WlCompositor>(4)
            .map_err(|_| "no wl_compositor".to_string())?;
        let _wm_base = globals
            .instantiate_exact::<xdg_wm_base::XdgWmBase>(1)
            .map_err(|_| "no xdg_wm_base (need xdg-shell)".to_string())?;
        let _shm = globals
            .instantiate_exact::<wl_shm::WlShm>(1)
            .map_err(|_| "no wl_shm".to_string())?;

        _wm_base.quick_assign(|wm, event, _| {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm.pong(serial);
            }
        });

        let events: Arc<Mutex<VecDeque<UiEvent>>> = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard_text: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let owns_clipboard: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let clipboard_read_fd: Arc<Mutex<Option<RawFd>>> = Arc::new(Mutex::new(None));
        let last_pointer: Arc<Mutex<LastPointerState>> =
            Arc::new(Mutex::new(LastPointerState::default()));

        let data_device_manager =
            globals.instantiate_exact::<wl_data_device_manager::WlDataDeviceManager>(3).ok();

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
            decoration_manager: None,
            decoration: None,
            xdg_decoration_manager: None,
            xdg_toplevel_decoration: None,
            shm_buffers: [None, None],
            active_buffer: 0,
            events,
            seat: None,
            pointer: Arc::new(Mutex::new(None)),
            keyboard: Arc::new(Mutex::new(None)),
            last_pointer,
            keys_down: Arc::new(Mutex::new(HashSet::new())),
            input_region: None,
            data_device_manager,
            data_device: None,
            clipboard_text,
            owns_clipboard,
            clipboard_read_fd,
        })
    }
}
