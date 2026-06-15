#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_64bit: bool,
}

/// 查询系统默认字体路径（平台特定实现）。
/// - Linux: 用 fontconfig `fc-match` 查询系统当前配置的字体
/// - Windows: 用 `system_default_font_path()` 查询系统字体目录
/// - macOS: 暂无原生实现，返回 None
#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
pub fn probe_system_default_font() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub fn probe_system_default_font() -> Option<String> {
    crate::platform::windows::util::system_default_font_path()
}

#[cfg(target_os = "linux")]
pub fn probe_system_default_font() -> Option<String> {
    crate::platform::linux::probe_system_default_font()
}

pub trait ISystemInfo {
    fn os_info(&self) -> OsInfo;
    fn cpu_count(&self) -> u32;
    fn memory_info(&self) -> MemoryInfo;
    fn hostname(&self) -> String;
    fn username(&self) -> String;
    fn up_time(&self) -> u64;

    /// 返回系统当前默认字体文件的路径（如有）。
    ///
    /// 平台层使用原生 API 查询（Linux: fontconfig fc-match, Windows: DirectWrite, macOS: CoreText），
    /// 引擎层拿到路径后自行加载。返回 `None` 表示无法确定，引擎应 fallback 到目录扫描。
    fn default_font_path(&self) -> Option<String> {
        None
    }
}
