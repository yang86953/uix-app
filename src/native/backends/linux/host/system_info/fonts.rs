//! Linux fontconfig discovery. A collection file and its face index form one
//! identity; dropping the index silently changes regional CJK glyph forms.
use crate::platform::services::SystemFontSource;

const MAX_CANDIDATES: usize = 32;

// Stable output includes an index on every row. Malformed metadata is rejected,
// never reinterpreted as face zero. Spaces in file names remain intact.
fn parse_font_sources(output: &str) -> Vec<SystemFontSource> {
    let mut sources = Vec::new();
    for line in output.lines() {
        let Some((path, index)) = line.rsplit_once('\t') else {
            continue;
        };
        if path.is_empty() || path == "(null)" {
            continue;
        }
        let Ok(face_index) = index.trim().parse::<u32>() else {
            continue;
        };
        let source = SystemFontSource {
            path: path.to_owned(),
            face_index,
        };
        if !sources.contains(&source) {
            sources.push(source);
        }
        if sources.len() >= MAX_CANDIDATES {
            break;
        }
    }
    sources
}

fn probe_font_sources(pattern: &str, sorted: bool) -> Vec<SystemFontSource> {
    let mut command = std::process::Command::new("fc-match");
    if sorted {
        command.arg("-s");
    }
    let output = command
        .args(["-f", "%{file}\t%{index}\n", pattern])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_font_sources(&String::from_utf8_lossy(&output.stdout))
}

pub(crate) fn probe_font_source_via_fc_match(pattern: &str) -> Option<SystemFontSource> {
    probe_font_sources(pattern, false).into_iter().next()
}

pub(crate) fn probe_font_path_via_fc_match(pattern: &str) -> Option<String> {
    probe_font_source_via_fc_match(pattern).map(|source| source.path)
}

fn collect_sources(patterns: impl IntoIterator<Item = String>) -> Vec<SystemFontSource> {
    let mut sources = Vec::new();
    for pattern in patterns {
        for source in probe_font_sources(&pattern, true) {
            if !sources.contains(&source) {
                sources.push(source);
            }
            if sources.len() >= MAX_CANDIDATES {
                return sources;
            }
        }
    }
    sources
}

// Compatibility path-only APIs intentionally cannot identify collection members.
fn unique_paths(sources: Vec<SystemFontSource>) -> Vec<String> {
    let mut paths = Vec::new();
    for source in sources {
        if !paths.contains(&source.path) {
            paths.push(source.path);
        }
    }
    paths
}

pub(crate) fn probe_system_default_font_sources() -> Vec<SystemFontSource> {
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
    collect_sources(patterns)
}

pub(crate) fn probe_system_default_font_paths() -> Vec<String> {
    unique_paths(probe_system_default_font_sources())
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

// Preserve the existing fontconfig language/family policy; only retain the face
// it actually selected. Do not force a font family or rewrite OS configuration.
pub(crate) fn probe_cjk_font_sources() -> Vec<SystemFontSource> {
    collect_sources(
        [
            "sans-serif:lang=zh-cn:scalable=true",
            "sans-serif:lang=zh:scalable=true",
            "serif:lang=zh-cn:scalable=true",
        ]
        .into_iter()
        .map(str::to_owned),
    )
}

pub(crate) fn probe_cjk_font_paths() -> Vec<String> {
    unique_paths(probe_cjk_font_sources())
}

pub(crate) fn probe_cjk_font() -> Option<String> {
    probe_cjk_font_sources()
        .into_iter()
        .next()
        .map(|source| source.path)
}

#[cfg(test)]
#[path = "../../../../../../tests-src/native/backends/linux/host/system_info/fonts_tests.rs"]
mod tests;

