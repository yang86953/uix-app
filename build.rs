use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[path = "build_support/usage_markdown.rs"]
mod usage_markdown;

const USAGE_DOC: &str = "docs/使用.md";
const USAGE_GUIDE_DIR: &str = "docs/使用指南";
const CARGO_MANIFEST: &str = "Cargo.toml";

fn main() {
    if let Err(error) = run() {
        eprintln!("usage markdown compile generation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let document_paths = usage_document_paths()?;
    println!("cargo:rerun-if-changed={USAGE_GUIDE_DIR}");
    for path in &document_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed={CARGO_MANIFEST}");
    println!("cargo:rerun-if-changed=build_support/usage_markdown.rs");
    let mut documents = Vec::with_capacity(document_paths.len());
    for path in document_paths {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let source_path = path.to_string_lossy().replace('\\', "/");
        documents.push((source_path, source));
    }
    let manifest = fs::read_to_string(CARGO_MANIFEST)
        .map_err(|error| format!("failed to read {CARGO_MANIFEST}: {error}"))?;
    let document_refs = documents
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let generated = usage_markdown::generate_documents(&document_refs, &manifest)?;
    let output_dir = env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR must be set".to_string())?;
    let output = PathBuf::from(output_dir).join("usage_markdown_compile.rs");
    fs::write(&output, generated)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(())
}

fn usage_document_paths() -> Result<Vec<PathBuf>, String> {
    let mut paths = vec![PathBuf::from(USAGE_DOC)];
    let guide_dir = Path::new(USAGE_GUIDE_DIR);
    if guide_dir.exists() {
        collect_markdown_files(guide_dir, &mut paths)?;
    }
    paths.sort();
    Ok(paths)
}

fn collect_markdown_files(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_markdown_files(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            paths.push(path);
        }
    }
    Ok(())
}
