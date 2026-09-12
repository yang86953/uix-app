//! UIX 的 Cargo 构建期入口。使用方声明源文件，生成物只写入 OUT_DIR。

use super::compiler::{CompileTarget, CompilerDiagnostic, CompilerSystem};
use std::path::{Path, PathBuf};
mod components;

/// 一份 UIX 文件的生成形状。
#[derive(Debug, Clone, Copy)]
pub enum OutputKind {
    View,
    App,
    Items,
    Module,
}

impl OutputKind {
    fn directory(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::App => "app",
            Self::Items => "items",
            Self::Module => "module",
        }
    }
}

/// 同一次构建的源码根和生成物根，编译器与运行时无需额外辅助包。
pub struct Builder {
    source_root: PathBuf,
    output_root: PathBuf,
    rust_path: Option<String>,
    catalog: std::sync::Arc<super::compiler::components::ComponentCatalog>,
}

impl Builder {
    /// 从具名 Rust 源码入口发现文件形式的 UIX 引用。只读取语法，不执行宏或源码。
    pub fn compile_rust_directory(&self, directory: impl AsRef<Path>) -> Result<(), String> {
        use syn::visit::Visit;
        #[derive(Default)]
        struct References(Vec<(String, OutputKind)>);
        impl<'ast> Visit<'ast> for References {
            fn visit_macro(&mut self, node: &'ast syn::Macro) {
                let Some(name) = node.path.segments.last().map(|part| part.ident.to_string())
                else {
                    return;
                };
                let kind = match name.as_str() {
                    "uix" => OutputKind::View,
                    "uix_app" => OutputKind::App,
                    "uix_items" => OutputKind::Items,
                    "uix_module" => OutputKind::Module,
                    _ => return,
                };
                if let Ok(path) = syn::parse2::<syn::LitStr>(node.tokens.clone()) {
                    if path.value().ends_with(".uix") {
                        self.0.push((path.value(), kind));
                    }
                }
            }
        }
        let mut pending = vec![self.source_root.join(directory)];
        let mut references = References::default();
        while let Some(directory) = pending.pop() {
            println!("cargo::rerun-if-changed={}", directory.display());
            for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
                let path = entry.map_err(|error| error.to_string())?.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let source =
                        std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
                    let syntax = syn::parse_file(&source)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    references.visit_file(&syntax);
                }
            }
        }
        for (source, kind) in references.0 {
            self.compile(source, kind)?;
        }
        Ok(())
    }

    /// 从 Cargo 当前构建环境创建入口；没有 Cargo 环境时返回错误。
    pub fn from_env() -> Result<Self, String> {
        let source_root =
            std::env::var_os("CARGO_MANIFEST_DIR").ok_or("UIX 构建缺少 CARGO_MANIFEST_DIR")?;
        let output_root = std::env::var_os("OUT_DIR").ok_or("UIX 构建缺少 OUT_DIR")?;
        let mut builder = Self::new(source_root, output_root);
        builder.catalog = std::sync::Arc::new(
            super::compiler::components::ComponentCatalog::for_project(&builder.source_root)?,
        );
        Ok(builder)
    }

    /// CLI 与集成构建可显式提供目录。
    pub fn new(source_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            source_root: source_root.into(),
            output_root: output_root.into(),
            rust_path: None,
            catalog: Default::default(),
        }
    }

    /// Selects the Rust facade used by generated UI declarations.
    pub fn rust_path(mut self, path: impl Into<String>) -> Self {
        self.rust_path = Some(path.into());
        self
    }

    /// Adds component declarations from a library or an application-local descriptor.
    pub fn library(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        let path = self.source_root.join(path);
        println!("cargo::rerun-if-changed={}", path.display());
        std::sync::Arc::make_mut(&mut self.catalog).read_library(&path)?;
        Ok(self)
    }

    /// Adds an embedded descriptor without assuming a dependency filesystem path.
    pub fn library_source(mut self, source: &str) -> Result<Self, String> {
        std::sync::Arc::make_mut(&mut self.catalog).read_library_source(source)?;
        Ok(self)
    }

    /// 编译相对源码根的文件，并登记完整导入闭包的变更追踪。
    pub fn compile(&self, source: impl AsRef<Path>, kind: OutputKind) -> Result<PathBuf, String> {
        self.catalog
            .with(|| self.compile_inner(source.as_ref(), kind))
    }

    fn compile_inner(&self, source: &Path, kind: OutputKind) -> Result<PathBuf, String> {
        validate_source(source)?;
        let path = self.source_root.join(source);
        let tokens = match kind {
            OutputKind::Module => {
                let output = super::compiler::modules::compile_file(&path).map_err(diagnostic)?;
                println!("cargo::rerun-if-changed={}", path.display());
                output
            }
            _ => {
                let target = match kind {
                    OutputKind::View => CompileTarget::View,
                    OutputKind::App => CompileTarget::App,
                    OutputKind::Items => CompileTarget::Items,
                    OutputKind::Module => unreachable!(),
                };
                let output = CompilerSystem::new()
                    .compile_file(&path, target)
                    .map_err(diagnostic)?;
                for tracked in output.tracked_files {
                    println!("cargo::rerun-if-changed={}", tracked.display());
                }
                output.tokens
            }
        };
        let destination = self
            .output_root
            .join("uix")
            .join(kind.directory())
            .join(source)
            .with_added_extension("rs");
        let parent = destination.parent().ok_or("UIX 生成路径没有父目录")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let tokens = readable_tokens(tokens);
        let tokens = match &self.rust_path {
            Some(path) => tokens.replace(":: uix_app :: prelude", &format!(":: {path} :: prelude")),
            None => tokens,
        };
        let generated = format!(
            "// Generated from {}; edit the .uix source.\n{tokens}\n",
            source.display()
        );
        if std::fs::read_to_string(&destination).ok().as_deref() != Some(&generated) {
            std::fs::write(&destination, generated).map_err(|error| error.to_string())?;
        }
        Ok(destination)
    }
}

fn validate_source(source: &Path) -> Result<(), String> {
    if source.is_absolute()
        || source.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "UIX 入口必须是源码根内的相对路径：{}",
            source.display()
        ));
    }
    Ok(())
}

// Preserve Rust token boundaries while giving diagnostics bounded source lines.
// This does not depend on a rustfmt installation or reinterpret string literals.
fn readable_tokens(tokens: proc_macro2::TokenStream) -> String {
    use proc_macro2::{Delimiter, Spacing, TokenTree};
    fn write(tokens: proc_macro2::TokenStream, output: &mut String, indent: usize) {
        let mut joint = true;
        for token in tokens {
            if output.ends_with('\n') {
                output.push_str(&"    ".repeat(indent));
            } else if !joint {
                output.push(' ');
            }
            match token {
                TokenTree::Group(group) => {
                    let (open, close) = match group.delimiter() {
                        Delimiter::Brace => ("{\n", "}"),
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::None => ("", ""),
                    };
                    output.push_str(open);
                    let block = group.delimiter() == Delimiter::Brace;
                    write(group.stream(), output, indent + usize::from(block));
                    if block && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    if block {
                        output.push_str(&"    ".repeat(indent));
                    }
                    output.push_str(close);
                    joint = false;
                }
                TokenTree::Punct(punct) => {
                    output.push(punct.as_char());
                    if punct.as_char() == ';' {
                        output.push('\n');
                    }
                    joint = punct.spacing() == Spacing::Joint;
                }
                token => {
                    output.push_str(&token.to_string());
                    joint = false;
                }
            }
        }
    }
    let mut output = String::new();
    write(tokens, &mut output, 0);
    output
}

fn diagnostic(error: CompilerDiagnostic) -> String {
    format!(
        "{}:{}:{}: {} {} ({})",
        error.source_name, error.line, error.column, error.code, error.message, error.suggestion
    )
}
