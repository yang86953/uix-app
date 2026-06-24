// ============================================================================
// platform/linux/system_info/fonts.rs — Font discovery via fontconfig
// ============================================================================
//
// Uses fc-match / fontconfig to discover system-default and CJK fonts.
// Also handles CFF2 variable font detection (fontdue-incompatible).
// ============================================================================

/// 当 fontdue 不支持的 CFF2 可变字体被匹配到时，
/// 尝试问同家族的非 VF 常规风格。
fn try_cff2_alt_family(pattern: &str) -> Option<String> {
    let fam_output = std::process::Command::new("fc-match")
        .args(["-f", "%{family}\n", pattern])
        .output()
        .ok()?;
    if !fam_output.status.success() {
        return None;
    }
    let family = String::from_utf8_lossy(&fam_output.stdout)
        .trim()
        .to_string();
    if family.is_empty() || family == "(null)" {
        return None;
    }
    let query = format!("{}:scalable=True:regular", family);
    let alt_output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}\n", &query])
        .output()
        .ok()?;
    if !alt_output.status.success() {
        return None;
    }
    let alt = String::from_utf8_lossy(&alt_output.stdout)
        .trim()
        .to_string();
    if alt.is_empty() || alt == "(null)" {
        return None;
    }
    Some(alt)
}

/// 用 `fc-match` 查询字体路径，预读数据，自动跳过 CFF2 可变字体。
///
/// 预读取数据让调用方可直接加载，避免二次 I/O。调用方应优先使用返回的 data
/// 而非自己重复 `fs::read(&path)`。
/// Returns `None` if no compatible font found.
fn probe_fc_match_with_data(pattern: &str) -> Option<(String, Vec<u8>)> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}\n", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() || path == "(null)" {
        return None;
    }

    // 快速检测 CFF2 可变字体（fontdue 不支持 CFF2/CFF2 表格）
    if is_cff2_variable_font(&path) {
        // 尝试多种方案找到非 CFF2 替代字体
        let orig_path = path.clone();

        // 方案 1：问同家族的非 VF 版
        if let Some(alt) = try_cff2_alt_family(&pattern) {
            if alt != orig_path && !is_cff2_variable_font(&alt) {
                path = alt;
            }
        }

        // 如果方案 1 没换到有效字体，尝试方案 2：按 locale 找后备
        if path == orig_path {
            let locale = std::env::var("LANG")
                .or_else(|_| std::env::var("LC_ALL"))
                .unwrap_or_default();
            // 中文 locale → 先试 CJK 字体
            let found = if locale.starts_with("zh")
                || locale.starts_with("ja")
                || locale.starts_with("ko")
            {
                probe_font_path_via_fc_match("sans-serif:scalable=true:lang=zh")
                    .or_else(|| probe_font_path_via_fc_match("sans-serif:scalable=true:lang=en"))
            } else {
                probe_font_path_via_fc_match("sans-serif:scalable=true:lang=en")
            };
            if let Some(alt) = found {
                if !is_cff2_variable_font(&alt) {
                    path = alt;
                }
            }
        }

        // 还是 CFF2？最后试 serif
        if is_cff2_variable_font(&path) {
            if let Some(alt) = probe_font_path_via_fc_match("serif:scalable=true") {
                if !is_cff2_variable_font(&alt) {
                    path = alt;
                }
            }
        }

        // 实在找不到非 CFF2 字体，放弃
        if is_cff2_variable_font(&path) {
            return None;
        }
    }

    // 预读字体数据（调用方会用它加载，避免重复 I/O）
    let data = std::fs::read(&path).ok()?;
    Some((path, data))
}

pub fn probe_font_path_via_fc_match(pattern: &str) -> Option<String> {
    probe_fc_match_with_data(pattern).map(|(p, _)| p)
}

/// 快速检测字体文件是否为 CFF2 可变字体（fontdue 不支持 CFF2 glyph 格式）。
///
/// 读前 4 字节判断：
/// - `0x00010000` → TrueType（fontdue 支持）
/// - `OTTO`       → CFF2 单字体（fontdue 不支持）
/// - `ttcf`       → TrueType Collection，进一步检查第一个子字体的 SFNT 版本
fn is_cff2_variable_font(path: &str) -> bool {
    use std::io::{Read, Seek};
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }

    // CFF2 单字体：OTTO
    if &magic == b"OTTO" {
        return true;
    }

    // 非 TTC 合集 → 普通 TrueType，fontdue 支持
    if &magic != b"ttcf" {
        return false;
    }

    // TTC：读取版本号(4B)、字体数量(4B)、第一个子字体 offset(4B)
    let mut buf = [0u8; 12];
    if file.read_exact(&mut buf).is_err() {
        return false;
    }
    let offset = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);

    // 跳转到第一个子字体的 SFNT header
    if file.seek(std::io::SeekFrom::Start(offset as u64)).is_err() {
        return false;
    }
    let mut sfnt = [0u8; 4];
    if file.read_exact(&mut sfnt).is_err() {
        return false;
    }
    // 子字体 SFNT 版本为 OTTO → CFF2 可变字体
    &sfnt == b"OTTO"
}

/// 在 Linux 上用 fontconfig 查询系统默认字体路径（独立函数，引擎可直接调用）。
pub fn probe_system_default_font() -> Option<String> {
    // 第 1 优先：桌面环境的系统字体设置（GNOME/KDE）
    // fc-match 'sans-serif' 不准确——fontconfig 的通用家族别名独立于桌面配置。
    if let Some(desktop_font) = probe_desktop_font() {
        let r = probe_font_path_via_fc_match(&desktop_font);
        if r.is_some() {
            return r;
        }
    }

    // 第 2 优先：fontconfig 的 sans-serif 默认
    let path = probe_font_path_via_fc_match("sans-serif:scalable=true")
        .or_else(|| probe_font_path_via_fc_match("sans-serif"));
    if path.is_some() {
        return path;
    }

    // 后备：locale 感知的 CJK 字体
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    if locale.starts_with("zh") || locale.starts_with("ja") || locale.starts_with("ko") {
        for pattern in &[
            "sans-serif:lang=zh:scalable=true",
            "sans-serif:lang=zh",
            "serif:lang=zh:scalable=true",
        ] {
            let p = probe_font_path_via_fc_match(pattern);
            if p.is_some() {
                return p;
            }
        }
    }

    // 最后的兜底：任何语言的无衬线字体
    probe_font_path_via_fc_match("sans-serif:scalable=true:lang=en")
        .or_else(|| probe_font_path_via_fc_match("serif:scalable=true"))
}

/// 探测桌面环境配置的系统界面字体（GNOME/KDE）。
/// 返回的字符串可直接传给 fc-match 使用。
fn probe_desktop_font() -> Option<String> {
    // GNOME: gsettings get org.gnome.desktop.interface font-name
    // 输出格式: "'字体名称 字号'"  如 "'霞鹜文楷 10'"
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "font-name"])
        .output()
    {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() && s != "'" {
            // 去掉首尾单引号，再切掉末尾的字号
            let cleaned = s.trim_matches('\'');
            if let Some(space_idx) = cleaned.rfind(' ') {
                let family = cleaned[..space_idx].trim();
                if !family.is_empty() {
                    return Some(family.to_string());
                }
            } else {
                // 没有空格（没有字号），整个就是字体名
                let family = cleaned.trim();
                if !family.is_empty() {
                    return Some(family.to_string());
                }
            }
        }
    }

    // KDE: 通过 kreadconfig5 读取系统字体设置
    // 格式: "Noto Sans,10,-1,5,50,0,0,0,0,0"
    if let Ok(output) = std::process::Command::new("kreadconfig5")
        .args(["--group", "General", "--key", "font"])
        .output()
    {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() {
            // 字体名是第一个逗号前的部分
            if let Some(family) = s.split(',').next() {
                let cleaned = family.trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }

    None
}

/// 探测系统上支持中文（CJK）的字体路径。
///
/// 优先用 `:lang=zh` 找含中日韩统一表意文字的字形回退字体。
/// 适用于主字体不含中文时需要找回退字体的场景。
pub fn probe_cjk_font() -> Option<String> {
    // 第 1 优先：明确带 scalable 限制的匹配
    let path = probe_font_path_via_fc_match(":lang=zh:scalable=true");
    if path.is_some() {
        return path;
    }

    // 第 2 优先：sans-serif + lang=zh
    let path = probe_font_path_via_fc_match("sans-serif:lang=zh:scalable=true");
    if path.is_some() {
        return path;
    }

    // 第 3 优先：任何 serif + lang=zh
    probe_font_path_via_fc_match("serif:lang=zh:scalable=true")
}
