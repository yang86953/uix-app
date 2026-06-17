// ============================================================================
// platform/linux/wayland.rs — Native Wayland backend (wayland-client 0.29)
// ============================================================================

use crate::base::{KeyCode, KeyMod, MouseButton, Point, Rect};
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::types::{CursorType, DisplayInfo};
use crate::platform::*;

use crate::platform::linux::backend::Backend;
use crate::platform::linux::clipboard::{ClipboardState, LinuxClipboard};

use std::collections::VecDeque;
use std::fs::File;
use std::os::unix::io::AsRawFd;

use libc::{poll, pollfd, POLLIN};
use std::sync::{Arc, Mutex};

use wayland_client::{
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Display, EventQueue, GlobalManager, Main,
};
use wayland_protocols::misc::server_decoration::client::{
    org_kde_kwin_server_decoration::{Mode, OrgKdeKwinServerDecoration},
    org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager,
};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

// ════════════════════════════════════════════════════════════════════════════
// ShmBuffer — RAII wrapper for SHM pool + buffer + backing file
// ════════════════════════════════════════════════════════════════════════════

struct ShmBuffer {
    /// The backing temp file (kept alive to prevent fd close)
    file: File,
    /// Size of the buffer in bytes
    size: usize,
    /// SHM pool (kept alive; dropping it would invalidate the buffer)
    #[allow(dead_code)]
    pool: Main<wl_shm_pool::WlShmPool>,
    /// Buffer for window content
    buffer: Main<wl_buffer::WlBuffer>,
}

impl ShmBuffer {
    /// Write BGRA pixel data to the shared memory buffer.
    /// `pixels` is &[u32] in ARGB8888 format (0xAARRGGBB, BGRA byte order on LE).
    /// Returns the number of bytes written.
    fn write_pixels(&mut self, pixels: &[u32]) -> std::io::Result<usize> {
        use std::io::{Seek, Write};
        let byte_len = pixels.len().min(self.size / 4) * 4;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // Convert &[u32] to &[u8] via safe pointer cast
        let bytes = unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len) };
        self.file.write(bytes)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// LastPointerState — tracks cursor position for Button events
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct LastPointerState {
    position: Point,
}

// ════════════════════════════════════════════════════════════════════════════
// WaylandBackend
// ════════════════════════════════════════════════════════════════════════════

pub struct WaylandBackend {
    #[allow(dead_code)]
    display: Display,
    event_queue: EventQueue,
    _globals: GlobalManager,

    // Keep-alive protocol globals (Main<T> handles, never dropped while backend lives)
    _compositor: Main<wl_compositor::WlCompositor>,
    _wm_base: Main<xdg_wm_base::XdgWmBase>,
    _shm: Main<wl_shm::WlShm>,

    // Window objects (Main<T> handles)
    surface: Option<Main<wl_surface::WlSurface>>,
    xdg_surface: Option<Main<xdg_surface::XdgSurface>>,
    toplevel: Option<Main<xdg_toplevel::XdgToplevel>>,

    // Window state
    width: i32,
    height: i32,
    shown: bool,
    closed: bool,
    configured: bool,
    /// Tracked from xdg_toplevel configure events (state = 1 = maximized)
    maximized: Arc<Mutex<bool>>,
    /// Tracked from xdg_toplevel configure events (state = 3 or 4 = fullscreen)
    fullscreen: Arc<Mutex<bool>>,

    // UiEvent queue (thread-safe for quick_assign callbacks)
    events: Arc<Mutex<VecDeque<UiEvent>>>,

    // Server-side decoration (title bar with min/max/close)
    decoration_manager: Option<Main<OrgKdeKwinServerDecorationManager>>,
    decoration: Option<Main<OrgKdeKwinServerDecoration>>,

    // SHM double buffers — always compose into the buffer that the
    // compositor is NOT currently displaying, eliminating contention.
    shm_buffers: [Option<ShmBuffer>; 2],
    active_buffer: usize,

    // Last pointer position (for Button events that lack position)
    last_pointer: Arc<Mutex<LastPointerState>>,

    // Backend subsystems
    clipboard: LinuxClipboard,
}

impl WaylandBackend {
    pub fn new() -> Result<Self, String> {
        let display =
            Display::connect_to_env().map_err(|e| format!("Wayland connect failed: {}", e))?;

        let mut event_queue = display.create_event_queue();
        let attached = (*display).clone().attach(event_queue.token());
        let globals = GlobalManager::new(&attached);

        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;

        // Required globals
        let _compositor = globals
            .instantiate_exact::<wl_compositor::WlCompositor>(4)
            .map_err(|_| "no wl_compositor".to_string())?;
        let _wm_base = globals
            .instantiate_exact::<xdg_wm_base::XdgWmBase>(1)
            .map_err(|_| "no xdg_wm_base (need xdg-shell)".to_string())?;
        let _shm = globals
            .instantiate_exact::<wl_shm::WlShm>(1)
            .map_err(|_| "no wl_shm".to_string())?;

        // Ping handler for xdg_wm_base
        _wm_base.quick_assign(|wm, event, _| {
            if let xdg_wm_base::Event::Ping { serial } = event {
                wm.pong(serial);
            }
        });

        let events: Arc<Mutex<VecDeque<UiEvent>>> = Arc::new(Mutex::new(VecDeque::new()));
        let clipboard = LinuxClipboard::new(Arc::new(Mutex::new(ClipboardState::new())));
        let last_pointer: Arc<Mutex<LastPointerState>> =
            Arc::new(Mutex::new(LastPointerState::default()));

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
            shm_buffers: [None, None],
            active_buffer: 0,
            events,
            last_pointer,

            clipboard,
        })
    }

    /// Non-blocking event dispatch: flushes pending requests, then uses
    /// `poll()` to check if any data is available on the Wayland socket.
    /// Only calls the blocking `dispatch()` when data is actually ready,
    /// so the function never blocks waiting for events.
    fn try_dispatch(&mut self) {
        let _ = self.display.flush();
        let fd = self.display.get_connection_fd();
        let mut pfd = pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let ret = unsafe { poll(&mut pfd as *mut pollfd, 1, 0) };
        if ret > 0 && (pfd.revents & POLLIN) != 0 {
            let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        }
    }

    pub fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) {
        // Create/resize both buffers when dimensions change.
        if width != self.width
            || height != self.height
            || self.shm_buffers[0].is_none()
            || self.shm_buffers[1].is_none()
        {
            self.width = width;
            self.height = height;
            for buf in self.shm_buffers.iter_mut() {
                if let Ok(new_buf) = Self::create_shm_buffer(&self._shm, width, height) {
                    *buf = Some(new_buf);
                } else {
                    log::warn!("Wayland: SHM buffer {}x{} failed", width, height);
                    return;
                }
            }
        }

        // Write to the buffer the compositor is NOT reading.
        // active_buffer tracks the last committed (displayed) buffer;
        // we write to the other one.
        let write_idx = 1 - self.active_buffer;
        let shm = match self.shm_buffers[write_idx].as_mut() {
            Some(s) => s,
            None => return,
        };
        if let Err(e) = shm.write_pixels(pixels) {
            log::warn!("Wayland: write_pixels failed: {}", e);
            return;
        }

        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        surface.attach(Some(&shm.buffer), 0, 0);
        // 局部 damage：仅标记实际发生变化的区域，减少合成器工作量
        match dirty_rect {
            Some((x, y, w, h)) if w > 0 && h > 0 => {
                surface.damage_buffer(x, y, w, h);
            }
            _ => {
                surface.damage(0, 0, width, height);
            }
        }
        surface.commit();
        self.active_buffer = write_idx;

        if !self.shown {
            self.shown = true;
        }

        self.try_dispatch();
    }

    pub(crate) fn create_window_inner(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<(), Error> {
        self.width = width;
        self.height = height;

        let events = self.events.clone();
        let last_pointer = self.last_pointer.clone();

        let surface = self._compositor.create_surface();
        let xdg_surface = self._wm_base.get_xdg_surface(&surface);
        let toplevel = xdg_surface.get_toplevel();
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("uix-app".to_string());

        // xdg_surface configure → ack_configure + mark configured
        let cfg_events = events.clone();
        xdg_surface.quick_assign(move |xs, event, _| {
            if let xdg_surface::Event::Configure { serial } = event {
                xs.ack_configure(serial);
                let _ = cfg_events
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push_back(UiEvent::resize(0, 0));
            }
        });

        // xdg_toplevel events → UiEvent
        let toplevel_events = events.clone();
        let maximized_state = self.maximized.clone();
        let fullscreen_state = self.fullscreen.clone();
        toplevel.quick_assign(move |_, event, _| {
            match event {
                xdg_toplevel::Event::Close => {
                    let _ = toplevel_events
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_back(UiEvent::close());
                }
                xdg_toplevel::Event::Configure {
                    width: w,
                    height: h,
                    states,
                } => {
                    // Parse xdg_toplevel_state from wl_array of u32:
                    //   1 = maximized, 2 = fullscreen
                    let is_maximized = states
                        .chunks_exact(4)
                        .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 1);
                    let is_fullscreen = states
                        .chunks_exact(4)
                        .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 2);
                    if let Ok(mut m) = maximized_state.lock() {
                        *m = is_maximized;
                    }
                    if let Ok(mut f) = fullscreen_state.lock() {
                        *f = is_fullscreen;
                    }
                    if w > 0 && h > 0 {
                        let _ = toplevel_events
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent::resize(w, h));
                    } else if is_maximized {
                        // Compositor wants us maximized without giving
                        // explicit dimensions.  Push a no-op resize so
                        // the widget tree knows the state changed.
                        let _ = toplevel_events
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent::resize(0, 0));
                    }
                }
                _ => {}
            }
        });

        // Seat: pointer + keyboard
        if let Ok(seat) = self._globals.instantiate_exact::<wl_seat::WlSeat>(7) {
            let ptr_events = self.events.clone();
            let ptr_pos = last_pointer.clone();
            seat.quick_assign(move |seat, event, _| {
                if let wl_seat::Event::Capabilities { capabilities } = event {
                    use wayland_client::protocol::wl_seat::Capability;
                    if capabilities.contains(Capability::Pointer) {
                        let ev = ptr_events.clone();
                        let pos = ptr_pos.clone();
                        let ptr = seat.get_pointer();
                        ptr.quick_assign(move |_, event, _| {
                            let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                            match event {
                                wl_pointer::Event::Enter {
                                    surface_x,
                                    surface_y,
                                    ..
                                }
                                | wl_pointer::Event::Motion {
                                    surface_x,
                                    surface_y,
                                    ..
                                } => {
                                    let p = Point::new(surface_x as f32, surface_y as f32);
                                    // Track last position for button events
                                    if let Ok(mut lp) = pos.lock() {
                                        lp.position = p;
                                    }
                                    q.push_back(UiEvent::mouse_move(p));
                                }
                                wl_pointer::Event::Button { button, state, .. } => {
                                    let btn = match button {
                                        0x110 => MouseButton::Left,
                                        0x111 => MouseButton::Right,
                                        0x112 => MouseButton::Middle,
                                        _ => MouseButton::None,
                                    };
                                    // Use tracked position instead of Point::default()
                                    let click_pos =
                                        pos.lock().map(|lp| lp.position).unwrap_or_default();
                                    if state == wl_pointer::ButtonState::Pressed {
                                        q.push_back(UiEvent::mouse_down(click_pos, btn));
                                    } else {
                                        q.push_back(UiEvent::mouse_up(click_pos, btn));
                                    }
                                }
                                wl_pointer::Event::Axis { axis, value, .. } => {
                                    // Natural scrolling: content follows the
                                    // scroll direction.  Wheel down → content
                                    // down, wheel up → content up.
                                    let (dx, dy) = match axis {
                                        wl_pointer::Axis::VerticalScroll => (0.0, value),
                                        wl_pointer::Axis::HorizontalScroll => (value, 0.0),
                                        _ => (0.0, 0.0),
                                    };
                                    if dx != 0.0 || dy != 0.0 {
                                        q.push_back(UiEvent::mouse_wheel(
                                            Point::default(),
                                            dx as f32,
                                            dy as f32,
                                            KeyMod::NONE,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        });
                    }
                    if capabilities.contains(Capability::Keyboard) {
                        let ev = ptr_events.clone();
                        let mods = Arc::new(Mutex::new(KeyMod::NONE));
                        let kbd = seat.get_keyboard();
                        kbd.quick_assign(move |_, event, _| {
                            match event {
                                wl_keyboard::Event::Key { key, state, .. } => {
                                    let code = linux_keycode_to_keycode(key);
                                    let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                                    let shift_down = mods
                                        .lock()
                                        .map(|m| m.intersects(KeyMod::SHIFT))
                                        .unwrap_or(false);
                                    if state == wl_keyboard::KeyState::Pressed {
                                        q.push_back(UiEvent::key_down(code, KeyMod::NONE));
                                        // 生成 KeyPress 文本字符（可打印字符才有）
                                        // 使用已映射的 KeyCode 而非原始 evdev 码，
                                        // 确保与 linux_keycode_to_keycode 映射一致。
                                        if let Some(text) = keycode_to_char(code, shift_down) {
                                            q.push_back(UiEvent::key_press(text));
                                        }
                                    } else {
                                        q.push_back(UiEvent::key_up(code, KeyMod::NONE));
                                    }
                                }
                                wl_keyboard::Event::Modifiers { mods_depressed, .. } => {
                                    if let Ok(mut m) = mods.lock() {
                                        const SHIFT_MASK: u32 = 1; // wl_keyboard modifier bit 0
                                        *m = if (mods_depressed & SHIFT_MASK) != 0 {
                                            KeyMod::SHIFT
                                        } else {
                                            KeyMod::NONE
                                        };
                                    }
                                }
                                _ => {}
                            }
                        });
                    }
                }
            });
        }

        // Request server-side decorations (title bar) via KDE protocol
        match self
            ._globals
            .instantiate_exact::<OrgKdeKwinServerDecorationManager>(1)
        {
            Ok(dm) => {
                let deco = dm.create(&surface);
                deco.request_mode(Mode::Server);
                self.decoration_manager = Some(dm);
                self.decoration = Some(deco);
            }
            Err(_) => {
                log::warn!(
                    "Wayland: no server_decoration_manager (running on non-KDE compositor?)"
                );
            }
        }

        // First commit triggers xdg_surface.configure
        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surface);
        self.toplevel = Some(toplevel);

        // Roundtrip: process configure event + ack_configure
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});

        // Commit without buffer to acknowledge the configure
        // Surface will become visible only on first present_pixels() call,
        // which provides the real UI content — no white flash at startup.
        if let Some(ref s) = self.surface {
            s.commit();
        }

        self.configured = true;
        Ok(())
    }

    /// Create an SHM buffer with solid white fill.
    /// Owns the backing temp file via RAII to prevent fd leak.
    fn create_shm_buffer(
        shm: &Main<wl_shm::WlShm>,
        width: i32,
        height: i32,
    ) -> Result<ShmBuffer, String> {
        let stride = width * 4;
        let size = (stride * height) as usize;

        // Create a temp file for shared memory
        let tmp_path = std::env::temp_dir().join(format!("uix-shm-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| format!("shm temp file create: {}", e))?;

        // Allocate space
        file.set_len(size as u64).ok();

        // Fill with white pixels (ARGB8888)
        use std::io::Write;
        let pixel = 0xFF_FF_FF_FFu32.to_ne_bytes();
        // Use a BufWriter for performance
        let mut writer = std::io::BufWriter::new(&file);
        for _ in 0..(width * height) {
            if writer.write_all(&pixel).is_err() {
                break;
            }
        }
        writer.flush().ok();
        drop(writer);

        let raw_fd = file.as_raw_fd();
        let pool = shm.create_pool(raw_fd, size as i32);
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888);

        // Clean up temp file (the fd stays open via `file`)
        let _ = std::fs::remove_file(&tmp_path);

        Ok(ShmBuffer {
            file,
            size,
            pool,
            buffer,
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for WaylandBackend {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<(), Error> {
        self.create_window_inner(title, width, height)
    }
    fn destroy_window(&mut self) {
        self.decoration = None;
        self.decoration_manager = None;
        self.shm_buffers = [None, None];
        self.toplevel = None;
        self.xdg_surface = None;
        self.surface = None;
        self.shown = false;
    }
    fn set_title(&mut self, title: &str) {
        if let Some(ref t) = self.toplevel {
            t.set_title(title.to_string());
        }
    }
    fn show(&mut self) {
        if !self.shown {
            self.shown = true;
            if let Some(ref s) = self.surface {
                if !self.configured {
                    let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
                }
                s.commit();
            }
        }
    }
    fn hide(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.set_minimized();
        }
    }
    fn is_visible(&self) -> bool {
        self.shown
    }
    fn center_on_screen(&mut self) { /* Wayland compositor controls placement */
    }
    fn raise(&mut self) { /* Wayland compositor controls stacking */
    }
    fn lower(&mut self) { /* Wayland compositor controls stacking */
    }
    fn set_window_icon(&mut self, _: &str) {
        log::warn!("Wayland: set_window_icon not implemented");
    }
    fn flash_window(&mut self) {
        log::warn!("Wayland: flash_window not implemented");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for WaylandBackend {
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }

    fn set_size(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        if let Some(ref xs) = self.xdg_surface {
            xs.set_window_geometry(0, 0, w, h);
        }
    }
    fn set_minimum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel {
            t.set_min_size(w, h);
        }
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel {
            t.set_max_size(w, h);
        }
    }
    fn position(&self) -> Point {
        Point::default() /* Wayland: no absolute position */
    }
    fn set_position(&mut self, _: i32, _: i32) { /* Wayland: compositor-controlled */
    }
    fn set_resizable(&mut self, _: bool) { /* Wayland: xdg-shell handles this */
    }
    fn is_maximized(&self) -> bool {
        self.maximized.lock().map(|m| *m).unwrap_or(false)
    }
    fn is_minimized(&self) -> bool {
        false
    }
    fn maximize(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.set_maximized();
        }
    }
    fn minimize(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.set_minimized();
        }
    }
    fn restore(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.unset_maximized();
            t.unset_fullscreen();
        }
    }
    fn set_borderless(&mut self, _: bool) {
        log::warn!("Wayland: set_borderless not directly supported");
    }
    fn set_fullscreen(&mut self, fullscreen: bool) {
        if let Some(ref t) = self.toplevel {
            if fullscreen {
                t.set_fullscreen(None);
            } else {
                t.unset_fullscreen();
            }
        }
    }
    fn is_fullscreen(&self) -> bool {
        self.fullscreen.lock().map(|f| *f).unwrap_or(false)
    }
    fn set_always_on_top(&mut self, _: bool) {
        log::warn!("Wayland: set_always_on_top not supported");
    }
    fn set_window_opacity(&mut self, _: f32) {
        log::warn!("Wayland: set_window_opacity not supported");
    }
    fn start_text_input(&mut self) {
        log::warn!("Wayland: text_input protocol not yet implemented");
    }
    fn stop_text_input(&mut self) {}
    fn enable_file_drop(&mut self, _: bool) {
        log::warn!("Wayland: file drop not yet implemented");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop
// ════════════════════════════════════════════════════════════════════════════

impl IEventLoop for WaylandBackend {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        self.try_dispatch();
        let mut q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(event) = q.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        drop(q);
        if self.closed {
            return false;
        }
        true
    }

    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        {
            let q = self.events.lock().unwrap_or_else(|e| e.into_inner());
            if !q.is_empty() {
                drop(q);
                return IEventLoop::poll_event(self, callback);
            }
        }
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        IEventLoop::poll_event(self, callback)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle
// ════════════════════════════════════════════════════════════════════════════

impl INativeHandle for WaylandBackend {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.surface.as_ref().map_or(std::ptr::null_mut(), |s| {
            s as *const Main<wl_surface::WlSurface> as *mut std::ffi::c_void
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Backend trait impl — delegates to trait impls + inline stubs for Wayland
// ════════════════════════════════════════════════════════════════════════════

// ════════════════════════════════════════════════════════════════════════════
// IPresenter impl — WordPress 的呈现通过 backend 自身的 SHM buffer 完成
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for WaylandBackend {
    fn present(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<(), Error> {
        self.present_pixels(pixels, width, height, None);
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // SHM buffer 在 present() 中按需重建，无需提前 resize
        Ok(())
    }
}

impl Backend for WaylandBackend {
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) {
        self.present_pixels(pixels, width, height, dirty_rect);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Backend 子 trait 的直接实现（供 LinuxPlatform 通过 &mut dyn Backend 上转型使用）
// ════════════════════════════════════════════════════════════════════════════

impl ICursor for WaylandBackend {
    fn set_cursor(&mut self, _: CursorType) {}
    fn show_cursor(&mut self, _: bool) {}
    fn cursor_position(&self) -> Point {
        Point::default()
    }
    fn set_cursor_position(&mut self, _: i32, _: i32) {}
    fn confine_cursor(&mut self, _: bool) {}
    fn capture_mouse(&mut self) {}
    fn release_mouse(&mut self) {}
}

impl IKeyboard for WaylandBackend {
    fn is_down(&self, _: KeyCode) -> bool {
        false
    }
    fn idle_ms(&self) -> u32 {
        0
    }
    fn double_click_ms(&self) -> u32 {
        400
    }
}

impl IDisplay for WaylandBackend {
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn is_dark_mode(&self) -> bool {
        std::env::var("GTK_THEME")
            .map(|t| t.contains("dark"))
            .unwrap_or(false)
    }
    fn count(&self) -> i32 {
        1
    }
    fn info(&self, _: i32) -> DisplayInfo {
        DisplayInfo {
            bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            dpi_scale: 1.0,
            is_primary: true,
        }
    }
}

impl IClipboard for WaylandBackend {
    fn text(&self) -> String {
        self.clipboard.text()
    }
    fn set_text(&mut self, text: &str) {
        self.clipboard.set_text(text);
    }
    fn has_text(&self) -> bool {
        self.clipboard.has_text()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Linux keycode → KeyCode
// ════════════════════════════════════════════════════════════════════════════

fn linux_keycode_to_keycode(code: u32) -> KeyCode {
    match code {
        1 => KeyCode::Escape,
        2..=11 => [
            KeyCode::Num1,
            KeyCode::Num2,
            KeyCode::Num3,
            KeyCode::Num4,
            KeyCode::Num5,
            KeyCode::Num6,
            KeyCode::Num7,
            KeyCode::Num8,
            KeyCode::Num9,
            KeyCode::Num0,
        ][(code - 2) as usize],
        14 => KeyCode::Backspace,
        15 => KeyCode::Tab,
        16 => KeyCode::Q,
        17 => KeyCode::W,
        18 => KeyCode::E,
        19 => KeyCode::R,
        20 => KeyCode::T,
        21 => KeyCode::Y,
        22 => KeyCode::U,
        23 => KeyCode::I,
        24 => KeyCode::O,
        25 => KeyCode::P,
        28 => KeyCode::Enter,
        29 => KeyCode::Ctrl,
        30 => KeyCode::A,
        31 => KeyCode::S,
        32 => KeyCode::D,
        33 => KeyCode::F,
        34 => KeyCode::G,
        35 => KeyCode::H,
        36 => KeyCode::J,
        37 => KeyCode::K,
        38 => KeyCode::L,
        42 => KeyCode::Shift,
        44 => KeyCode::Z,
        45 => KeyCode::X,
        46 => KeyCode::C,
        47 => KeyCode::V,
        48 => KeyCode::B,
        49 => KeyCode::N,
        50 => KeyCode::M,
        56 => KeyCode::Alt,
        57 => KeyCode::Space,
        59..=68 => [
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
            KeyCode::F5,
            KeyCode::F6,
            KeyCode::F7,
            KeyCode::F8,
            KeyCode::F9,
            KeyCode::F10,
        ][(code - 59) as usize],
        87 => KeyCode::F11,
        88 => KeyCode::F12,
        97 => KeyCode::Ctrl,
        100 => KeyCode::Alt,
        102 => KeyCode::Home,
        103 => KeyCode::Up,
        104 => KeyCode::PageUp,
        105 => KeyCode::Left,
        106 => KeyCode::Right,
        107 => KeyCode::End,
        108 => KeyCode::Down,
        109 => KeyCode::PageDown,
        110 => KeyCode::Insert,
        111 => KeyCode::Delete,
        // 小键盘（映射到主行数字 KeyCode，keycode_to_char 自动支持）
        71 => KeyCode::Num7,
        72 => KeyCode::Num8,
        73 => KeyCode::Num9,
        75 => KeyCode::Num4,
        76 => KeyCode::Num5,
        77 => KeyCode::Num6,
        79 => KeyCode::Num1,
        80 => KeyCode::Num2,
        81 => KeyCode::Num3,
        82 => KeyCode::Num0,
        125 | 126 => KeyCode::Super,
        _ => KeyCode::Unknown,
    }
}

/// Map KeyCode to ASCII printable character (US keyboard layout).
/// Returns `None` for non-printable keys (modifiers, function keys, etc.).
fn keycode_to_char(code: KeyCode, shift: bool) -> Option<String> {
    let ch = match code {
        KeyCode::A => {
            if shift {
                'A'
            } else {
                'a'
            }
        }
        KeyCode::B => {
            if shift {
                'B'
            } else {
                'b'
            }
        }
        KeyCode::C => {
            if shift {
                'C'
            } else {
                'c'
            }
        }
        KeyCode::D => {
            if shift {
                'D'
            } else {
                'd'
            }
        }
        KeyCode::E => {
            if shift {
                'E'
            } else {
                'e'
            }
        }
        KeyCode::F => {
            if shift {
                'F'
            } else {
                'f'
            }
        }
        KeyCode::G => {
            if shift {
                'G'
            } else {
                'g'
            }
        }
        KeyCode::H => {
            if shift {
                'H'
            } else {
                'h'
            }
        }
        KeyCode::I => {
            if shift {
                'I'
            } else {
                'i'
            }
        }
        KeyCode::J => {
            if shift {
                'J'
            } else {
                'j'
            }
        }
        KeyCode::K => {
            if shift {
                'K'
            } else {
                'k'
            }
        }
        KeyCode::L => {
            if shift {
                'L'
            } else {
                'l'
            }
        }
        KeyCode::M => {
            if shift {
                'M'
            } else {
                'm'
            }
        }
        KeyCode::N => {
            if shift {
                'N'
            } else {
                'n'
            }
        }
        KeyCode::O => {
            if shift {
                'O'
            } else {
                'o'
            }
        }
        KeyCode::P => {
            if shift {
                'P'
            } else {
                'p'
            }
        }
        KeyCode::Q => {
            if shift {
                'Q'
            } else {
                'q'
            }
        }
        KeyCode::R => {
            if shift {
                'R'
            } else {
                'r'
            }
        }
        KeyCode::S => {
            if shift {
                'S'
            } else {
                's'
            }
        }
        KeyCode::T => {
            if shift {
                'T'
            } else {
                't'
            }
        }
        KeyCode::U => {
            if shift {
                'U'
            } else {
                'u'
            }
        }
        KeyCode::V => {
            if shift {
                'V'
            } else {
                'v'
            }
        }
        KeyCode::W => {
            if shift {
                'W'
            } else {
                'w'
            }
        }
        KeyCode::X => {
            if shift {
                'X'
            } else {
                'x'
            }
        }
        KeyCode::Y => {
            if shift {
                'Y'
            } else {
                'y'
            }
        }
        KeyCode::Z => {
            if shift {
                'Z'
            } else {
                'z'
            }
        }
        KeyCode::Num1 => {
            if shift {
                '!'
            } else {
                '1'
            }
        }
        KeyCode::Num2 => {
            if shift {
                '@'
            } else {
                '2'
            }
        }
        KeyCode::Num3 => {
            if shift {
                '#'
            } else {
                '3'
            }
        }
        KeyCode::Num4 => {
            if shift {
                '$'
            } else {
                '4'
            }
        }
        KeyCode::Num5 => {
            if shift {
                '%'
            } else {
                '5'
            }
        }
        KeyCode::Num6 => {
            if shift {
                '^'
            } else {
                '6'
            }
        }
        KeyCode::Num7 => {
            if shift {
                '&'
            } else {
                '7'
            }
        }
        KeyCode::Num8 => {
            if shift {
                '*'
            } else {
                '8'
            }
        }
        KeyCode::Num9 => {
            if shift {
                '('
            } else {
                '9'
            }
        }
        KeyCode::Num0 => {
            if shift {
                ')'
            } else {
                '0'
            }
        }
        KeyCode::Space => ' ',
        _ => return None,
    };
    Some(ch.to_string())
}
