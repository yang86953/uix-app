use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;

const USAGE_DOC: &str = "docs/使用.md";
const CARGO_MANIFEST: &str = "Cargo.toml";

struct CompiledBlock {
    id: String,
    feature: Option<String>,
    first_line: usize,
    code: String,
}

struct CompileTag {
    id: String,
    feature: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("usage markdown compile generation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    println!("cargo:rerun-if-changed={USAGE_DOC}");
    println!("cargo:rerun-if-changed={CARGO_MANIFEST}");
    let source = fs::read_to_string(USAGE_DOC)
        .map_err(|error| format!("failed to read {USAGE_DOC}: {error}"))?;
    let (total, blocks) = extract_compiled_blocks(&source)?;
    let manifest = fs::read_to_string(CARGO_MANIFEST)
        .map_err(|error| format!("failed to read {CARGO_MANIFEST}: {error}"))?;
    let declared_features = extract_declared_features(&manifest)?;
    validate_block_features(&blocks, &declared_features)?;
    let generated = render_test_module(total, &blocks);
    let output_dir = env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR must be set".to_string())?;
    let output = PathBuf::from(output_dir).join("usage_markdown_compile.rs");
    fs::write(&output, generated)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(())
}

fn extract_compiled_blocks(source: &str) -> Result<(usize, Vec<CompiledBlock>), String> {
    let mut total = 0;
    let mut current: Option<(Option<(CompileTag, usize)>, Vec<&str>)> = None;
    let mut blocks = Vec::new();
    let mut ids = HashSet::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        if let Some((tag, lines)) = current.as_mut() {
            if line.trim() == "```" {
                if let Some((tag, first_line)) = tag.take() {
                    if !ids.insert(tag.id.clone()) {
                        return Err(format!(
                            "duplicate uix-compile id `{}` in {USAGE_DOC}",
                            tag.id
                        ));
                    }
                    blocks.push(CompiledBlock {
                        id: tag.id,
                        feature: tag.feature,
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
            let tag = tag.map(|tag| (tag, line_number + 1));
            current = Some((tag, Vec::new()));
        }
    }

    if current.is_some() {
        return Err(format!("unterminated Rust fence in {USAGE_DOC}"));
    }
    Ok((total, blocks))
}

fn extract_declared_features(source: &str) -> Result<HashSet<String>, String> {
    let mut declared = HashSet::new();
    let mut in_features = false;
    let mut found_features = false;
    for line in source.lines() {
        let trimmed = line.split('#').next().unwrap_or_default().trim();
        if trimmed.starts_with('[') {
            if trimmed == "[features]" {
                in_features = true;
                found_features = true;
                continue;
            }
            if in_features {
                break;
            }
        }
        if !in_features || trimmed.is_empty() {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name != "default" {
            declared.insert(name.to_string());
        }
    }
    if !found_features {
        return Err(format!("missing [features] table in {CARGO_MANIFEST}"));
    }
    Ok(declared)
}

fn validate_block_features(
    blocks: &[CompiledBlock],
    declared_features: &HashSet<String>,
) -> Result<(), String> {
    for block in blocks {
        if let Some(feature) = block.feature.as_ref() {
            if !declared_features.contains(feature) {
                return Err(format!(
                    "unknown uix-feature `{feature}` at line {} in {USAGE_DOC}",
                    block.first_line - 1
                ));
            }
        }
    }
    Ok(())
}

fn parse_rust_fence(line: &str, line_number: usize) -> Result<Option<Option<CompileTag>>, String> {
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
    let mut fields = metadata.split_whitespace();
    let Some(id_field) = fields.next() else {
        return Err(format!(
            "missing Rust fence metadata at line {line_number} in {USAGE_DOC}"
        ));
    };
    let Some(id) = id_field.strip_prefix("uix-compile=") else {
        return Err(format!(
            "unsupported Rust fence metadata `{metadata}` at line {line_number} in {USAGE_DOC}"
        ));
    };
    let mut feature = None;
    for field in fields {
        let Some(value) = field.strip_prefix("uix-feature=") else {
            return Err(format!(
                "unsupported Rust fence metadata `{field}` at line {line_number} in {USAGE_DOC}"
            ));
        };
        if feature.is_some() {
            return Err(format!(
                "duplicate uix-feature metadata at line {line_number} in {USAGE_DOC}"
            ));
        }
        feature = Some(sanitize_feature(value, line_number)?);
    }
    Ok(Some(Some(CompileTag {
        id: sanitize_id(id, line_number)?,
        feature,
    })))
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

fn sanitize_feature(feature: &str, line_number: usize) -> Result<String, String> {
    if feature.is_empty()
        || !feature.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(format!(
            "invalid uix-feature `{feature}` at line {line_number} in {USAGE_DOC}"
        ));
    }
    Ok(feature.to_string())
}

fn render_test_module(total: usize, blocks: &[CompiledBlock]) -> String {
    let compiled_count = render_compiled_count(blocks);
    let documented_features = blocks
        .iter()
        .filter_map(|block| block.feature.as_deref())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut output = format!(
        "pub(super) const USAGE_RUST_BLOCKS_TOTAL: usize = {total};\n\
         pub(super) const USAGE_RUST_BLOCKS_COMPILED: usize = {compiled_count};\n\
         pub(super) const USAGE_RUST_BLOCK_FEATURES: &[&str] = &[{documented_features}];\n\
         #[allow(dead_code, unnameable_test_items, unused_imports, unused_mut, unused_must_use, unused_variables)]\n\
         mod compiled_usage_examples {{\n\
             use uix::prelude::*;\n\
             type GuideResult = Result<(), Box<dyn std::error::Error>>;\n",
    );
    for block in blocks {
        let feature_gate = block
            .feature
            .as_ref()
            .map(|feature| format!("    #[cfg(feature = \"{feature}\")]\n"))
            .unwrap_or_default();
        output.push_str(&format!(
            "\n    // {USAGE_DOC}:{}\n{feature_gate}    fn {}() -> GuideResult {{\n        {{\n{}\n        }};\n        Ok(())\n    }}\n",
            block.first_line,
            block.id,
            indent(&block.code, 12),
        ));
    }
    output.push_str("}\n");
    output
}

fn render_compiled_count(blocks: &[CompiledBlock]) -> String {
    let mut unconditional = 0;
    let mut feature_counts = BTreeMap::<&str, usize>::new();
    for block in blocks {
        if let Some(feature) = block.feature.as_deref() {
            *feature_counts.entry(feature).or_default() += 1;
        } else {
            unconditional += 1;
        }
    }

    feature_counts
        .into_iter()
        .fold(unconditional.to_string(), |expression, (feature, count)| {
            format!("{expression} + {count} * (cfg!(feature = \"{feature}\") as usize)")
        })
}

fn indent(source: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    source
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
