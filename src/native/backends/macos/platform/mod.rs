use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use crate::core::{Errc, Error, Rect, Result, WindowId};
use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
use crate::native::capabilities::display::{DisplayInfo, IDisplay};
use crate::native::capabilities::services::{FileSystemCore, SpecialDirProvider};
use crate::native::capabilities::system::{
    ConsoleColor, IConsole, IFileDialog, IFileSystem, INotification, ISystemInfo, ITimer,
    MemoryInfo, OsInfo, SpecialDir, TerminalCapabilities,
};
use crate::native::platform::Platform;
use crate::native::present::{IPresenter, PresentDamage, validate_pixel_buffer};
use crate::native::windowing::event::{EventBus, EventLoopWaker, FrameRequestToken, UiEvent};
use crate::native::windowing::input::{
    CursorType, IClipboard, ICursor, IKeyboard, ITextInput, KeyCode, KeyMod, MouseButton,
};
use crate::native::windowing::shared::{OsEventSource, PlatformWindowCore, WindowOps, WindowState};
use crate::native::windowing::window::{
    IWindowManager, NativeFrameRequest, PlatformWindow, WindowOcclusionState,
};

use super::display_link::MacosFramePacer;
use super::text_input_view::{self, MacosTextInput};
use super::window_delegate::{self, WindowDelegateContext};

pub struct MacosPlatform {
    pending_failures: PendingFailureSource,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    event_bus: EventBus,
    clipboard: MacosClipboard,
    cursor: MacosCursor,
    display: MacosDisplay,
    file_dialog: MacosFileDialog,
    file_system: MacosFileSystem,
    keyboard: MacosKeyboard,
    text_input: MacosTextInput,
    timer: MacosTimer,
    notification: MacosNotification,
    console: MacosConsole,
    system_info: MacosSystemInfo,
    next_window_id: u64,
}

impl MacosPlatform {
    pub fn new(pending_failures: PendingFailureQueue) -> Self {
        let pending_failures = pending_failures.source();
        Self {
            pending_failures,
            events: Arc::new(Mutex::new(VecDeque::new())),
            event_bus: EventBus::new(),
            clipboard: MacosClipboard::new(),
            cursor: MacosCursor::new(),
            display: MacosDisplay,
            file_dialog: MacosFileDialog,
            file_system: MacosFileSystem::new(),
            keyboard: MacosKeyboard::new(),
            text_input: MacosTextInput::new(),
            timer: MacosTimer::new(),
            notification: MacosNotification,
            console: MacosConsole,
            system_info: MacosSystemInfo,
            next_window_id: 1,
        }
    }
}

impl Default for MacosPlatform {
    fn default() -> Self {
        Self::new(PendingFailureQueue::new())
    }
}

impl Drop for MacosPlatform {
    fn drop(&mut self) {
        self.pending_failures.close();
    }
}

impl OsEventSource for MacosPlatform {
    fn dispatch_pending(&mut self) -> bool {
        // SAFETY: Cocoa event dispatch stays on the caller's UI thread and uses
        // AppKit-owned singleton objects; no Rust references cross the FFI boundary.
        unsafe { while self.dispatch_cocoa_event(cocoa::distant_past()) {} }
        true
    }

    fn dispatch_blocking(&mut self) -> bool {
        // SAFETY: See dispatch_pending; distantFuture is an autoreleased NSDate
        // owned by Foundation and valid for the duration of this message send.
        unsafe {
            let _ = self.dispatch_cocoa_event(cocoa::distant_future());
        }
        true
    }

    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        // SAFETY: See dispatch_pending; the NSDate is only used synchronously by
        // nextEventMatchingMask and is not retained by Rust.
        unsafe {
            let until = cocoa::date_with_time_interval(timeout.as_secs_f64());
            let _ = self.dispatch_cocoa_event(until);
        }
        true
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        let event = self
            .events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front();
        if event.as_ref().is_some_and(|event| {
            matches!(
                event.type_,
                crate::native::windowing::event::UiEventType::WindowBlur
                    | crate::native::windowing::event::UiEventType::WindowClose
            )
        }) {
            self.keyboard.keys_down.clear();
        }
        event
    }

    fn waker(&self) -> EventLoopWaker {
        EventLoopWaker::new(|| {
            // SAFETY: CFRunLoopGetMain returns the process main run loop and
            // CFRunLoopWakeUp is safe to call from any thread to unblock
            // nextEventMatchingMask waits on the UI thread.
            unsafe {
                cocoa::wake_main_run_loop();
            }
        })
    }
}

impl MacosPlatform {
    unsafe fn dispatch_cocoa_event(&mut self, until: cocoa::Id) -> bool {
        let Some(event) = cocoa::dispatch_one_event(until) else {
            return false;
        };
        let suppress_keydown_text = self.text_input.suppress_keydown_text(event.window_id);
        for ui_event in event.into_ui_events(suppress_keydown_text) {
            match ui_event.type_ {
                crate::native::windowing::event::UiEventType::KeyDown => {
                    if let crate::native::windowing::event::UiEventPayload::Key(data) =
                        &ui_event.payload
                    {
                        self.keyboard.keys_down.insert(data.key);
                    }
                }
                crate::native::windowing::event::UiEventType::KeyUp => {
                    if let crate::native::windowing::event::UiEventPayload::Key(data) =
                        &ui_event.payload
                    {
                        self.keyboard.keys_down.remove(&data.key);
                    }
                }
                _ => {}
            }
            self.events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push_back(ui_event);
        }
        true
    }
}

impl IWindowManager for MacosPlatform {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> crate::core::Result<Box<dyn PlatformWindow>, Error> {
        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        // SAFETY: create_window constructs AppKit objects on the current thread
        // and returns Objective-C object pointers managed by AppKit.
        let text_input_owner = self.text_input.owner_handle();
        let (window, content_layer) = unsafe {
            cocoa::create_window(
                title,
                width,
                height,
                Arc::clone(&self.events),
                window_id,
                Arc::clone(&text_input_owner),
                self.pending_failures.clone(),
            )
        }?;
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            window_id, width, height,
        )));
        let open = Rc::new(Cell::new(true));
        // SAFETY: the NSWindow owns the installed Objective-C delegate; its raw
        // ivar owns the Rust context until delegate dealloc.
        if let Err(error) = unsafe {
            window_delegate::install(
                window,
                WindowDelegateContext {
                    events: Arc::clone(&self.events),
                    window_id,
                    state: Rc::clone(&state),
                    open: Rc::clone(&open),
                    pending_failures: self.pending_failures.clone(),
                },
            )
        } {
            // SAFETY: create_window returned the still-owned +1 NSWindow. No
            // delegate was published, so close/release is the complete rollback.
            unsafe {
                cocoa::close_and_release_window(window);
            }
            return Err(error);
        }
        let ops = MacosWindowOps::new(
            window,
            content_layer.layer,
            window_id,
            Arc::clone(&self.events),
            text_input_owner,
            open,
            self.pending_failures.clone(),
        );
        let presenter = MacosPresenter::new(content_layer.layer, width, height);
        let core = PlatformWindowCore::new(state, ops, Box::new(presenter));
        Ok(Box::new(core))
    }
}

impl Platform for MacosPlatform {
    fn take_pending_failure(&mut self) -> Option<Error> {
        self.pending_failures.take()
    }

    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        self
    }

    fn event_loop(&mut self) -> &mut dyn crate::native::windowing::event::IEventLoop {
        self
    }

    fn event_bus(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.clipboard
    }

    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.cursor
    }

    fn display(&self) -> &dyn IDisplay {
        &self.display
    }

    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog
    }

    fn keyboard(&self) -> &dyn IKeyboard {
        &self.keyboard
    }

    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.text_input
    }

    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer
    }

    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification
    }

    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console
    }

    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system
    }

    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info
    }
}

fn home_dir() -> Option<String> {
    std::env::var("HOME").ok()
}

fn home_child(child: &str) -> Result<String> {
    let home = home_dir()
        .ok_or_else(|| Error::new(Errc::NotFound, "MacosSpecialDirs: HOME is not set"))?;
    Ok(PathBuf::from(home)
        .join(child)
        .to_string_lossy()
        .to_string())
}

fn find_existing_path(paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| (*path).to_string())
}

/// Update MoltenVK / Metal drawable extent before Vulkan swapchain recreate.
pub(crate) unsafe fn set_metal_layer_drawable_size(layer: cocoa::Id, width: i32, height: i32) {
    cocoa::set_metal_layer_drawable_size(layer, width, height);
}

mod app_event;
mod cocoa;
mod presenter;
mod services;
mod services2;
mod window_ops;
