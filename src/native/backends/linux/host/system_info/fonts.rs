// ============================================================================
// platform/linux/system_info/fonts.rs — Font discovery via fontconfig
// ============================================================================
//
// Uses fc-match / fontconfig to discover system-default and CJK fonts.
// Also handles CFF2 variable font detection for formats unsupported by fontdue.
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
/// Returns `None` if no supported font is found.
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

pub(crate) fn probe_font_path_via_fc_match(pattern: &str) -> Option<String> {
    probe_fc_match_with_data(pattern).map(|(p, _)| p)
}

/// 使用 fontconfig 的排序结果收集同一模式下的全部字体路径。
fn probe_font_paths_via_fc_match(pattern: &str) -> Vec<String> {
    // 要求 fontconfig 返回按匹配优先级排列的完整候选，而不是只取可能缺字的首项。
    let output = std::process::Command::new("fc-match")
        // `-s` 保留排序列表，稳定格式只输出文件路径。
        .args(["-s", "-f", "%{file}\n", pattern])
        // 同步等待短生命周期的字体发现进程完成。
        .output();
    // 命令不可用时保持平台能力缺失语义。
    let Ok(output) = output else {
        // 空列表允许 FontService 使用最终 bitmap fallback。
        return Vec::new();
    };
    // fontconfig 失败时不得解析不完整的标准输出。
    if !output.status.success() {
        // 返回空列表保留调用方既有失败处理。
        return Vec::new();
    }
    // 把 fontconfig 的逐行路径转成拥有所有权的候选列表。
    String::from_utf8_lossy(&output.stdout)
        // 每一行对应一个按优先级排序的字体文件。
        .lines()
        // 清除命令输出可能携带的首尾空白。
        .map(str::trim)
        // 忽略空行与 fontconfig 的空值占位。
        .filter(|path| !path.is_empty() && *path != "(null)")
        // 平台发现层不猜测字体轮廓能力；FontService 会用当前文本后端真实尝试并继续回退。
        // 候选必须独立拥有路径，不能借用命令输出缓冲区。
        .map(str::to_owned)
        // 收集后交给跨模式去重逻辑。
        .collect()
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
pub(crate) fn probe_system_default_font() -> Option<String> {
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
pub(crate) fn probe_cjk_font() -> Option<String> {
    // 兼容单候选调用方时沿用完整有序列表的首项。
    probe_cjk_font_paths().into_iter().next()
}

/// 探测系统上按优先级排列的 CJK 字体候选。
pub(crate) fn probe_cjk_font_paths() -> Vec<String> {
    // 限制启动阶段保留的候选数量，避免异常 fontconfig 配置放大内存和失败探测成本。
    const MAX_CANDIDATES: usize = 32;
    // zh-cn 在常见 fontconfig 配置中比宽泛 zh 更可靠地把真实 CJK 字体排在首位。
    const PATTERNS: [&str; 3] = [
        // 优先选择简体中文无衬线 UI 字体。
        "sans-serif:lang=zh-cn:scalable=true",
        // 兼容只登记宽泛中文语言标签的系统字体。
        "sans-serif:lang=zh:scalable=true",
        // 无衬线候选均不可用时允许中文衬线字体回退。
        "serif:lang=zh-cn:scalable=true",
    ];
    // 按模式与 fontconfig 排序共同维护稳定候选顺序。
    let mut paths = Vec::new();
    // 逐个查询从具体到宽泛的 CJK 模式。
    for pattern in PATTERNS {
        // 保留当前模式下的全部后续候选，让 FontService 能跳过缺字首项。
        for path in probe_font_paths_via_fc_match(pattern) {
            // 同一字体可能被多个模式返回，只允许进入候选列表一次。
            if !paths.iter().any(|known| known == &path) {
                // 新候选保持 fontconfig 的原始优先级。
                paths.push(path);
            }
            // 达到上限后停止继续解析低优先级候选。
            if paths.len() >= MAX_CANDIDATES {
                // 返回已经按优先级去重的有界列表。
                return paths;
            }
        }
    }
    // 返回不足上限的完整去重候选列表。
    paths
}
