// ============================================================================
// platform/linux/console.rs — Linux ANSI terminal implementation (IConsole)
// ============================================================================
//
// Uses ANSI escape codes for color output. Stubs raw-mode operations since
// the platform abstraction does not require full terminal raw mode at this
// level — it is handled by higher-level interactive shells when needed.
// ============================================================================

use crate::native::traits::system::{ConsoleColor, TerminalCapabilities};
use crate::native::IConsole;

// ════════════════════════════════════════════════════════════════════════════
// ANSI color values
// ════════════════════════════════════════════════════════════════════════════

const COLORS: [&str; 7] = [
    "\x1b[0m",  // Default -> reset
    "\x1b[90m", // Trace   -> bright black (gray)
    "\x1b[0m",  // Debug   -> default
    "\x1b[32m", // Info    -> green
    "\x1b[33m", // Warn    -> yellow
    "\x1b[31m", // Error   -> red
    "\x1b[91m", // Fatal   -> bright red
];

// ════════════════════════════════════════════════════════════════════════════
// LinuxConsole
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct LinuxConsole;

impl LinuxConsole {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl IConsole for LinuxConsole {
    fn write(&mut self, text: &str) {
        use std::io::Write;
        let _ = std::io::stdout().write_all(text.as_bytes());
        let _ = std::io::stdout().flush();
    }

    fn write_line(&mut self, text: &str) {
        use std::io::Write;
        let mut line = text.to_string();
        line.push('\n');
        let _ = std::io::stdout().write_all(line.as_bytes());
        let _ = std::io::stdout().flush();
    }

    fn set_color(&mut self, color: ConsoleColor) {
        use std::io::Write;
        let _ = std::io::stdout().write_all(COLORS[color as usize].as_bytes());
        let _ = std::io::stdout().flush();
    }

    fn reset_color(&mut self) {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"\x1b[0m");
        let _ = std::io::stdout().flush();
    }

    fn show_terminal_cursor(&mut self, visible: bool) {
        use std::io::Write;
        let seq = if visible { b"\x1b[?25h" } else { b"\x1b[?25l" };
        let _ = std::io::stdout().write_all(seq);
        let _ = std::io::stdout().flush();
    }

    fn set_terminal_title(&mut self, title: &str) {
        use std::io::Write;
        let _ = write!(std::io::stdout(), "\x1b]0;{}\x07", title);
        let _ = std::io::stdout().flush();
    }

    fn capabilities(&self) -> TerminalCapabilities {
        // Most modern Linux terminals support ANSI color and cursor control.
        TermEnv::probe()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Terminal capability probing
// ════════════════════════════════════════════════════════════════════════════

struct TermEnv;

impl TermEnv {
    fn probe() -> TerminalCapabilities {
        let term = std::env::var("TERM").unwrap_or_default();
        let has_color = Self::check_color(&term);
        TerminalCapabilities {
            has_color,
            has_raw_mode: false,      // raw mode handled at a higher layer
            has_cursor_control: true, // most modern terminals support this
        }
    }

    fn check_color(term: &str) -> bool {
        // Common color-capable terminal patterns
        matches!(
            term,
            "xterm"
                | "xterm-256color"
                | "xterm-color"
                | "gnome-terminal"
                | "gnome"
                | "linux"
                | "screen"
                | "screen-256color"
                | "tmux"
                | "tmux-256color"
                | "alacritty"
                | "kitty"
                | "wezterm"
                | "foot"
                | "st"
                | "st-256color"
                | "rxvt-unicode"
                | "rxvt-unicode-256color"
                | "konsole"
                | "konsole-256color"
                | "terminator"
        ) || term.contains("256color")
            || term.contains("color")
    }
}
