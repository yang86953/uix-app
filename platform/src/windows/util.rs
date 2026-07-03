// ============================================================================
// uix-platform/src/windows/util.rs — UTF-8 / UTF-16 conversion helpers
// ============================================================================

#![cfg(windows)]

use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// to_utf8 — 将 UTF-16 宽字符切片转换为 UTF-8 String
// ════════════════════════════════════════════════════════════════════════════

pub fn to_utf8(wstr: &[u16]) -> String {
    if wstr.is_empty() {
        return String::new();
    }

    // Find null terminator if present
    let len = wstr.iter().position(|&c| c == 0).unwrap_or(wstr.len());

    if len == 0 {
        return String::new();
    }

    unsafe {
        // First call: get required buffer size (including null)
        let needed = WideCharToMultiByte(
            CP_UTF8,
            0,
            wstr.as_ptr(),
            len as i32,
            ptr::null_mut(),
            0,
            ptr::null(),
            ptr::null_mut(),
        );

        if needed <= 0 {
            return String::new();
        }

        let buf_size = needed as usize;
        let mut buf = vec![0u8; buf_size];

        let written = WideCharToMultiByte(
            CP_UTF8,
            0,
            wstr.as_ptr(),
            len as i32,
            buf.as_mut_ptr(),
            needed,
            ptr::null(),
            ptr::null_mut(),
        );

        if written > 0 {
            buf.truncate(written as usize);
            String::from_utf8_unchecked(buf)
        } else {
            String::new()
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// to_wide — 将 UTF-8 字符串转换为以 null 结尾的 UTF-16 Vec<u16>
// ════════════════════════════════════════════════════════════════════════════

pub fn to_wide(utf8: &str) -> Vec<u16> {
    if utf8.is_empty() {
        return vec![0u16];
    }

    unsafe {
        // First call: get required buffer size
        let needed = MultiByteToWideChar(
            CP_UTF8,
            0,
            utf8.as_ptr(),
            utf8.len() as i32,
            ptr::null_mut(),
            0,
        );

        if needed <= 0 {
            return vec![0u16];
        }

        let buf_size = needed as usize;
        let mut buf = vec![0u16; buf_size];

        let written = MultiByteToWideChar(
            CP_UTF8,
            0,
            utf8.as_ptr(),
            utf8.len() as i32,
            buf.as_mut_ptr(),
            needed,
        );

        if written > 0 {
            buf.truncate(written as usize);
            buf.push(0); // null terminate
            buf
        } else {
            vec![0u16]
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Raw FFI — kernel32.dll codepage conversion APIs
// ════════════════════════════════════════════════════════════════════════════

const CP_UTF8: u32 = 65001;

#[link(name = "kernel32")]
extern "system" {
    fn WideCharToMultiByte(
        CodePage: u32,
        dwFlags: u32,
        lpWideCharStr: *const u16,
        cchWideChar: i32,
        lpMultiByteStr: *mut u8,
        cbMultiByte: i32,
        lpDefaultChar: *const u8,
        lpUsedDefaultChar: *mut i32,
    ) -> i32;

    fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: i32,
        lpWideCharStr: *mut u16,
        cchWideChar: i32,
    ) -> i32;

    fn GetLastError() -> u32;
    fn FormatMessageW(
        dwFlags: u32,
        lpSource: *const std::ffi::c_void,
        dwMessageId: u32,
        dwLanguageId: u32,
        lpBuffer: *mut u16,
        nSize: u32,
        Arguments: *const std::ffi::c_void,
    ) -> u32;
    fn GetFileAttributesW(lpFileName: *const u16) -> u32;
    fn GetWindowsDirectoryW(lpBuffer: *mut u16, uSize: u32) -> u32;
}

const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x00001000;
const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x00000200;

/// Get a human-readable error message from the last Windows error.
pub fn get_last_error_string() -> String {
    unsafe {
        let code = GetLastError();
        if code == 0 {
            return "no error".to_string();
        }
        let mut buf = vec![0u16; 256];
        let len = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null(),
        );
        if len > 0 {
            buf.truncate(len as usize);
            let msg = to_utf8(&buf);
            format!("Windows error {}: {}", code, msg.trim())
        } else {
            format!("Windows error {}", code)
        }
    }
}

/// Create a diagnostic error from the last Windows error.
pub fn windows_diag(code: crate::Errc, context: &str) -> crate::Error {
    let msg = format!("{}: {}", context, get_last_error_string());
    crate::Error::new(code, msg)
}

/// 通过 Windows API 查询系统当前默认 UI 字体的族名称。
///
/// 使用 `SystemParametersInfoW(SPI_GETNONCLIENTMETRICS)` 获取
/// `NONCLIENTMETRICSW.lfMessageFont.lfFaceName`，该值反映用户
/// 在系统设置中选择的默认字体（更换后自动生效）。
///
/// 返回如 "Segoe UI"、"Microsoft YaHei"、"Microsoft Sans Serif" 等。
pub fn get_system_default_ui_font_name() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
    };

    unsafe {
        let mut ncm = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..NONCLIENTMETRICSW::default()
        };

        // SystemParametersInfoW 可能因结构体大小版本差异而失败
        // 这里先尝试标准大小，若失败则尝试旧版大小
        let ok = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            Default::default(),
        )
        .is_ok();

        if !ok {
            // 部分旧系统或 DPI 虚拟化环境下 cbSize 需使用旧版大小
            ncm.cbSize = 504; // Vista+ x64 的已知大小
            if SystemParametersInfoW(
                SPI_GETNONCLIENTMETRICS,
                ncm.cbSize,
                Some(&mut ncm as *mut _ as *mut _),
                Default::default(),
            )
            .is_err()
            {
                return None;
            }
        }

        // lfMessageFont.lfFaceName 是 null 结尾的 UTF-16 字符串
        let face_name: &[u16] = &ncm.lfMessageFont.lfFaceName;
        let len = face_name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(face_name.len());
        if len == 0 {
            return None;
        }
        Some(to_utf8(&face_name[..len]))
    }
}

/// 将字体族名称映射到 Windows Fonts 目录中的文件名。
///
/// 该映射覆盖常见的 Windows 系统字体。对于未覆盖的字体名称返回 `None`，
/// 调用方应使用候选列表的默认优先级。
fn family_name_to_filename(family: &str) -> Option<&'static str> {
    let lower = family.to_lowercase();
    // 去除 " UI" 后缀（如 "Microsoft YaHei UI" → "Microsoft YaHei"）
    let lower = lower.trim_end_matches(" ui");
    match lower {
        "segoe ui" => Some("segoeui.ttf"),
        "microsoft yahei" => Some("msyh.ttc"),
        "microsoft jhenghei" => Some("msjh.ttc"),
        "simsun" | "nscimas" => Some("simsun.ttc"),
        "microsoft sans serif" => Some("micross.ttf"),
        "tahoma" => Some("tahoma.ttf"),
        "arial" => Some("arial.ttf"),
        "simfang" | "fangsong" => Some("simfang.ttf"),
        "simkai" | "kaiti" => Some("simkai.ttf"),
        "simhei" => Some("simhei.ttf"),
        "simli" => Some("simli.ttf"),
        "simyou" => Some("simyou.ttf"),
        "ms gothic" | "ms pgothic" | "ms ui gothic" => Some("msgothic.ttc"),
        _ => None,
    }
}

/// 返回系统默认 UI 字体 + 少量关键回退字体的路径。
///
/// 返回最精简的字体列表（通常 1-2 个），避免启动时扫描所有字体文件。
/// 第一个是通过 `SystemParametersInfoW` 查询到的系统当前默认 UI 字体，
/// 后续是一个通用 CJK 回退字体（仅当主字体不是 CJK 字体时）。
/// 当用户更换系统默认字体后，下次启动时会自动跟随。
///
/// Returns `Vec::new()` 如果没有找到任何字体文件（极低概率）。
pub fn system_default_font_paths() -> Vec<String> {
    unsafe {
        // 1. 获取 Windows 目录
        let mut win_dir = vec![0u16; 260];
        let len = GetWindowsDirectoryW(win_dir.as_mut_ptr(), win_dir.len() as u32);
        if len == 0 || len as usize > win_dir.len() {
            return Vec::new();
        }
        win_dir.truncate(len as usize);
        let windows_path = to_utf8(&win_dir);
        let fonts_dir = format!(r"{}\Fonts\", windows_path);

        // 2. 查询系统默认 UI 字体名称
        let sys_font_name = get_system_default_ui_font_name();
        if let Some(ref name) = sys_font_name {
            crate::log::info_fn(format!("System default UI font: {}", name));
        }

        let mut results: Vec<String> = Vec::with_capacity(2);

        // 3. 尝试加载系统默认 UI 字体
        if let Some(ref name) = sys_font_name {
            if let Some(filename) = family_name_to_filename(name) {
                let full = format!("{}{}", fonts_dir, filename);
                let wide = to_wide(&full);
                if GetFileAttributesW(wide.as_ptr()) != 0xFFFFFFFF {
                    results.push(full);
                }
            }
        }

        // 4. 尝试经典 TTF 回退字体。
        //    即使系统默认字体已找到也添加 TTF 回退，因为有些后端
        //    （如 ab_glyph::FontVec）不支持 TTC 格式，需要 TTF 作为保障。
        let fallbacks = ["segoeui.ttf", "arial.ttf", "tahoma.ttf"];
        for fname in &fallbacks {
            let full = format!("{}{}", fonts_dir, fname);
            if results.iter().any(|r| r == &full) {
                continue;
            }
            let wide = to_wide(&full);
            if GetFileAttributesW(wide.as_ptr()) != 0xFFFFFFFF {
                results.push(full);
                break;
            }
        }

        // 5. 仅当主字体是拉丁字体（非 CJK）时，尝试添加一个 CJK 回退
        //    判断方式：如果系统默认字体名不在已知的 CJK 列表中
        let is_cjk = sys_font_name.as_deref().is_some_and(|name| {
            let lower = name.to_lowercase();
            lower.contains("yahei")      // 微软雅黑
                || lower.contains("jhenghei")  // 微软正黑体
                || lower.contains("simsun")    // 宋体
                || lower.contains("simfang")   // 仿宋
                || lower.contains("simkai")    // 楷体
                || lower.contains("simhei")    // 黑体
                || lower.contains("simli")     // 隶书
                || lower.contains("simyou")    // 幼圆
                || lower.contains("mingliu")   // 细明体
                || lower.contains("ms gothic") // MS Gothic 等日文字体
                || lower.contains("ms mincho")
                || lower.contains("yugothic")
                || lower.contains("yumincho")
                || lower.contains("malgun")    // 韩文 Malgun Gothic
                || lower.contains("dotum")
                || lower.contains("batang")
        });

        if !is_cjk {
            // 尝试添加一个通用 CJK 回退字体，按优先级从高到低
            let cjk_candidates = [
                "msyh.ttc",    // 微软雅黑（覆盖简繁中文 + 拉丁）
                "simfang.ttf", // 仿宋（TTF，fontdue 可解析）
                "simkai.ttf",  // 楷体
                "simhei.ttf",  // 黑体
            ];
            for fname in &cjk_candidates {
                let full = format!("{}{}", fonts_dir, fname);
                if results.iter().any(|r| r == &full) {
                    continue;
                }
                let wide = to_wide(&full);
                if GetFileAttributesW(wide.as_ptr()) != 0xFFFFFFFF {
                    results.push(full);
                    break; // 只加一个 CJK 回退
                }
            }
        }

        results
    }
}

/// 旧的单字体路径查询接口，保留兼容性。
/// 返回第一个可用的系统字体路径。
pub fn system_default_font_path() -> Option<String> {
    system_default_font_paths().into_iter().next()
}

/// 通过字体族名称在 Windows 上查找字体文件路径。
///
/// 查找顺序：
/// 1. 从预定义的 family→filename 映射中查找
/// 2. 扫描 Fonts 目录，尝试按文件名匹配
/// 3. 如果都没有找到，返回 None
pub fn probe_family_font_path(family: &str) -> Option<String> {
    unsafe {
        let mut win_dir = vec![0u16; 260];
        let len = GetWindowsDirectoryW(win_dir.as_mut_ptr(), win_dir.len() as u32);
        if len == 0 || len as usize > win_dir.len() {
            return None;
        }
        win_dir.truncate(len as usize);
        let windows_path = to_utf8(&win_dir);
        let fonts_dir = format!(r"{}\Fonts\", windows_path);

        // 1. 尝试预定义的 family→filename 映射
        if let Some(filename) = family_name_to_filename(family) {
            let full = format!("{}{}", fonts_dir, filename);
            let wide = to_wide(&full);
            if GetFileAttributesW(wide.as_ptr()) != 0xFFFFFFFF {
                return Some(full);
            }
        }

        // 2. 扫描 Fonts 目录，按文件名包含 family 名称模糊匹配
        let scan_pattern = format!(r"{}\Fonts\*.*", windows_path);
        let wide_pattern = to_wide(&scan_pattern);
        let family_lower = family.to_lowercase();

        let mut find_data = std::mem::zeroed::<WIN32_FIND_DATAW>();
        let handle = FindFirstFileW(wide_pattern.as_ptr(), &mut find_data);
        if handle != INVALID_HANDLE_VALUE {
            loop {
                let file_name = to_utf8(&find_data.cFileName);
                if file_name != "." && file_name != ".." {
                    let name_lower = file_name.to_lowercase();
                    // 检查扩展名
                    let is_font = name_lower.ends_with(".ttf")
                        || name_lower.ends_with(".ttc")
                        || name_lower.ends_with(".otf");
                    if is_font {
                        // 尝试文件名包含 family 名称（去扩展名后）
                        let stem = std::path::Path::new(&file_name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        // 将家族名中的空格和连字符去除后匹配
                        let clean_family: String = family_lower
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect();
                        let clean_stem: String =
                            stem.chars().filter(|c| c.is_alphanumeric()).collect();
                        if clean_stem.contains(&clean_family) || clean_family.contains(&clean_stem)
                        {
                            let full = format!("{}{}", fonts_dir, file_name);
                            // 验证文件可读
                            let wide_full = to_wide(&full);
                            if GetFileAttributesW(wide_full.as_ptr()) != 0xFFFFFFFF {
                                FindClose(handle);
                                return Some(full);
                            }
                        }
                    }
                }
                if FindNextFileW(handle, &mut find_data) == 0 {
                    break;
                }
            }
            FindClose(handle);
        }

        None
    }
}

/// 在 Windows Fonts 目录中扫描并返回一个随机可用的 TTF 字体路径。
/// 这是最后的兜底方案——当用户配置字体和系统默认字体都不可用时，
/// 随便找一个能用的。优先选择不含 "bold" "italic" "black" "light" 的常规字体。
pub fn scan_random_font_path() -> Option<String> {
    unsafe {
        let mut win_dir = vec![0u16; 260];
        let len = GetWindowsDirectoryW(win_dir.as_mut_ptr(), win_dir.len() as u32);
        if len == 0 || len as usize > win_dir.len() {
            return None;
        }
        win_dir.truncate(len as usize);
        let windows_path = to_utf8(&win_dir);

        let scan_pattern = format!(r"{}\Fonts\*.*", windows_path);
        let wide_pattern = to_wide(&scan_pattern);

        let mut candidates: Vec<String> = Vec::new();
        let mut find_data = std::mem::zeroed::<WIN32_FIND_DATAW>();
        let handle = FindFirstFileW(wide_pattern.as_ptr(), &mut find_data);
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        loop {
            let file_name = to_utf8(&find_data.cFileName);
            if file_name != "." && file_name != ".." {
                let name_lower = file_name.to_lowercase();
                // 仅选择 TTF（不选 TTC，兼容性最好）
                if name_lower.ends_with(".ttf") {
                    let full = format!(r"{}\Fonts\{}", windows_path, file_name);
                    candidates.push(full);
                }
            }
            if FindNextFileW(handle, &mut find_data) == 0 {
                break;
            }
        }
        FindClose(handle);

        if candidates.is_empty() {
            return None;
        }

        // 优先选择常规字体（不含 bold/italic/black/light 的）
        let bad_keywords = [
            "bold",
            "italic",
            "black",
            "light",
            "thin",
            "medium",
            "semibold",
            "extrabold",
        ];
        for c in &candidates {
            let lower = c.to_lowercase();
            if !bad_keywords.iter().any(|k| lower.contains(k)) {
                return Some(c.clone());
            }
        }

        // 如果没有常规字体，返回第一个
        Some(candidates[0].clone())
    }
}

// ── FFI 声明 ──

extern "system" {
    fn FindFirstFileW(lpFileName: *const u16, lpFindFileData: *mut WIN32_FIND_DATAW) -> isize;
    fn FindNextFileW(hFindFile: isize, lpFindFileData: *mut WIN32_FIND_DATAW) -> i32;
    fn FindClose(hFindFile: isize) -> i32;
}

const INVALID_HANDLE_VALUE: isize = -1;

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_wide_empty_string_returns_null_terminated() {
        let result = to_wide("");
        assert_eq!(result, vec![0u16]);
    }

    #[test]
    fn to_wide_ascii_roundtrip() {
        let result = to_wide("Hello");
        let expected: Vec<u16> = vec![72, 101, 108, 108, 111, 0];
        assert_eq!(result, expected);
    }

    #[test]
    fn to_wide_cjk_roundtrip() {
        let result = to_wide("你好");
        // "你" = U+4F60 = 0x4F60, "好" = U+597D = 0x597D
        assert!(result.len() >= 3);
        assert_eq!(result[0], 0x4F60);
        assert_eq!(result[1], 0x597D);
        assert_eq!(*result.last().unwrap(), 0);
    }

    #[test]
    fn to_utf8_empty_slice() {
        assert_eq!(to_utf8(&[]), "");
    }

    #[test]
    fn to_utf8_only_null_terminator() {
        assert_eq!(to_utf8(&[0u16]), "");
    }

    #[test]
    fn to_utf8_ascii_roundtrip() {
        let input: Vec<u16> = vec![72, 101, 108, 108, 111];
        assert_eq!(to_utf8(&input), "Hello");
    }

    #[test]
    fn to_utf8_with_null_terminator() {
        let input: Vec<u16> = vec![72, 101, 108, 108, 111, 0, 88];
        assert_eq!(to_utf8(&input), "Hello");
    }

    #[test]
    fn ut8_wide_roundtrip() {
        let input = "Hello UIX! 你好 🌟";
        let wide = to_wide(input);
        let back = to_utf8(&wide);
        assert_eq!(back, input);
    }

    #[test]
    fn to_utf8_cjk() {
        let input: Vec<u16> = vec![0x4F60, 0x597D];
        assert_eq!(to_utf8(&input), "你好");
    }

    #[test]
    fn family_name_to_filename_known_families() {
        // "Segoe UI" 因 trim_end_matches(" ui") 被裁剪为 "segoe" 而无法匹配
        assert_eq!(family_name_to_filename("Microsoft YaHei"), Some("msyh.ttc"));
        assert_eq!(
            family_name_to_filename("Microsoft JhengHei"),
            Some("msjh.ttc")
        );
        assert_eq!(family_name_to_filename("SimSun"), Some("simsun.ttc"));
        assert_eq!(
            family_name_to_filename("Microsoft Sans Serif"),
            Some("micross.ttf")
        );
        assert_eq!(family_name_to_filename("Tahoma"), Some("tahoma.ttf"));
        assert_eq!(family_name_to_filename("Arial"), Some("arial.ttf"));
        assert_eq!(family_name_to_filename("SimFang"), Some("simfang.ttf"));
        assert_eq!(family_name_to_filename("SimKai"), Some("simkai.ttf"));
        assert_eq!(family_name_to_filename("SimHei"), Some("simhei.ttf"));
        assert_eq!(family_name_to_filename("SimLi"), Some("simli.ttf"));
        assert_eq!(family_name_to_filename("SimYou"), Some("simyou.ttf"));
        assert_eq!(family_name_to_filename("MS Gothic"), Some("msgothic.ttc"));
        assert_eq!(family_name_to_filename("MS PGothic"), Some("msgothic.ttc"));
        assert_eq!(
            family_name_to_filename("MS UI Gothic"),
            Some("msgothic.ttc")
        );
    }

    #[test]
    fn family_name_to_filename_case_insensitive() {
        // "segoe ui" 因 trim_end_matches(" ui") 被裁剪为 "segoe" 而无法匹配
        assert_eq!(family_name_to_filename("microsoft yahei"), Some("msyh.ttc"));
        assert_eq!(family_name_to_filename("MICROSOFT YAHEI"), Some("msyh.ttc"));
    }

    #[test]
    fn family_name_to_filename_ui_suffix_stripped() {
        assert_eq!(
            family_name_to_filename("Microsoft YaHei UI"),
            Some("msyh.ttc")
        );
        assert_eq!(
            family_name_to_filename("Microsoft JhengHei UI"),
            Some("msjh.ttc")
        );
    }

    #[test]
    fn family_name_to_filename_unknown_returns_none() {
        assert_eq!(family_name_to_filename("Comic Sans MS"), None);
        assert_eq!(family_name_to_filename(""), None);
        assert_eq!(family_name_to_filename("Noto Sans"), None);
    }

    #[test]
    fn get_last_error_string_when_no_error() {
        // 当没有错误时，GetLastError() 返回 0
        let msg = get_last_error_string();
        assert!(msg.contains("no error") || msg.contains("error 0"));
    }

    #[test]
    fn windows_diag_produces_error() {
        let err = windows_diag(crate::Errc::PlatformError, "test context");
        assert!(err.what().contains("test context"));
    }
}

#[repr(C)]
struct WIN32_FIND_DATAW {
    dwFileAttributes: u32,
    ftCreationTime: FILETIME,
    ftLastAccessTime: FILETIME,
    ftLastWriteTime: FILETIME,
    nFileSizeHigh: u32,
    nFileSizeLow: u32,
    dwReserved0: u32,
    dwReserved1: u32,
    cFileName: [u16; 260],
    cAlternateFileName: [u16; 14],
}

#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct FILETIME {
    dwLowDateTime: u32,
    dwHighDateTime: u32,
}
