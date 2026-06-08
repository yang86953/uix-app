use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::app::di::Container;
use crate::app::window::Window;
use crate::app::cli::Cli;

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    GUI,
    CLI,
}

impl Default for AppMode { fn default() -> Self { Self::GUI } }

/// Application entry point — manages lifecycle, mode, DI, window, and CLI.
pub struct App {
    mode: AppMode,
    window: Option<Window>,
    cli: Option<Cli>,
    container: Container,
    title: String,
    width: i32,
    height: i32,
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
            title: "UIX App".to_string(),
            width: 1200,
            height: 800,
            running: Arc::new(AtomicBool::new(false)),
            exit_code: 0,
            initialized: false,
        }
    }
}

impl App {
    pub fn new() -> Self { Self::default() }

    /// Initialize the application. Must be called before run().
    pub fn init(&mut self) -> &mut Self {
        if self.initialized {
            log::warn!("App::init: already initialized");
            return self;
        }

        self.initialized = true;

        // Register core services into DI container
        log::info!("UIX App initialized");

        self
    }

    /// Set application mode.
    pub fn mode(&mut self, m: AppMode) -> &mut Self {
        self.mode = m;
        self
    }

    /// Get current mode.
    pub fn current_mode(&self) -> AppMode { self.mode }

    /// Create the main window (GUI mode).
    pub fn create_window(&mut self, title: &str, width: i32, height: i32) -> &mut Self {
        self.title = title.to_string();
        self.width = width;
        self.height = height;
        self.window = Some(Window::new(title, width, height));
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
    pub fn run(&mut self) -> i32 {
        if !self.initialized {
            log::warn!("App::run: not initialized, auto-initializing");
            self.init();
        }

        self.running.store(true, Ordering::SeqCst);

        match self.mode {
            AppMode::GUI => {
                let window = match self.window.as_mut() {
                    Some(w) => w,
                    None => {
                        log::error!("App::run: no window created for GUI mode");
                        return 1;
                    }
                };
                window.run();
            }
            AppMode::CLI => {
                if let Some(ref mut cli) = self.cli {
                    // Use default args for now; real implementation would parse std::env::args()
                    let args = std::env::args().collect::<Vec<_>>();
                    self.exit_code = cli.run(&args);
                } else {
                    log::info!("Running in CLI mode (no commands registered)");
                }
            }
        }

        self.running.store(false, Ordering::SeqCst);
        self.exit_code
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
    pub fn exit_code(&self) -> i32 { self.exit_code }

    /// Get a reference to the window.
    pub fn window(&self) -> Option<&Window> { self.window.as_ref() }

    /// Get a mutable reference to the window.
    pub fn window_mut(&mut self) -> Option<&mut Window> { self.window.as_mut() }
}
