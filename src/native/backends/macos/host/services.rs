// 导入 macOS Platform System 根模块统一拥有的契约与辅助类型。
use super::*;
use crate::platform::system::ITimer;
use crate::platform::system::filesystem::{FileSystemCore, SpecialDir, SpecialDirProvider};

// 剪贴板组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosClipboard;

impl MacosClipboard {
    // 由平台根对象构造剪贴板组件。
    pub(super) fn new() -> Self {
        // 无状态剪贴板组件直接构造单元结构体。
        Self
    }
}

impl IClipboard for MacosClipboard {
    fn text(&self) -> Result<String> {
        // SAFETY: NSPasteboard is an AppKit singleton; returned NSString data is
        // copied into a Rust String before leaving the FFI boundary.
        Ok(unsafe { cocoa::clipboard_text() })
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        // SAFETY: text is converted to NSString and consumed synchronously by
        // NSPasteboard's setter; Rust does not retain Objective-C pointers.
        unsafe {
            cocoa::set_clipboard_text(text);
        }
        Ok(())
    }

    fn has_text(&self) -> Result<bool> {
        // SAFETY: Same invariant as text(); this only checks whether the
        // pasteboard currently has a string payload.
        Ok(unsafe { cocoa::clipboard_has_text() })
    }
}

// 光标组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosCursor {
    cursor: CursorType,
    position: crate::core::Point,
}

impl MacosCursor {
    // 由平台根对象构造光标组件。
    pub(super) fn new() -> Self {
        Self {
            cursor: CursorType::Arrow,
            position: crate::core::Point::zero(),
        }
    }
}

impl crate::platform::windowing::ICursor for MacosCursor {
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
        self.cursor = cursor;
        Ok(())
    }

    fn show_cursor(&mut self, _visible: bool) -> Result<()> {
        Ok(())
    }

    fn cursor_position(&self) -> Result<crate::core::Point> {
        Ok(self.position)
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) -> Result<()> {
        self.position = crate::core::Point::new(x as f32, y as f32);
        Ok(())
    }

    fn confine_cursor(&mut self, _confine: bool) -> Result<()> {
        Ok(())
    }

    fn capture_mouse(&mut self) -> Result<()> {
        Ok(())
    }

    fn release_mouse(&mut self) -> Result<()> {
        Ok(())
    }
}

// 显示组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosDisplay;

impl crate::platform::display::IDisplay for MacosDisplay {
    fn dpi_scale(&self) -> Result<f32> {
        // SAFETY: NSScreen returns AppKit-owned objects and scalar values; Rust
        // copies the scale factor immediately.
        Ok(unsafe { cocoa::main_screen_scale() as f32 })
    }

    fn is_dark_mode(&self) -> Result<bool> {
        // SAFETY: NSUserDefaults returns an autoreleased NSString that is copied
        // into Rust before comparison.
        Ok(unsafe { cocoa::is_dark_mode() })
    }

    fn count(&self) -> Result<i32> {
        // SAFETY: NSScreen screens is an AppKit-owned NSArray; only its count is read.
        Ok(unsafe { cocoa::screen_count() as i32 })
    }

    fn info(&self, index: i32) -> Result<crate::platform::display::DisplayInfo> {
        // SAFETY: screen_info copies the selected NSScreen frame and scale into
        // plain Rust values and falls back to the main screen for invalid indexes.
        let screen = unsafe { cocoa::screen_info(index.max(0) as usize) };
        Ok(crate::platform::display::DisplayInfo {
            bounds: screen.bounds,
            dpi_scale: screen.scale as f32,
            is_primary: true,
        })
    }
}

// 文件系统适配器仅对 macOS Platform System 内部可见。
pub(super) type MacosFileSystem = FileSystemCore<MacosSpecialDirs>;

#[derive(Debug, Clone, Default)]
// 特殊目录策略随文件系统适配器在 macOS Platform System 内部共享。
pub(super) struct MacosSpecialDirs;

impl SpecialDirProvider for MacosSpecialDirs {
    fn special_dir(&self, dir: SpecialDir) -> Result<String> {
        match dir {
            SpecialDir::Home => home_dir()
                .ok_or_else(|| Error::new(Errc::NotFound, "MacosSpecialDirs: HOME is not set")),
            SpecialDir::Temp => Ok(std::env::temp_dir().to_string_lossy().to_string()),
            SpecialDir::AppData | SpecialDir::LocalAppData => {
                home_child("Library/Application Support")
            }
            SpecialDir::Documents => home_child("Documents"),
            SpecialDir::Desktop => home_child("Desktop"),
            SpecialDir::Downloads => home_child("Downloads"),
            SpecialDir::Current | SpecialDir::Executable => Err(Error::new(
                Errc::NotImplemented,
                "MacosSpecialDirs: Current/Executable are handled by FileSystemCore",
            )),
        }
    }
}

// 键盘组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosKeyboard {
    // 平台事件循环维护当前按下键集合，查询能力只读该状态。
    pub(super) keys_down: HashSet<KeyCode>,
}

impl MacosKeyboard {
    // 由平台根对象构造键盘组件。
    pub(super) fn new() -> Self {
        Self {
            keys_down: HashSet::new(),
        }
    }
}

impl crate::platform::windowing::IKeyboard for MacosKeyboard {
    fn is_down(&self, key: KeyCode) -> bool {
        self.keys_down.contains(&key)
    }

    fn idle_ms(&self) -> u32 {
        0
    }

    fn double_click_ms(&self) -> u32 {
        500
    }
}

// 定时器组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosTimer;

impl MacosTimer {
    // 由平台根对象构造定时器组件。
    pub(super) fn new() -> Self {
        Self
    }
}

impl ITimer for MacosTimer {
    fn set(&mut self, _interval_ms: u32, _repeating: bool) -> Result<u32> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosTimer::set: not implemented",
        ))
    }

    fn clear(&mut self, _id: u32) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosTimer::clear: not implemented",
        ))
    }
}

// 通知组件在 services2 中实现契约，并由平台根对象持有。
pub(super) struct MacosNotification;
