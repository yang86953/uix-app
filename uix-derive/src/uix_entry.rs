// 引入环境与路径能力。
use std::{
    env,
    path::{Path, PathBuf},
};

// 引入过程宏跨度与令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;
// 引入字符串字面量语法节点。
use syn::LitStr;
// 引入共享 Compiler System 的三个公开编译目标。
use uix_lang_compiler::{
    CompileOutput, CompileTarget, CompilerDiagnostic, compile_file, compile_inline,
};

// 引入生成文件与 include! 输出边界。
use crate::uix_generated::emit_generated_expression;

// 公开 uix! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_public(input: &LitStr) -> TokenStream {
    if is_file_input(input) {
        expand_file(input)
    } else {
        expand_inline(input)
    }
}

// 内部测试宏始终把字符串视为内嵌 View 源码。
pub(crate) fn expand_inline(input: &LitStr) -> TokenStream {
    expand_inline_target(input, CompileTarget::View, "view")
}

// 公开 uix_app! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_app_public(input: &LitStr) -> TokenStream {
    if is_file_input(input) {
        expand_app_file(input)
    } else {
        expand_app_inline(input)
    }
}

// 内部测试宏始终把字符串视为内嵌 App 源码。
pub(crate) fn expand_app_inline(input: &LitStr) -> TokenStream {
    expand_inline_target(input, CompileTarget::App, "app")
}

// 公开 uix_items! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_items_public(input: &LitStr) -> TokenStream {
    if is_file_input(input) {
        expand_items_file(input)
    } else {
        expand_items_inline(input)
    }
}

// 内部入口把字符串视为内嵌 Record 文档。
fn expand_items_inline(input: &LitStr) -> TokenStream {
    expand_inline_target(input, CompileTarget::Items, "items")
}

// 精确识别公开文件入口。
fn is_file_input(input: &LitStr) -> bool {
    Path::new(&input.value())
        .extension()
        .is_some_and(|extension| extension == "uix")
}

// 编译内嵌输入并按目标形状交给生成文件 Adapter。
fn expand_inline_target(input: &LitStr, target: CompileTarget, entry_kind: &str) -> TokenStream {
    let output = match compile_inline(&input.value(), "<inline>", target) {
        Ok(output) => output,
        Err(error) => return diagnostic_error(&error, input),
    };
    finish_output(output, "<inline>", target, entry_kind, input)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 View 文件。
fn expand_file(input: &LitStr) -> TokenStream {
    let Some(manifest_dir) = manifest_dir(input, "uix!") else {
        return missing_manifest_error(input, "uix!");
    };
    expand_file_at(input, &manifest_dir)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 App 文件。
fn expand_app_file(input: &LitStr) -> TokenStream {
    let Some(manifest_dir) = manifest_dir(input, "uix_app!") else {
        return missing_manifest_error(input, "uix_app!");
    };
    expand_app_file_at(input, &manifest_dir)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 Record 文件。
fn expand_items_file(input: &LitStr) -> TokenStream {
    let Some(manifest_dir) = manifest_dir(input, "uix_items!") else {
        return missing_manifest_error(input, "uix_items!");
    };
    expand_items_file_at(input, &manifest_dir)
}

// 读取调用 crate 清单目录；错误由统一入口生成。
fn manifest_dir(_input: &LitStr, _macro_name: &str) -> Option<PathBuf> {
    env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from)
}

// 生成缺失 Cargo 调用环境的稳定诊断。
fn missing_manifest_error(input: &LitStr, macro_name: &str) -> TokenStream {
    entry_error(
        &input.value(),
        1,
        1,
        format!("无法确定 {macro_name} 调用 crate 的清单目录"),
        "通过 Cargo 编译调用 crate，并确认 CARGO_MANIFEST_DIR 可用",
        input,
    )
}

// 相对指定清单目录读取文件并生成 View。
fn expand_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    expand_file_target(input, manifest_dir, CompileTarget::View, "view")
}

// 相对指定清单目录读取文件并生成 App builder。
fn expand_app_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    expand_file_target(input, manifest_dir, CompileTarget::App, "app")
}

// 相对指定清单目录读取文件并生成 Record items。
fn expand_items_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    expand_file_target(input, manifest_dir, CompileTarget::Items, "items")
}

// 把过程宏路径环境适配为 Compiler System 的真实文件命令。
fn expand_file_target(
    input: &LitStr,
    manifest_dir: &Path,
    target: CompileTarget,
    entry_kind: &str,
) -> TokenStream {
    let requested = input.value();
    let path = if Path::new(&requested).is_absolute() {
        PathBuf::from(&requested)
    } else {
        manifest_dir.join(&requested)
    };
    let output = match compile_file(&path, target) {
        Ok(output) => output,
        Err(error) => return diagnostic_error(&error, input),
    };
    finish_output(output, &requested, target, entry_kind, input)
}

// 按表达式或模块级 items 形状完成依赖追踪与生成文件适配。
fn finish_output(
    output: CompileOutput,
    source_name: &str,
    target: CompileTarget,
    entry_kind: &str,
    input: &LitStr,
) -> TokenStream {
    let included = generated_expression(
        output.tokens,
        source_name,
        entry_kind,
        &output.source_map,
        input,
    );
    if target == CompileTarget::Items {
        tracked_items(included, &output.tracked_files, input)
    } else {
        tracked_expression(included, &output.tracked_files, input)
    }
}

// 把预生成表达式写盘并把文件系统错误转换为入口诊断。
fn generated_expression(
    generated: TokenStream,
    source_name: &str,
    entry_kind: &str,
    source_map: &uix_lang_compiler::source_map::SourceMap,
    input: &LitStr,
) -> TokenStream {
    match emit_generated_expression(generated, source_name, entry_kind, source_map, input) {
        Ok(included) => included,
        Err(error) => entry_error(
            source_name,
            1,
            1,
            error,
            "确认 Cargo OUT_DIR 可写且磁盘空间充足",
            input,
        ),
    }
}

// 生成每个依赖文件对应的 include_str! 字面量。
fn tracked_literals(paths: &[PathBuf], input: &LitStr) -> Vec<LitStr> {
    paths
        .iter()
        .map(|path| LitStr::new(&path.to_string_lossy(), input.span()))
        .collect()
}

// 把依赖追踪包裹到表达式生成物之外。
fn tracked_expression(generated: TokenStream, paths: &[PathBuf], input: &LitStr) -> TokenStream {
    let tracked = tracked_literals(paths, input);
    quote! {{
        #(const _: &str = ::std::include_str!(#tracked);)*
        #generated
    }}
}

// 把依赖追踪与模块级 items 顺序拼接。
fn tracked_items(generated: TokenStream, paths: &[PathBuf], input: &LitStr) -> TokenStream {
    let tracked = tracked_literals(paths, input);
    quote! {
        #(const _: &str = ::std::include_str!(#tracked);)*
        #generated
    }
}

// 把共享结构化诊断转换为过程宏 compile_error!。
fn diagnostic_error(error: &CompilerDiagnostic, input: &LitStr) -> TokenStream {
    let message = format!(
        "UIX {} {} {}:{}:{} [bytes {}..{}]: {}；建议：{}",
        error.code,
        error.phase.as_str(),
        error.source_name,
        error.line,
        error.column,
        error.start,
        error.end,
        error.message,
        error.suggestion,
    );
    syn::Error::new(input.span(), message).to_compile_error()
}

// 生成统一来源与修复建议格式的 compile_error!。
fn entry_error(
    source_name: &str,
    line: usize,
    column: usize,
    message: impl AsRef<str>,
    suggestion: impl AsRef<str>,
    input: &LitStr,
) -> TokenStream {
    let message = format!(
        "UIX {source_name}:{line}:{column}: {}；建议：{}",
        message.as_ref(),
        suggestion.as_ref(),
    );
    syn::Error::new(input.span(), message).to_compile_error()
}

// 集中验证入口选择、诊断与零运行时 parser 事实。
#[cfg(test)]
mod tests {
    use proc_macro2::Span;

    use super::*;

    #[test]
    fn inline_entry_generates_view_without_runtime_parser() {
        let input = LitStr::new("<Text>Hello</Text>", Span::call_site());
        let tokens = expand_public(&input).to_string();
        assert!(tokens.contains("include !"));
        assert!(!tokens.contains("parse_document"));
        assert!(!tokens.contains("include_str"));
    }

    #[test]
    fn inline_errors_preserve_structured_diagnostics() {
        let syntax = LitStr::new("<Column>\n<Text>x</Column>", Span::call_site());
        let syntax_tokens = expand_public(&syntax).to_string();
        assert!(syntax_tokens.contains("compile_error"));
        assert!(syntax_tokens.contains("<inline>:2:"));
        assert!(syntax_tokens.contains("建议"));

        let mapping = LitStr::new("<Mystery />", Span::call_site());
        let mapping_tokens = expand_public(&mapping).to_string();
        assert!(mapping_tokens.contains("compile_error"));
        assert!(mapping_tokens.contains("Mystery"));
    }

    #[test]
    fn missing_file_generates_path_diagnostic() {
        let input = LitStr::new("missing/uix_entry_gate.uix", Span::call_site());
        let tokens = expand_file_at(&input, Path::new("missing_manifest_root")).to_string();
        assert!(tokens.contains("compile_error"));
        assert!(tokens.contains("missing/uix_entry_gate.uix"));
        assert!(tokens.contains("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn file_entry_tracks_source_without_runtime_parser() {
        let input = LitStr::new(
            "tests/fixtures/uix_lang/public_entry.uix",
            Span::call_site(),
        );
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let tokens = expand_file_at(&input, &repository_root).to_string();
        assert!(tokens.contains("include !"));
        assert!(!tokens.contains("parse_document"));
    }

    #[test]
    fn file_entries_track_every_nested_import() {
        let input = LitStr::new(
            "tests/fixtures/uix_lang/imports/root.uix",
            Span::call_site(),
        );
        let app_input = LitStr::new(
            "tests/fixtures/uix_lang/imports/app-root.uix",
            Span::call_site(),
        );
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let outputs = [
            expand_file_at(&input, &repository_root),
            expand_app_file_at(&app_input, &repository_root),
            expand_items_file_at(&input, &repository_root),
        ];
        for output in outputs {
            let tokens = output.to_string();
            assert!(tokens.contains("include !"), "{tokens}");
            assert_eq!(tokens.matches("include_str").count(), 3, "{tokens}");
            assert!(!tokens.contains("parse_document"));
        }
    }
}
