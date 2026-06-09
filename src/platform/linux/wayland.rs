// ============================================================================
// platform/linux/wayland.rs — Native Wayland backend (wayland-client 0.29)
// ============================================================================

use crate::platform::event::*;
use crate::platform::types::*;
use crate::platform::*;

use crate::platform::linux::backend::Backend;
use crate::platform::linux::clipboard::{ClipboardState, LinuxClipboard};

use std::collections::VecDeque;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

use wayland_client::{
    protocol::{wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_shm_pool, wl_surface},
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
    /// Write pixel data to the shared memory buffer.
    /// Internal pixel format is ABGR8888 (0xAABBGGRR, byte order R,G,B,A on LE),
    /// which matches wl_shm::Format::Abgr8888 directly.
    /// Returns the number of bytes written.
    fn write_pixels(&mut self, pixels: &[u32]) -> std::io::Result<usize> {
        use std::io::{Seek, Write};
        let byte_len = pixels.len().min(self.size / 4) * 4;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // Convert &[u32] to &[u8] via safe pointer cast
        let bytes = unsafe {
            std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len)
        };
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

    // UiEvent queue (thread-safe for quick_assign callbacks)
    events: Arc<Mutex<VecDeque<UiEvent>>>,

    // Server-side decoration (title bar with min/max/close)
    decoration_manager: Option<Main<OrgKdeKwinServerDecorationManager>>,
    decoration: Option<Main<OrgKdeKwinServerDecoration>>,

    // SHM buffer (RAII — owns fd, pool, and buffer)
    shm_buffer: Option<ShmBuffer>,

    // Last pointer position (for Button events that lack position)
    last_pointer: Arc<Mutex<LastPointerState>>,

    // Backend subsystems
    clipboard: LinuxClipboard,
}

impl WaylandBackend {
    pub fn new() -> Result<Self, String> {
        let display = Display::connect_to_env()
            .map_err(|e| format!("Wayland connect failed: {}", e))?;

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
            decoration_manager: None,
            decoration: None,
            shm_buffer: None,
            events,
            last_pointer,
            clipboard,
        })
    }

    /// Present a BGRA pixel buffer to the Wayland surface.
    /// Writes pixels to the SHM buffer, attaches it, damages the surface, and commits.
    pub fn present_pixels(&mut self, pixels: &[u32], width: i32, height: i32) {
        let w = self.width;
        let h = self.height;

        // Create or re-create SHM buffer if missing or dimensions changed
        if self.shm_buffer.is_none() || width != w || height != h {
            self.width = width;
            self.height = height;
            if let Ok(new_buf) = Self::create_shm_buffer(&self._shm, width, height) {
                self.shm_buffer = Some(new_buf);
            } else {
                log::warn!("Wayland: failed to create SHM buffer for {}x{}", width, height);
                return;
            }
        }

        if let Some(ref mut shm) = self.shm_buffer {
            if let Err(e) = shm.write_pixels(pixels) {
                log::warn!("Wayland: write_pixels failed: {}", e);
                return;
            }
            if let Some(ref surface) = self.surface {
                surface.attach(Some(&shm.buffer), 0, 0);
                surface.damage(0, 0, width, height);
                surface.commit();
                if !self.shown {
                    self.shown = true;
                }
            }
            let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        }
    }

    pub(crate) fn create_window_inner(&mut self, title: &str, width: i32, height: i32) -> bool {
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
                let _ = cfg_events.lock().unwrap_or_else(|e| e.into_inner())
                    .push_back(UiEvent::resize(0, 0));
            }
        });

        // xdg_toplevel events → UiEvent
        let toplevel_events = events.clone();
        toplevel.quick_assign(move |_, event, _| {
            match event {
                xdg_toplevel::Event::Close => {
                    let _ = toplevel_events.lock().unwrap_or_else(|e| e.into_inner())
                        .push_back(UiEvent::close());
                }
                xdg_toplevel::Event::Configure { width: w, height: h, .. } => {
                    if w > 0 && h > 0 {
                        let _ = toplevel_events.lock().unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent::resize(w, h));
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
                                wl_pointer::Event::Enter { surface_x, surface_y, .. }
                                | wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
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
                                    let click_pos = pos.lock()
                                        .map(|lp| lp.position)
                                        .unwrap_or_default();
                                    if state == wl_pointer::ButtonState::Pressed {
                                        q.push_back(UiEvent::mouse_down(click_pos, btn));
                                    } else {
                                        q.push_back(UiEvent::mouse_up(click_pos, btn));
                                    }
                                }
                                wl_pointer::Event::Axis { axis, value, .. } => {
                                    let (dx, dy) = match axis {
                                        wl_pointer::Axis::VerticalScroll => (0.0, value),
                                        wl_pointer::Axis::HorizontalScroll => (value, 0.0),
                                        _ => (0.0, 0.0),
                                    };
                                    if dx != 0.0 || dy != 0.0 {
                                        q.push_back(UiEvent::mouse_wheel(
                                            Point::default(), dx as f32, dy as f32, KeyMod::NONE,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        });
                    }
                    if capabilities.contains(Capability::Keyboard) {
                        let ev = ptr_events.clone();
                        let kbd = seat.get_keyboard();
                        kbd.quick_assign(move |_, event, _| {
                            if let wl_keyboard::Event::Key { key, state, .. } = event {
                                let code = linux_keycode_to_keycode(key);
                                let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                                if state == wl_keyboard::KeyState::Pressed {
                                    q.push_back(UiEvent::key_down(code, KeyMod::NONE));
                                } else {
                                    q.push_back(UiEvent::key_up(code, KeyMod::NONE));
                                }
                            }
                        });
                    }
                }
            });
        }

        // Request server-side decorations (title bar) via KDE protocol
        match self._globals.instantiate_exact::<OrgKdeKwinServerDecorationManager>(1) {
            Ok(dm) => {
                let deco = dm.create(&surface);
                deco.request_mode(Mode::Server);
                self.decoration_manager = Some(dm);
                self.decoration = Some(deco);
            }
            Err(_) => {
                log::warn!("Wayland: no server_decoration_manager (running on non-KDE compositor?)");
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
        if let Some(ref s) = self.surface { s.commit(); }

        self.configured = true;
        true
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
            if writer.write_all(&pixel).is_err() { break; }
        }
        writer.flush().ok();
        drop(writer);

        let raw_fd = file.as_raw_fd();
        let pool = shm.create_pool(raw_fd, size as i32);
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Abgr8888);

        // Clean up temp file (the fd stays open via `file`)
        let _ = std::fs::remove_file(&tmp_path);

        Ok(ShmBuffer { file, size, pool, buffer })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for WaylandBackend {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> bool {
        self.create_window_inner(title, width, height)
    }
    fn destroy_window(&mut self) {
        self.decoration = None;
        self.decoration_manager = None;
        self.shm_buffer = None;
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
        if let Some(ref t) = self.toplevel { t.set_minimized(); }
    }
    fn is_visible(&self) -> bool { self.shown }
    fn center_on_screen(&mut self) { /* Wayland compositor controls placement */ }
    fn raise(&mut self) { /* Wayland compositor controls stacking */ }
    fn lower(&mut self) { /* Wayland compositor controls stacking */ }
    fn set_window_icon(&mut self, _: &str) { log::warn!("Wayland: set_window_icon not implemented"); }
    fn flash_window(&mut self) { log::warn!("Wayland: flash_window not implemented"); }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for WaylandBackend {
    fn width(&self) -> i32 { self.width }
    fn height(&self) -> i32 { self.height }

    fn set_size(&mut self, w: i32, h: i32) {
        self.width = w; self.height = h;
        if let Some(ref xs) = self.xdg_surface {
            xs.set_window_geometry(0, 0, w, h);
        }
    }
    fn set_minimum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_min_size(w, h); }
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) {
        if let Some(ref t) = self.toplevel { t.set_max_size(w, h); }
    }
    fn position(&self) -> Point { Point::default() /* Wayland: no absolute position */ }
    fn set_position(&mut self, _: i32, _: i32) { /* Wayland: compositor-controlled */ }
    fn set_resizable(&mut self, _: bool) { /* Wayland: xdg-shell handles this */ }
    fn is_maximized(&self) -> bool { false /* TODO: track via xdg_toplevel configure events */ }
    fn is_minimized(&self) -> bool { false }
    fn maximize(&mut self) { if let Some(ref t) = self.toplevel { t.set_maximized(); } }
    fn minimize(&mut self) { if let Some(ref t) = self.toplevel { t.set_minimized(); } }
    fn restore(&mut self) {
        if let Some(ref t) = self.toplevel {
            t.unset_maximized();
            t.unset_fullscreen();
        }
    }
    fn set_borderless(&mut self, _: bool) { log::warn!("Wayland: set_borderless not directly supported"); }
    fn set_fullscreen(&mut self, fullscreen: bool) {
        if let Some(ref t) = self.toplevel {
            if fullscreen { t.set_fullscreen(None); } else { t.unset_fullscreen(); }
        }
    }
    fn is_fullscreen(&self) -> bool { false /* TODO: track via xdg_toplevel configure events */ }
    fn set_always_on_top(&mut self, _: bool) { log::warn!("Wayland: set_always_on_top not supported"); }
    fn set_window_opacity(&mut self, _: f32) { log::warn!("Wayland: set_window_opacity not supported"); }
    fn start_text_input(&mut self) { log::warn!("Wayland: text_input protocol not yet implemented"); }
    fn stop_text_input(&mut self) {}
    fn enable_file_drop(&mut self, _: bool) { log::warn!("Wayland: file drop not yet implemented"); }
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop
// ════════════════════════════════════════════════════════════════════════════

impl IEventLoop for WaylandBackend {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        let mut q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(event) = q.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        drop(q);
        if self.closed { return false; }
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

impl Backend for WaylandBackend {
    fn present_pixels(&mut self, pixels: &[u32], width: i32, height: i32) {
        self.present_pixels(pixels, width, height);
    }

    fn cursor(&mut self) -> &mut dyn ICursor {
        struct WC;
        impl ICursor for WC {
            fn set_cursor(&mut self, _: CursorType) {}
            fn show_cursor(&mut self, _: bool) {}
            fn cursor_position(&self) -> Point { Point::default() }
            fn set_cursor_position(&mut self, _: i32, _: i32) {}
            fn confine_cursor(&mut self, _: bool) {}
            fn capture_mouse(&mut self) {}
            fn release_mouse(&mut self) {}
        }
        Box::leak(Box::new(WC))
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        struct WK;
        impl IKeyboard for WK {
            fn is_down(&self, _: KeyCode) -> bool { false }
            fn idle_ms(&self) -> u32 { 0 }
            fn double_click_ms(&self) -> u32 { 400 }
        }
        static K: WK = WK;
        &K
    }
    fn display(&self) -> &dyn IDisplay {
        struct WD;
        impl IDisplay for WD {
            fn dpi_scale(&self) -> f32 { 1.0 }
            fn is_dark_mode(&self) -> bool {
                std::env::var("GTK_THEME").map(|t| t.contains("dark")).unwrap_or(false)
            }
            fn count(&self) -> i32 { 1 }
            fn info(&self, _: i32) -> DisplayInfo {
                DisplayInfo {
                    bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
                    dpi_scale: 1.0, is_primary: true,
                }
            }
        }
        static D: WD = WD;
        &D
    }
    fn clipboard(&mut self) -> &mut dyn IClipboard { &mut self.clipboard }
}

// ════════════════════════════════════════════════════════════════════════════
// Linux keycode → KeyCode
// ════════════════════════════════════════════════════════════════════════════

fn linux_keycode_to_keycode(code: u32) -> KeyCode {
    match code {
        1 => KeyCode::Escape,
        2..=11 => [KeyCode::Num1, KeyCode::Num2, KeyCode::Num3, KeyCode::Num4,
                   KeyCode::Num5, KeyCode::Num6, KeyCode::Num7, KeyCode::Num8,
                   KeyCode::Num9, KeyCode::Num0][(code - 2) as usize],
        14 => KeyCode::Backspace, 15 => KeyCode::Tab,
        16 => KeyCode::Q, 17 => KeyCode::W, 18 => KeyCode::E, 19 => KeyCode::R,
        20 => KeyCode::T, 21 => KeyCode::Y, 22 => KeyCode::U, 23 => KeyCode::I,
        24 => KeyCode::O, 25 => KeyCode::P,
        28 => KeyCode::Enter, 29 => KeyCode::Ctrl,
        30 => KeyCode::A, 31 => KeyCode::S, 32 => KeyCode::D, 33 => KeyCode::F,
        34 => KeyCode::G, 35 => KeyCode::H, 36 => KeyCode::J, 37 => KeyCode::K,
        38 => KeyCode::L,
        42 => KeyCode::Shift,
        44 => KeyCode::Z, 45 => KeyCode::X, 46 => KeyCode::C, 47 => KeyCode::V,
        48 => KeyCode::B, 49 => KeyCode::N, 50 => KeyCode::M,
        56 => KeyCode::Alt, 57 => KeyCode::Space,
        59..=68 => [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4,
                    KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8,
                    KeyCode::F9, KeyCode::F10][(code - 59) as usize],
        87 => KeyCode::F11, 88 => KeyCode::F12,
        97 => KeyCode::Ctrl, 100 => KeyCode::Alt,
        102 => KeyCode::Home, 103 => KeyCode::Up, 104 => KeyCode::PageUp,
        105 => KeyCode::Left, 106 => KeyCode::Right, 107 => KeyCode::End,
        108 => KeyCode::Down, 109 => KeyCode::PageDown,
        110 => KeyCode::Insert, 111 => KeyCode::Delete,
        125 | 126 => KeyCode::Super,
        _ => KeyCode::Unknown,
    }
}
