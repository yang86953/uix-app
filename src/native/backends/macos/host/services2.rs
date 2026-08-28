// 导入 macOS Platform System 根模块统一拥有的契约与基础服务类型。
use super::*;
use crate::platform::system::info::{ISystemInfo, MemoryInfo, OsInfo};

// 系统信息组件仅对 macOS Platform System 内部可见。
pub(super) struct MacosSystemInfo;

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
