// ============================================================================
// platform/linux/system_info/fonts.rs — Font discovery via fontconfig
// ============================================================================
//
// Uses fc-match / fontconfig to discover ordered system-default and CJK fonts.
// ============================================================================

/// 用 `fc-match` 查询首个字体路径；轮廓能力统一由 FontService 的真实栅格探针判断。
pub(crate) fn probe_font_path_via_fc_match(pattern: &str) -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}\n", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() || path == "(null)" {
        return None;
    }
    Some(path)
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

/// 在 Linux 上返回 fontconfig 排序后的默认字体候选，让 FontService 跳过空栅格字体。
pub(crate) fn probe_system_default_font_paths() -> Vec<String> {
    // 有界候选避免异常 fontconfig 配置放大启动探测成本。
    const MAX_CANDIDATES: usize = 32;
    // 桌面显式字体优先，其后才是通用 sans 与保底 serif。
    let mut patterns = Vec::new();
    if let Some(desktop_font) = probe_desktop_font() {
        patterns.push(desktop_font);
    }
    patterns.extend(
        [
            "sans-serif:scalable=true",
            "sans-serif:scalable=true:lang=en",
            "serif:scalable=true",
        ]
        .into_iter()
        .map(str::to_owned),
    );

    let mut paths = Vec::new();
    for pattern in patterns {
        for path in probe_font_paths_via_fc_match(&pattern) {
            if !paths.iter().any(|known| known == &path) {
                paths.push(path);
            }
            if paths.len() >= MAX_CANDIDATES {
                return paths;
            }
        }
    }
    paths
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
