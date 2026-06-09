use crate::app::cli::Cli;
use crate::app::di::Container;
use crate::app::window::Window;
use crate::platform::create_platform;
use crate::platform::event::{UiEvent, UiEventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    GUI,
    CLI,
}

/// Application entry point — manages lifecycle, mode, DI, window, and CLI.
///
/// Does **not** duplicate window state — delegates to `Window` / `Platform`.
pub struct App {
    mode: AppMode,
    window: Option<Window>,
    cli: Option<Cli>,
    container: Container,
    running: Arc<AtomicBool>,
    exit_code: i32,
    initialized: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::GUI,
            window: None,
            cli: None,
            container: Container::new(),
            running: Arc::new(AtomicBool::new(false)),
            exit_code: 0,
            initialized: false,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize the application. Must be called before run().
    pub fn init(&mut self) -> &mut Self {
        if self.initialized {
            log::warn!("App::init: already initialized");
            return self;
        }

        self.initialized = true;
        log::info!("UIX App initialized");
        self
    }

    /// Set application mode.
    pub fn mode(&mut self, m: AppMode) -> &mut Self {
        self.mode = m;
        self
    }

    /// Get current mode.
    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    /// Create the main window (GUI mode).
    ///
    /// Internally creates a platform implementation via `create_platform()`
    /// and wraps it in a `Window`. No window state is duplicated in `App`.
    pub fn create_window(&mut self, title: &str, width: i32, height: i32) -> &mut Self {
        let platform = create_platform();
        let mut window = Window::new(platform);
        window.create(title, width, height);
        self.window = Some(window);
        self
    }

    /// Register CLI commands.
    pub fn cli(&mut self, cli: Cli) -> &mut Self {
        self.cli = Some(cli);
        self
    }

    /// Access the DI container.
    pub fn container(&mut self) -> &mut Container {
        &mut self.container
    }

    /// Register a singleton service.
    pub fn singleton<T: 'static + Send + Clone>(&mut self, instance: T) -> &mut Self {
        self.container.singleton(instance);
        self
    }

    /// Run the application (blocks until exit).
    ///
    /// ## GUI mode
    /// Runs a default event loop (no-op frame callback).
    /// Use `window_mut().run(|platform| ...)` for custom rendering.
    pub fn run(&mut self) -> i32 {
        if !self.initialized {
            log::warn!("App::run: not initialized, auto-initializing");
            self.init();
        }

        self.running.store(true, Ordering::SeqCst);

        match self.mode {
            AppMode::GUI => match self.window.as_mut() {
                Some(window) => {
                    let running = Arc::clone(&self.running);
                    window.run(move |platform| {
                        // Wait for and dispatch platform events
                        platform.wait_event(&|event: &UiEvent| {
                            match event.type_ {
                                UiEventType::WindowClose => {
                                    running.store(false, Ordering::SeqCst);
                                    false
                                }
                                _ => true,
                            }
                        })
                    });
                    0
                }
                None => {
                    log::error!("App::run: no window created for GUI mode");
                    1
                }
            },
            AppMode::CLI => {
                if let Some(ref mut cli) = self.cli {
                    let args = std::env::args().collect::<Vec<_>>();
                    self.exit_code = cli.run(&args);
                } else {
                    log::info!("Running in CLI mode (no commands registered)");
                }
                self.exit_code
            }
        }
    }

    /// Request the application to quit.
    pub fn quit(&mut self, exit_code: i32) {
        self.exit_code = exit_code;
        self.running.store(false, Ordering::SeqCst);

        if let Some(ref mut window) = self.window {
            window.close();
        }
    }

    /// Check if the application is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the exit code.
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    /// Get a reference to the window.
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    /// Get a mutable reference to the window.
    pub fn window_mut(&mut self) -> Option<&mut Window> {
        self.window.as_mut()
    }
}
