use super::*;

use super::*;


impl INotification for MacosNotification {
    fn show(&mut self, _title: &str, _message: &str) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosNotification::show: not implemented",
        ))
    }
}

struct MacosConsole;

impl IConsole for MacosConsole {
    fn write(&mut self, text: &str) -> Result<()> {
        use std::io::Write;
        std::io::stdout()
            .write_all(text.as_bytes())
            .map_err(|error| {
                Error::new(Errc::PlatformError, format!("MacosConsole::write: {error}"))
            })?;
        std::io::stdout().flush().map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("MacosConsole::write flush: {error}"),
            )
        })
    }

    fn write_line(&mut self, text: &str) -> Result<()> {
        use std::io::Write;
        let mut line = text.to_string();
        line.push('\n');
        std::io::stdout()
            .write_all(line.as_bytes())
            .map_err(|error| {
                Error::new(
                    Errc::PlatformError,
                    format!("MacosConsole::write_line: {error}"),
                )
            })?;
        std::io::stdout().flush().map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("MacosConsole::write_line flush: {error}"),
            )
        })
    }

    fn set_color(&mut self, _color: ConsoleColor) -> Result<()> {
        Ok(())
    }

    fn reset_color(&mut self) -> Result<()> {
        Ok(())
    }

    fn show_terminal_cursor(&mut self, _visible: bool) -> Result<()> {
        Ok(())
    }

    fn set_terminal_title(&mut self, title: &str) -> Result<()> {
        use std::io::Write;
        write!(std::io::stdout(), "\x1b]0;{title}\x07").map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("MacosConsole::set_terminal_title: {error}"),
            )
        })?;
        std::io::stdout().flush().map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("MacosConsole::set_terminal_title flush: {error}"),
            )
        })
    }

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities {
            has_color: true,
            has_raw_mode: false,
            has_cursor_control: false,
        }
    }
}

struct MacosSystemInfo;

impl ISystemInfo for MacosSystemInfo {
    fn os_info(&self) -> Result<OsInfo> {
        Ok(OsInfo {
            name: "macOS".to_string(),
            version: String::new(),
            build: String::new(),
            is_64bit: cfg!(target_pointer_width = "64"),
        })
    }

    fn cpu_count(&self) -> Result<u32> {
        std::thread::available_parallelism()
            .map(|count| count.get() as u32)
            .map_err(|err| {
                Error::new(
                    Errc::PlatformError,
                    format!("MacosSystemInfo::cpu_count: {err}"),
                )
            })
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        Ok(MemoryInfo {
            total_bytes: 512 * 1024 * 1024,
            available_bytes: 512 * 1024 * 1024,
            process_working_set: 0,
            process_private_bytes: 0,
        })
    }

    fn hostname(&self) -> Result<String> {
        std::env::var("HOSTNAME").map_err(|_| {
            Error::new(
                Errc::NotFound,
                "MacosSystemInfo::hostname: HOSTNAME is not set",
            )
        })
    }

    fn username(&self) -> Result<String> {
        std::env::var("USER")
            .map_err(|_| Error::new(Errc::NotFound, "MacosSystemInfo::username: USER is not set"))
    }

    fn up_time(&self) -> Result<u64> {
        Err(Error::new(
            Errc::NotImplemented,
            "MacosSystemInfo::up_time: not implemented",
        ))
    }

    fn default_font_paths(&self) -> Result<Vec<String>> {
        Ok(vec![
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf".to_string(),
            "/System/Library/Fonts/Helvetica.ttc".to_string(),
        ])
    }

    fn probe_cjk_font_path(&self) -> Option<String> {
        find_existing_path(&[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ])
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        match family.to_ascii_lowercase().as_str() {
            "helvetica" => find_existing_path(&["/System/Library/Fonts/Helvetica.ttc"]),
            "pingfang" | "pingfang sc" => {
                find_existing_path(&["/System/Library/Fonts/PingFang.ttc"])
            }
            _ => None,
        }
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        find_existing_path(&[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ])
    }
}

