use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

const USAGE_DOC: &str = "docs/使用.md";

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
        if let Some(tag) = parse_rust_fence(trimmed, line_number)? {
            total += 1;
            let tag = tag.map(|id| (id, line_number + 1));
            current = Some((tag, Vec::new()));
        }
    }

    if current.is_some() {
        return Err(format!("unterminated Rust fence in {USAGE_DOC}"));
    }
    Ok((total, blocks))
}

fn parse_rust_fence(line: &str, line_number: usize) -> Result<Option<Option<String>>, String> {
    let Some(suffix) = line.strip_prefix("```rust") else {
        return Ok(None);
    };
    if suffix.is_empty() {
        return Ok(Some(None));
    }
    let Some(metadata) = suffix.strip_prefix(' ') else {
        return Err(format!(
            "unsupported Rust fence metadata `{suffix}` at line {line_number} in {USAGE_DOC}"
        ));
    };
    let Some(id) = metadata.strip_prefix("uix-compile=") else {
        return Err(format!(
            "unsupported Rust fence metadata `{metadata}` at line {line_number} in {USAGE_DOC}"
        ));
    };
    Ok(Some(Some(sanitize_id(id.trim(), line_number)?)))
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
         #[allow(dead_code, unnameable_test_items, unused_imports, unused_mut, unused_must_use, unused_variables)]\n\
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
