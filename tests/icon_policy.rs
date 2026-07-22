#![allow(
    clippy::expect_used,
    reason = "these policy tests use expect as an assertion with a focused failure message"
)]

use std::fs;
use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontArc};
use regex::Regex;

fn rust_sources(root: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("enumerate {}: {error}", root.display()));
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn ui_icons_use_the_icon_component_pipeline() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&manifest.join("src/ui/widgets"), &mut sources);
    rust_sources(&manifest.join("demo/src"), &mut sources);
    sources.push(manifest.join("src/ui/foundation/locale.rs"));
    sources.push(manifest.join("docs/使用/输入与表单.md"));
    sources.push(manifest.join("docs/使用/多窗口.md"));
    sources.push(manifest.join("src/tests/ui/window_chrome.rs"));
    sources.push(manifest.join("src/tests/prelude.rs"));
    sources.push(manifest.join("src/tests/ui/widgets/input/select.rs"));

    let standalone_icon_literals = [
        "\"‹\"", "\"›\"", "\"←\"", "\"→\"", "\"↑\"", "\"↓\"", "\"×\"", "\"✕\"", "\"✖\"", "\"✓\"",
        "\"✔\"", "\"▶\"", "\"◀\"", "\"▲\"", "\"▼\"", "\"⚠\"", "\"ℹ\"", "\"★\"", "\"☆\"", "\"☑\"",
        "\"☐\"", "\"□\"", "\"◆\"", "\"📋\"",
    ];
    let retired_locale_icon_fields = [
        "cascader_arrow",
        "pagination_prev_symbol",
        "pagination_next_symbol",
        "table_sort_asc",
        "table_sort_desc",
        "table_sort_unsorted",
    ];
    let legacy_ascii_icon_calls = [
        "text_center(\"+\"",
        "text_center(\"-\"",
        "text_center(\"x\"",
        "text_center(\"<\"",
        "text_center(\">\"",
        "draw_text(\"+\"",
        "draw_text(\"-\"",
        "draw_text(\"x\"",
        "label(\"+\")",
        "label(\"-\")",
        "label(\"x\")",
        "label(\"—\")",
    ];
    let icon_module = manifest.join("src/ui/widgets/general/icon.rs");
    let mut violations = Vec::new();

    for path in sources {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let relative = path.strip_prefix(&manifest).unwrap_or(&path);

        if path != icon_module && source.contains("icon_char(") {
            violations.push(format!(
                "{} calls icon_char directly instead of Icon",
                relative.display()
            ));
        }
        if path != icon_module && source.contains("paint_icon_in_frame(") {
            violations.push(format!(
                "{} calls the legacy icon helper instead of Icon::paint_in_frame",
                relative.display()
            ));
        }
        if path == icon_module && source.contains("pub fn icon_char(") {
            violations.push(format!(
                "{} publicly exposes icon_char and lets callers bypass Icon",
                relative.display()
            ));
        }
        if path == icon_module && source.contains("pub fn paint_icon_in_frame(") {
            violations.push(format!(
                "{} publicly exposes the legacy free paint helper",
                relative.display()
            ));
        }
        for literal in standalone_icon_literals {
            if source.contains(literal) {
                violations.push(format!(
                    "{} contains standalone icon literal {literal}",
                    relative.display()
                ));
            }
        }
        for field in retired_locale_icon_fields {
            if source.contains(field) {
                violations.push(format!(
                    "{} retains text-icon locale field {field}",
                    relative.display()
                ));
            }
        }
        for call in legacy_ascii_icon_calls {
            if source.contains(call) {
                violations.push(format!(
                    "{} renders an ASCII icon outside Icon: {call}",
                    relative.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "all UI icons must use Icon::new(...) or Icon's embedded paint path:\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_mapped_icon_exists_in_the_bundled_lucide_font() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let icon_source = fs::read_to_string(manifest.join("src/ui/widgets/general/icon.rs"))
        .expect("read Icon mapping");
    let mapping = Regex::new(r#"\"([^\"]+)\"\s*=>\s*\"\\u\{([0-9A-Fa-f]+)\}\""#)
        .expect("valid Icon mapping regex");
    let font = FontArc::try_from_vec(
        fs::read(manifest.join("assets/fonts/lucide.ttf")).expect("read bundled Lucide font"),
    )
    .expect("parse bundled Lucide font");
    let mut count = 0;
    let mut missing = Vec::new();

    for captures in mapping.captures_iter(&icon_source) {
        let name = &captures[1];
        let codepoint = u32::from_str_radix(&captures[2], 16).expect("hex Icon codepoint");
        let character = char::from_u32(codepoint).expect("valid Icon Unicode scalar");
        if font.glyph_id(character).0 == 0 {
            missing.push(format!("{name}=U+{codepoint:04X}"));
        }
        count += 1;
    }

    assert!(count >= 134, "expected the frozen Icon mapping inventory");
    assert!(
        missing.is_empty(),
        "Icon mappings absent from bundled lucide.ttf: {}",
        missing.join(", ")
    );
}
