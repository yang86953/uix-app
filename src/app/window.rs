use std::time::{Instant, Duration};

/// Event-driven rendering window.
/// No fixed FPS — renders on demand when events, animations, or dirty regions occur.
pub struct Window {
    title: String,
    width: i32,
    height: i32,
    running: bool,
    frame_pending: bool,
    minimized: bool,
}

impl Window {
    pub fn new(title: &str, width: i32, height: i32) -> Self {
        Self {
            title: title.to_string(),
            width,
            height,
            running: false,
            frame_pending: true,
            minimized: false,
        }
    }

    /// Run the event loop (blocks until window closes).
    pub fn run(&mut self) {
        self.running = true;
        let mut last_time = Instant::now();

        log::info!("Window '{}' started ({}x{})", self.title, self.width, self.height);

        // Event-driven loop
        while self.running {
            let now = Instant::now();
            let _dt = now.duration_since(last_time).as_secs_f32();
            last_time = now;

            // Process platform events
            self.process_events();

            if self.minimized {
                // Sleep to avoid busy-waiting when minimized
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }

            // Check if we need to render
            if self.frame_pending {
                self.do_layout();
                self.do_render();
                self.frame_pending = false;
            } else {
                // No events + no animations = deep sleep via platform WaitMessage
                // In a real implementation, platform::wait_event() blocks here
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        log::info!("Window '{}' closed", self.title);
    }

    /// Request a frame to be rendered.
    pub fn request_frame(&mut self) {
        self.frame_pending = true;
    }

    /// Close the window.
    pub fn close(&mut self) {
        self.running = false;
    }

    fn process_events(&mut self) {
        // Platform-specific event processing would go here
        // For now, this is a stub
    }

    fn do_layout(&mut self) {
        // Layout pass — would traverse widget tree and compute positions
    }

    fn do_render(&mut self) {
        // Render pass — would traverse widget tree and draw using GraphicsEngine
    }

    // ── Accessors ──

    pub fn title(&self) -> &str { &self.title }
    pub fn set_title(&mut self, title: &str) { self.title = title.to_string(); }

    pub fn width(&self) -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }

    pub fn resize(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        self.request_frame();
    }

    pub fn set_minimized(&mut self, v: bool) { self.minimized = v; }
    pub fn is_minimized(&self) -> bool { self.minimized }
    pub fn is_running(&self) -> bool { self.running }
}
