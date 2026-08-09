// 引入环境、文件系统与路径能力。
use std::{env, fs, path::Path};

// 引入过程宏跨度与令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;
// 引入字符串字面量语法节点。
use syn::LitStr;

// 引入完整 UIX 文档解析与生成入口。
use crate::uix_lang::{generate_document_view, parse_document, Diagnostic};

// 公开 uix! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_public(input: &LitStr) -> TokenStream {
    // 读取宏字符串字面量值。
    let value = input.value();
    // .uix 后缀明确表示编译期文件入口。
    if Path::new(&value)
        .extension()
        .is_some_and(|extension| extension == "uix")
    {
        // 从调用 crate 清单目录解析文件。
        return expand_file(input);
    }
    // 其他字符串按内嵌 UIX 源码处理。
    expand_inline(input)
}

// 内部测试宏始终把字符串视为内嵌源码。
pub(crate) fn expand_inline(input: &LitStr) -> TokenStream {
    // 使用稳定内嵌来源标签生成代码。
    expand_source(&input.value(), "<inline>", None, input)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 .uix 文件。
fn expand_file(input: &LitStr) -> TokenStream {
    // 读取调用 crate 的清单目录。
    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        // 保存可用目录。
        Ok(directory) => directory,
        // 环境缺失时生成结构化编译错误。
        Err(error) => {
            // 返回入口环境诊断。
            return entry_error(
                // 使用用户输入路径作为来源。
                &input.value(),
                // 入口错误固定在一行一列。
                1,
                // 入口错误固定在一行一列。
                1,
                // 说明缺失环境变量。
                format!("无法确定 uix! 调用 crate 的清单目录：{error}"),
                // 给出 Cargo 构建要求。
                "通过 Cargo 编译调用 crate，并确认 CARGO_MANIFEST_DIR 可用",
                // 把错误锚定到宏字面量。
                input,
            );
        }
    };
    // 委托可测试的显式清单目录入口。
    expand_file_at(input, Path::new(&manifest_dir))
}

// 相对指定清单目录读取文件并生成依赖追踪令牌。
fn expand_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    // 读取用户传入的相对或绝对路径。
    let requested = input.value();
    // 绝对路径保持原样，相对路径基于调用 crate。
    let resolved = if Path::new(&requested).is_absolute() {
        // 复制绝对路径。
        Path::new(&requested).to_path_buf()
    } else {
        // 拼接调用 crate 清单目录。
        manifest_dir.join(&requested)
    };
    // 在过程宏执行期读取 UTF-8 源码。
    let source = match fs::read_to_string(&resolved) {
        // 保存有效 UTF-8 文档。
        Ok(source) => source,
        // 路径、权限或编码错误转为 compile_error。
        Err(error) => {
            // 返回文件读取诊断。
            return entry_error(
                // 展示用户可识别的请求路径。
                &requested,
                // 文件读取前没有更精确行号。
                1,
                // 文件读取前没有更精确列号。
                1,
                // 说明解析后的实际路径与系统原因。
                format!("无法读取 UIX 文件 {}：{error}", resolved.display()),
                // 给出路径、权限与 UTF-8 修复建议。
                "确认路径相对调用 crate 的 CARGO_MANIFEST_DIR、文件存在且为 UTF-8",
                // 把错误锚定到路径字面量。
                input,
            );
        }
    };
    // 成功读取后规范化路径供依赖追踪与诊断。
    let tracked_path = resolved
        // 尝试消除相对片段。
        .canonicalize()
        // 文件已读取成功，失败时保留原解析路径。
        .unwrap_or(resolved);
    // 把绝对路径转换为 include_str! 字面量。
    let tracked_literal = LitStr::new(&tracked_path.to_string_lossy(), input.span());
    // 使用请求路径作为稳定用户诊断标签。
    expand_source(
        // 传入文件源码。
        &source,
        // 传入用户请求路径。
        &requested,
        // 生成 rustc 文件依赖追踪。
        Some(tracked_literal),
        // 锚定到宏路径字面量。
        input,
    )
}

// 解析 UIX 源码并生成公开 Rust View 令牌。
fn expand_source(
    // 接收完整 UIX 源码。
    source: &str,
    // 接收诊断来源名称。
    source_name: &str,
    // 接收可选文件依赖字面量。
    tracked_file: Option<LitStr>,
    // 接收宏输入跨度。
    input: &LitStr,
) -> TokenStream {
    // 执行纯编译期解析与代码生成。
    let generated = parse_document(source)
        // 解析成功后生成组件感知 View。
        .and_then(|document| generate_document_view(&document));
    // 失败时生成包含来源、位置、原因与建议的编译错误。
    let view = match generated {
        // 保存成功生成的 Rust View。
        Ok(view) => view,
        // 转换结构化 UIX 诊断。
        Err(error) => return diagnostic_error(source_name, &error, input),
    };
    // 文件入口用 include_str! 让 rustc 跟踪变更。
    if let Some(tracked_file) = tracked_file {
        // 返回仅含编译期依赖与生成 View 的表达式。
        return quote! {{
            // 让文件修改触发宏调用 crate 重新编译。
            const _: &str = ::std::include_str!(#tracked_file);
            // 运行期只构建预生成 View，不解析源码。
            #view
        }};
    }
    // 内嵌入口直接返回预生成 View。
    view
}

// 把 UIX 结构化诊断转换为 compile_error!。
fn diagnostic_error(source_name: &str, error: &Diagnostic, input: &LitStr) -> TokenStream {
    // 委托统一入口错误格式。
    entry_error(
        // 写入内嵌标签或文件路径。
        source_name,
        // 写入一基行号。
        error.span.line,
        // 写入一基列号。
        error.span.column,
        // 写入失败原因。
        &error.message,
        // 写入可执行修复建议。
        &error.suggestion,
        // 锚定到宏输入字面量。
        input,
    )
}

// 生成统一来源与修复建议格式的 compile_error!。
fn entry_error(
    // 接收来源标签或路径。
    source_name: &str,
    // 接收一基行号。
    line: usize,
    // 接收一基列号。
    column: usize,
    // 接收失败原因。
    message: impl AsRef<str>,
    // 接收修复建议。
    suggestion: impl AsRef<str>,
    // 接收错误锚定字面量。
    input: &LitStr,
) -> TokenStream {
    // 拼接稳定诊断文本。
    let message = format!(
        // 固定文件、行列、原因与建议顺序。
        "UIX {source_name}:{line}:{column}: {}；建议：{}",
        // 写入失败原因。
        message.as_ref(),
        // 写入修复建议。
        suggestion.as_ref(),
    );
    // 使用 syn 生成调用点 compile_error!。
    syn::Error::new(
        // 锚定路径或内嵌字符串字面量。
        input.span(),
        // 写入完整诊断文本。
        message,
    )
    // 转换为 compile_error! 令牌。
    .to_compile_error()
}

// 集中验证入口选择、诊断与零运行时 parser 事实。
#[cfg(test)]
mod tests {
    // 引入调用点跨度与字符串字面量。
    use proc_macro2::Span;
    // 引入父模块入口。
    use super::*;

    // 验证内嵌源码直接展开为 View 且不携带 parser。
    #[test]
    fn inline_entry_generates_view_without_runtime_parser() {
        // 创建合法内嵌 UIX 字面量。
        let input = LitStr::new("<Text>Hello</Text>", Span::call_site());
        // 生成并规范化令牌文本。
        let tokens = expand_public(&input).to_string();
        // 生成物应调用公开 label View。
        assert!(tokens.contains("label"));
        // 生成物不能包含编译期解析函数。
        assert!(!tokens.contains("parse_document"));
        // 内嵌入口不需要文件依赖追踪。
        assert!(!tokens.contains("include_str"));
    }

    // 验证语法与映射错误包含来源、行列和建议。
    #[test]
    fn inline_errors_preserve_structured_diagnostics() {
        // 创建标签闭合错误源码。
        let syntax = LitStr::new("<Column>\n<Text>x</Column>", Span::call_site());
        // 生成语法错误令牌。
        let syntax_tokens = expand_public(&syntax).to_string();
        // 语法错误必须产生 compile_error!。
        assert!(syntax_tokens.contains("compile_error"));
        // 诊断必须包含内嵌来源与精确第二行。
        assert!(syntax_tokens.contains("<inline>:2:"));
        // 诊断必须包含修复建议字段。
        assert!(syntax_tokens.contains("建议"));

        // 创建未登记元素映射源码。
        let mapping = LitStr::new("<Mystery />", Span::call_site());
        // 生成映射错误令牌。
        let mapping_tokens = expand_public(&mapping).to_string();
        // 映射错误必须产生 compile_error!。
        assert!(mapping_tokens.contains("compile_error"));
        // 诊断必须保留未知元素名称。
        assert!(mapping_tokens.contains("Mystery"));
    }

    // 验证缺失文件报告解析路径与修复建议。
    #[test]
    fn missing_file_generates_path_diagnostic() {
        // 创建确定不存在的 .uix 相对路径。
        let input = LitStr::new("missing/uix_entry_gate.uix", Span::call_site());
        // 使用确定不存在的清单目录展开。
        let tokens = expand_file_at(&input, Path::new("missing_manifest_root")).to_string();
        // 路径错误必须产生 compile_error!。
        assert!(tokens.contains("compile_error"));
        // 诊断必须保留用户请求路径。
        assert!(tokens.contains("missing/uix_entry_gate.uix"));
        // 诊断必须包含调用 crate 路径语义建议。
        assert!(tokens.contains("CARGO_MANIFEST_DIR"));
    }

    // 验证文件入口加入依赖追踪但不携带运行时 parser。
    #[test]
    fn file_entry_tracks_source_without_runtime_parser() {
        // 创建仓库根测试 fixture 的相对路径。
        let input = LitStr::new(
            // 使用公开消费者共享的 .uix 文件。
            "tests/fixtures/uix_lang/public_entry.uix",
            // 使用调用点跨度。
            Span::call_site(),
        );
        // 从 uix-derive 清单目录上移到仓库根。
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        // 生成文件入口令牌。
        let tokens = expand_file_at(&input, &repository_root).to_string();
        // 生成物应包含 rustc 依赖追踪。
        assert!(tokens.contains("include_str"));
        // 生成物应包含公开 View 构造代码。
        assert!(tokens.contains("column"));
        // 生成物不能包含编译期 parser 调用。
        assert!(!tokens.contains("parse_document"));
    }
}
