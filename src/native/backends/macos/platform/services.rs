use super::*;

use super::*;

struct MacosClipboard;

impl MacosClipboard {
    fn new() -> Self {
        Self::default()
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

struct MacosCursor {
    cursor: CursorType,
    position: crate::core::Point,
}

impl MacosCursor {
    fn new() -> Self {
        Self {
            cursor: CursorType::Arrow,
            position: crate::core::Point::zero(),
        }
    }
}

impl ICursor for MacosCursor {
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

struct MacosDisplay;

impl IDisplay for MacosDisplay {
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

    fn info(&self, index: i32) -> Result<DisplayInfo> {
        // SAFETY: screen_info copies the selected NSScreen frame and scale into
        // plain Rust values and falls back to the main screen for invalid indexes.
        let screen = unsafe { cocoa::screen_info(index.max(0) as usize) };
        Ok(DisplayInfo {
            bounds: screen.bounds,
            dpi_scale: screen.scale as f32,
            is_primary: true,
        })
    }
}

struct MacosFileDialog;

impl IFileDialog for MacosFileDialog {
    fn open(&mut self, _title: &str, _filters: &str) -> Result<Option<Vec<String>>> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosFileDialog::open: not implemented",
        ))
    }

    fn save(&mut self, _title: &str, _filters: &str) -> Result<Option<String>> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosFileDialog::save: not implemented",
        ))
    }

    fn open_folder(&mut self, _title: &str) -> Result<Option<String>> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosFileDialog::open_folder: not implemented",
        ))
    }
}

type MacosFileSystem = FileSystemCore<MacosSpecialDirs>;

#[derive(Debug, Clone, Default)]
struct MacosSpecialDirs;

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

struct MacosKeyboard {
    keys_down: HashSet<KeyCode>,
}

impl MacosKeyboard {
    fn new() -> Self {
        Self {
            keys_down: HashSet::new(),
        }
    }
}

impl IKeyboard for MacosKeyboard {
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

struct MacosTimer;

impl MacosTimer {
    fn new() -> Self {
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

struct MacosNotification;
