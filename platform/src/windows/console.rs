// ============================================================================
// uix-platform/src/windows/console.rs — Windows console implementation (IConsole)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::types::{ConsoleColor, TerminalCapabilities};
use crate::IConsole;
use std::ptr;

use crate::windows::util::to_wide;

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
pub struct WindowsConsole {
    stdout: RawHandle,
}

type RawHandle = *mut std::ffi::c_void;

impl WindowsConsole {
    pub fn new() -> Self {
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
    fn write(&mut self, text: &str) {
        unsafe {
            let bytes = text.as_bytes();
            let mut written: u32 = 0;
            // Use WriteConsoleA for proper console output
            WriteConsoleA(
                self.stdout,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len() as u32,
                &mut written,
                ptr::null_mut(),
            );
        }
    }

    fn write_line(&mut self, text: &str) {
        let mut line = text.to_string();
        line.push('\n');
        self.write(&line);
    }

    fn set_color(&mut self, color: ConsoleColor) {
        let attr = COLORS[color as usize];
        unsafe {
            SetConsoleTextAttribute(self.stdout, attr);
        }
    }

    fn reset_color(&mut self) {
        unsafe {
            SetConsoleTextAttribute(self.stdout, COLORS[0]);
        }
    }

    fn show_terminal_cursor(&mut self, visible: bool) {
        unsafe {
            let mut info = CONSOLE_CURSOR_INFO {
                dwSize: 25,
                bVisible: if visible { 1 } else { 0 },
            };
            SetConsoleCursorInfo(self.stdout, &mut info);
        }
    }

    fn set_terminal_title(&mut self, title: &str) {
        let wide = to_wide(title);
        unsafe {
            SetConsoleTitleW(wide.as_ptr());
        }
    }

    fn capabilities(&self) -> TerminalCapabilities {
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
        // Reset colors on drop
        self.reset_color();
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
extern "system" {
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
