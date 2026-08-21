// ============================================================================
// native/backends/windows/console.rs — Windows console implementation (IConsole)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::core::error::Errc;
use crate::core::error::{Error, Result};
use crate::platform::system::console::{ConsoleColor, IConsole, TerminalCapabilities};
use std::ptr;

use crate::native::backends::windows::util::to_wide;

// ════════════════════════════════════════════════════════════════════════════
// 控制台颜色值映射 (Windows console attribute values)
// ════════════════════════════════════════════════════════════════════════════

const COLORS: [u16; 7] = [
    0x0007, // Default -> 灰色 (FOREGROUND_R|G|B)
    0x0008, // Trace   -> 深灰
    0x0007, // Debug   -> 灰色
    0x000A, // Info    -> 绿色
    0x0006, // Warn    -> 黄色
    0x000C, // Error   -> 红色
    0x0004, // Fatal   -> 深红
];

// ════════════════════════════════════════════════════════════════════════════
// 控制台句柄
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
// Windows 控制台后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsConsole {
    stdout: RawHandle,
}

type RawHandle = *mut std::ffi::c_void;

impl WindowsConsole {
    // 捕获当前进程标准输出句柄。
    pub(crate) fn new() -> Self {
        // SAFETY: GetStdHandle 无指针输入，返回标准输出句柄或 INVALID_HANDLE_VALUE；失败由后续控制台调用以返回码 0 呈现。
        Self {
            stdout: unsafe { GetStdHandle(STD_OUTPUT_HANDLE) },
        }
    }
}

impl Default for WindowsConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl IConsole for WindowsConsole {
    fn write(&mut self, text: &str) -> Result<()> {
        // SAFETY: self.stdout 为本实例取得的句柄；bytes 切片在同步调用期间存活且长度可转为 u32；written 为栈上可写输出。
        unsafe {
            let bytes = text.as_bytes();
            let mut written: u32 = 0;
            if WriteConsoleA(
                self.stdout,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len() as u32,
                &mut written,
                ptr::null_mut(),
            ) == 0
            {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WindowsConsole::write: WriteConsoleA failed",
                ));
            }
        }
        Ok(())
    }

    fn write_line(&mut self, text: &str) -> Result<()> {
        let mut line = text.to_string();
        line.push('\n');
        self.write(&line)
    }

    fn set_color(&mut self, color: ConsoleColor) -> Result<()> {
        let attr = COLORS[color as usize];
        // SAFETY: self.stdout 为本实例取得的句柄；attr 为合法颜色属性值。
        unsafe {
            if SetConsoleTextAttribute(self.stdout, attr) == 0 {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WindowsConsole::set_color: SetConsoleTextAttribute failed",
                ));
            }
        }
        Ok(())
    }

    fn reset_color(&mut self) -> Result<()> {
        // SAFETY: self.stdout 为本实例取得的句柄；COLORS[0] 为默认颜色属性值。
        unsafe {
            if SetConsoleTextAttribute(self.stdout, COLORS[0]) == 0 {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WindowsConsole::reset_color: SetConsoleTextAttribute failed",
                ));
            }
        }
        Ok(())
    }

    fn show_terminal_cursor(&mut self, visible: bool) -> Result<()> {
        // SAFETY: self.stdout 为本实例取得的句柄；info 为完整初始化的可写结构。
        unsafe {
            let mut info = CONSOLE_CURSOR_INFO {
                dwSize: 25,
                bVisible: if visible { 1 } else { 0 },
            };
            if SetConsoleCursorInfo(self.stdout, &mut info) == 0 {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WindowsConsole::show_terminal_cursor: SetConsoleCursorInfo failed",
                ));
            }
        }
        Ok(())
    }

    fn set_terminal_title(&mut self, title: &str) -> Result<()> {
        let wide = to_wide(title);
        // SAFETY: wide 为存活且 NUL 结尾的 UTF-16 缓冲。
        unsafe {
            if SetConsoleTitleW(wide.as_ptr()) == 0 {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WindowsConsole::set_terminal_title: SetConsoleTitleW failed",
                ));
            }
        }
        Ok(())
    }

    fn capabilities(&self) -> TerminalCapabilities {
        // SAFETY: self.stdout 为本实例取得的句柄；info 为完整初始化的可写结构，GetConsoleScreenBufferInfo 只写入它。
        unsafe {
            let mut info = CONSOLE_SCREEN_BUFFER_INFO {
                dwSize: COORD { X: 0, Y: 0 },
                dwCursorPosition: COORD { X: 0, Y: 0 },
                wAttributes: 0,
                srWindow: SMALL_RECT {
                    Left: 0,
                    Top: 0,
                    Right: 0,
                    Bottom: 0,
                },
                dwMaximumWindowSize: COORD { X: 0, Y: 0 },
            };
            let has_console = GetConsoleScreenBufferInfo(self.stdout, &mut info) != 0;
            TerminalCapabilities {
                has_color: has_console,
                has_raw_mode: false,
                has_cursor_control: has_console,
            }
        }
    }
}

impl Drop for WindowsConsole {
    fn drop(&mut self) {
        // Reset colors on drop — best effort, ignore failure.
        let _ = self.reset_color();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Raw FFI — kernel32.dll console API
// ════════════════════════════════════════════════════════════════════════════

const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct COORD {
    X: i16,
    Y: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SMALL_RECT {
    Left: i16,
    Top: i16,
    Right: i16,
    Bottom: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CONSOLE_SCREEN_BUFFER_INFO {
    dwSize: COORD,
    dwCursorPosition: COORD,
    wAttributes: u16,
    srWindow: SMALL_RECT,
    dwMaximumWindowSize: COORD,
}

#[repr(C)]
struct CONSOLE_CURSOR_INFO {
    dwSize: u32,
    bVisible: i32,
}

#[link(name = "kernel32")]
// SAFETY: 声明对应 kernel32 控制台 ABI，调用方负责句柄、缓冲区长度、结构尺寸和 NUL 结尾标题契约。
unsafe extern "system" {
    fn GetStdHandle(nStdHandle: u32) -> RawHandle;
    fn SetConsoleTextAttribute(hConsoleOutput: RawHandle, wAttributes: u16) -> i32;
    fn WriteConsoleA(
        hConsoleOutput: RawHandle,
        lpBuffer: *const std::ffi::c_void,
        nNumberOfCharsToWrite: u32,
        lpNumberOfCharsWritten: *mut u32,
        lpReserved: *mut std::ffi::c_void,
    ) -> i32;
    fn GetConsoleScreenBufferInfo(
        hConsoleOutput: RawHandle,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> i32;
    fn SetConsoleCursorInfo(
        hConsoleOutput: RawHandle,
        lpConsoleCursorInfo: *mut CONSOLE_CURSOR_INFO,
    ) -> i32;
    fn SetConsoleTitleW(lpConsoleTitle: *const u16) -> i32;
}
