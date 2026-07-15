use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

const USAGE_DOC: &str = "docs/使用.md";
const TAG_PREFIX: &str = "<!-- uix-compile:";

struct CompiledBlock {
    id: String,
    first_line: usize,
    code: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("usage markdown compile generation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    println!("cargo:rerun-if-changed={USAGE_DOC}");
    let source = fs::read_to_string(USAGE_DOC)
        .map_err(|error| format!("failed to read {USAGE_DOC}: {error}"))?;
    let (total, blocks) = extract_compiled_blocks(&source)?;
    let generated = render_test_module(total, &blocks);
    let output_dir = env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR must be set".to_string())?;
    let output = PathBuf::from(output_dir).join("usage_markdown_compile.rs");
    fs::write(&output, generated)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(())
}

fn extract_compiled_blocks(source: &str) -> Result<(usize, Vec<CompiledBlock>), String> {
    let mut total = 0;
    let mut pending_tag: Option<(String, usize)> = None;
    let mut current: Option<(Option<(String, usize)>, Vec<&str>)> = None;
    let mut blocks = Vec::new();
    let mut ids = HashSet::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        if let Some((tag, lines)) = current.as_mut() {
            if line.trim() == "```" {
                if let Some((id, first_line)) = tag.take() {
                    if !ids.insert(id.clone()) {
                        return Err(format!("duplicate uix-compile id `{id}` in {USAGE_DOC}"));
                    }
                    blocks.push(CompiledBlock {
                        id,
                        first_line,
                        code: lines.join("\n"),
                    });
                }
                current = None;
            } else {
                lines.push(line);
            }
            continue;
        }

        let trimmed = line.trim();
        if let Some(id) = parse_tag(trimmed) {
            if pending_tag.is_some() {
                return Err(format!(
                    "stacked uix-compile tags near line {line_number} in {USAGE_DOC}"
                ));
            }
            pending_tag = Some((sanitize_id(id, line_number)?, line_number));
            continue;
        }
        if trimmed == "```rust" {
            total += 1;
            let tag = pending_tag.take().map(|(id, _)| (id, line_number + 1));
            current = Some((tag, Vec::new()));
            continue;
        }
        if !trimmed.is_empty() {
            if let Some((id, tag_line)) = pending_tag.take() {
                return Err(format!(
                    "uix-compile tag `{id}` at line {tag_line} is not followed by a Rust fence"
                ));
            }
        }
    }

    if current.is_some() {
        return Err(format!("unterminated Rust fence in {USAGE_DOC}"));
    }
    if let Some((id, tag_line)) = pending_tag {
        return Err(format!(
            "dangling uix-compile tag `{id}` at line {tag_line} in {USAGE_DOC}"
        ));
    }
    Ok((total, blocks))
}

fn parse_tag(line: &str) -> Option<&str> {
    line.strip_prefix(TAG_PREFIX)
        .and_then(|value| value.strip_suffix("-->"))
        .map(str::trim)
}

fn sanitize_id(id: &str, line_number: usize) -> Result<String, String> {
    if id.is_empty()
        || !id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(format!(
            "invalid uix-compile id `{id}` at line {line_number} in {USAGE_DOC}"
        ));
    }
    Ok(id.replace('-', "_"))
}

fn render_test_module(total: usize, blocks: &[CompiledBlock]) -> String {
    let mut output = format!(
        "pub(super) const USAGE_RUST_BLOCKS_TOTAL: usize = {total};\n\
         pub(super) const USAGE_RUST_BLOCKS_COMPILED: usize = {};\n\
         #[allow(dead_code, unused_imports, unused_mut, unused_must_use, unused_variables)]\n\
         mod compiled_usage_examples {{\n\
             use crate::prelude::*;\n\
             type GuideResult = Result<(), Box<dyn std::error::Error>>;\n",
        blocks.len()
    );
    for block in blocks {
        output.push_str(&format!(
            "\n    // {USAGE_DOC}:{}\n    fn {}() -> GuideResult {{\n        {{\n{}\n        }};\n        Ok(())\n    }}\n",
            block.first_line,
            block.id,
            indent(&block.code, 12),
        ));
    }
    output.push_str("}\n");
    output
}

fn indent(source: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    source
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
