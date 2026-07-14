//! Windows backend utility functions.

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::native::{Errc, Error};
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn to_utf8(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

pub fn get_last_error_string() -> String {
    let code = unsafe { GetLastError() };
    windows_error_string(code)
}

pub fn windows_error_string(code: u32) -> String {
    if code == 0 {
        "no error".to_string()
    } else {
        format!("Windows error {}", code)
    }
}

pub fn windows_diag(code: Errc, context: &str) -> Error {
    Error::new(code, format!("{}: {}", context, get_last_error_string()))
}

pub fn windows_diag_for_code(code: Errc, context: &str, windows_code: u32) -> Error {
    Error::new(
        code,
        format!("{}: {}", context, windows_error_string(windows_code)),
    )
}

fn windows_fonts_dir() -> Option<String> {
    unsafe {
        let mut win_dir = vec![0u16; 260];
        let len = GetWindowsDirectoryW(win_dir.as_mut_ptr(), win_dir.len() as u32);
        if len == 0 || len as usize > win_dir.len() {
            return None;
        }
        win_dir.truncate(len as usize);
        Some(format!(r"{}\Fonts\", to_utf8(&win_dir)))
    }
}

fn file_exists(path: &str) -> bool {
    let wide = to_wide(path);
    unsafe { GetFileAttributesW(wide.as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

pub fn system_default_font_paths() -> Vec<String> {
    let Some(fonts_dir) = windows_fonts_dir() else {
        return Vec::new();
    };

    // 主字体候选（Latin UI）；CJK 走 probe_cjk_font_path，避免启动同步装一堆 fallback。
    let candidates = ["segoeui.ttf", "segoeuib.ttf", "arial.ttf", "tahoma.ttf"];

    let mut paths = Vec::new();
    for name in candidates {
        let full = format!("{}{}", fonts_dir, name);
        if file_exists(&full) && !paths.iter().any(|p| p == &full) {
            paths.push(full);
        }
    }

    if paths.is_empty() {
        if let Some(path) = scan_random_font_path() {
            paths.push(path);
        }
    }

    paths
}

/// 探测一枚可用的 CJK 字体路径（启动至多再装这一枚）。
pub fn probe_cjk_font_path() -> Option<String> {
    probe_cjk_font_paths().into_iter().next()
}

/// 有序 CJK 候选（存在的文件）；调用方依次尝试直至加载成功。
pub fn probe_cjk_font_paths() -> Vec<String> {
    let Some(fonts_dir) = windows_fonts_dir() else {
        return Vec::new();
    };
    let candidates = [
        "msyh.ttc",
        "msjh.ttc",
        "simsun.ttc",
        "simhei.ttf",
        "msgothic.ttc",
        "malgun.ttf",
    ];
    let mut paths = Vec::new();
    for name in candidates {
        let full = format!("{}{}", fonts_dir, name);
        if file_exists(&full) {
            paths.push(full);
        }
    }
    paths
}

pub fn probe_family_font_path(family: &str) -> Option<String> {
    let fonts_dir = windows_fonts_dir()?;

    if let Some(filename) = family_name_to_filename(family) {
        let full = format!("{}{}", fonts_dir, filename);
        if file_exists(&full) {
            return Some(full);
        }
    }

    let family_lower = family.to_lowercase();
    let clean_family: String = family_lower
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    if clean_family.is_empty() {
        return None;
    }

    scan_fonts_dir(&fonts_dir, |file_name| {
        let name_lower = file_name.to_lowercase();
        if !is_font_file(&name_lower) {
            return false;
        }
        let stem = std::path::Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let clean_stem: String = stem.chars().filter(|c| c.is_alphanumeric()).collect();
        clean_stem.contains(&clean_family) || clean_family.contains(&clean_stem)
    })
}

pub fn scan_random_font_path() -> Option<String> {
    let fonts_dir = windows_fonts_dir()?;
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

    let mut fallback = None;
    let preferred = scan_fonts_dir(&fonts_dir, |file_name| {
        let lower = file_name.to_lowercase();
        if !lower.ends_with(".ttf") {
            return false;
        }
        if fallback.is_none() {
            fallback = Some(format!("{}{}", fonts_dir, file_name));
        }
        !bad_keywords.iter().any(|k| lower.contains(k))
    });

    preferred.or(fallback)
}

fn scan_fonts_dir(fonts_dir: &str, mut accept: impl FnMut(&str) -> bool) -> Option<String> {
    unsafe {
        let scan_pattern = format!("{}*.*", fonts_dir);
        let wide_pattern = to_wide(&scan_pattern);
        let mut find_data = std::mem::zeroed::<WIN32_FIND_DATAW>();
        let handle = FindFirstFileW(wide_pattern.as_ptr(), &mut find_data);
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut result = None;
        loop {
            let file_name = to_utf8(&find_data.cFileName);
            if file_name != "." && file_name != ".." && accept(&file_name) {
                let full = format!("{}{}", fonts_dir, file_name);
                if file_exists(&full) {
                    result = Some(full);
                    break;
                }
            }

            if FindNextFileW(handle, &mut find_data) == 0 {
                break;
            }
        }
        FindClose(handle);
        result
    }
}

fn is_font_file(name_lower: &str) -> bool {
    name_lower.ends_with(".ttf") || name_lower.ends_with(".ttc") || name_lower.ends_with(".otf")
}

pub(crate) fn family_name_to_filename(family: &str) -> Option<&'static str> {
    let normalized = family
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let key = match normalized.as_str() {
        "microsoft yahei ui" => "microsoft yahei",
        "microsoft jhenghei ui" => "microsoft jhenghei",
        other => other,
    };

    match key {
        "microsoft yahei" => Some("msyh.ttc"),
        "microsoft jhenghei" => Some("msjh.ttc"),
        "simsun" | "simsun-extb" | "nsimsun" => Some("simsun.ttc"),
        "microsoft sans serif" => Some("micross.ttf"),
        "tahoma" => Some("tahoma.ttf"),
        "arial" => Some("arial.ttf"),
        "simfang" => Some("simfang.ttf"),
        "simkai" => Some("simkai.ttf"),
        "simhei" => Some("simhei.ttf"),
        "simli" => Some("simli.ttf"),
        "simyou" => Some("simyou.ttf"),
        "ms gothic" | "ms pgothic" | "ms ui gothic" => Some("msgothic.ttc"),
        "malgun gothic" => Some("malgun.ttf"),
        _ => None,
    }
}

const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;
const INVALID_HANDLE_VALUE: isize = -1;

#[repr(C)]
struct FILETIME {
    dwLowDateTime: u32,
    dwHighDateTime: u32,
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

#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
    fn GetWindowsDirectoryW(lpBuffer: *mut u16, uSize: u32) -> u32;
    fn GetFileAttributesW(lpFileName: *const u16) -> u32;
    fn FindFirstFileW(lpFileName: *const u16, lpFindFileData: *mut WIN32_FIND_DATAW) -> isize;
    fn FindNextFileW(hFindFile: isize, lpFindFileData: *mut WIN32_FIND_DATAW) -> i32;
    fn FindClose(hFindFile: isize) -> i32;
}
