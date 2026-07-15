use std::env;
use std::fs;
use std::path::PathBuf;

#[path = "build_support/usage_markdown.rs"]
mod usage_markdown;

const USAGE_DOC: &str = "docs/使用.md";
const CARGO_MANIFEST: &str = "Cargo.toml";

fn main() {
    if let Err(error) = run() {
        eprintln!("usage markdown compile generation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    println!("cargo:rerun-if-changed={USAGE_DOC}");
    println!("cargo:rerun-if-changed={CARGO_MANIFEST}");
    println!("cargo:rerun-if-changed=build_support/usage_markdown.rs");
    let source = fs::read_to_string(USAGE_DOC)
        .map_err(|error| format!("failed to read {USAGE_DOC}: {error}"))?;
    let manifest = fs::read_to_string(CARGO_MANIFEST)
        .map_err(|error| format!("failed to read {CARGO_MANIFEST}: {error}"))?;
    let generated = usage_markdown::generate(&source, &manifest)?;
    let output_dir = env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR must be set".to_string())?;
    let output = PathBuf::from(output_dir).join("usage_markdown_compile.rs");
    fs::write(&output, generated)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(())
}
