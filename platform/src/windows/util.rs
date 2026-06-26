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
pub fn windows_diag(code: uix_diag::Errc, context: &str) -> uix_diag::Error {
    let msg = format!("{}: {}", context, get_last_error_string());
    uix_diag::Error::new(code, msg)
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
        let mut ncm = NONCLIENTMETRICSW::default();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;

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
            uix_diag::log::info_fn(format!("System default UI font: {}", name));
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

        // 4. 如果系统默认字体未找到，尝试经典回退字体
        if results.is_empty() {
            let fallbacks = ["segoeui.ttf", "arial.ttf", "tahoma.ttf"];
            for fname in &fallbacks {
                let full = format!("{}{}", fonts_dir, fname);
                let wide = to_wide(&full);
                if GetFileAttributesW(wide.as_ptr()) != 0xFFFFFFFF {
                    results.push(full);
                    break;
                }
            }
        }

        // 5. 仅当主字体是拉丁字体（非 CJK）时，尝试添加一个 CJK 回退
        //    判断方式：如果系统默认字体名不在已知的 CJK 列表中
        let is_cjk = sys_font_name.as_deref().map_or(false, |name| {
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
