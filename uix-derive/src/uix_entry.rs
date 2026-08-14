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

// 引入完整 UIX 文档解析与生成入口。
use crate::uix_lang::{
    Diagnostic, Document, generate_document_app, generate_document_view, generate_record_items,
    parse_document, with_source_markers,
};
// 引入过程宏边界拥有的文件依赖解析 Gate。
use crate::uix_import::{ImportDiagnostic, ResolvedDocument, reject_inline_imports, resolve_file};
// 引入生成文件与 include! 输出边界。
use crate::uix_generated::emit_generated_expression;

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
    expand_source(&input.value(), "<inline>", input)
}

// 公开 uix_app! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_app_public(input: &LitStr) -> TokenStream {
    // 读取宏字符串字面量值。
    let value = input.value();
    // .uix 后缀明确表示编译期文件入口。
    if Path::new(&value)
        // 读取路径扩展名。
        .extension()
        // 精确识别 uix 扩展名。
        .is_some_and(|extension| extension == "uix")
    {
        // 从调用 crate 清单目录解析文件。
        return expand_app_file(input);
    }
    // 其他字符串按内嵌 UIX 源码处理。
    expand_app_inline(input)
}

// 内部测试宏始终把字符串视为内嵌 App 源码。
pub(crate) fn expand_app_inline(input: &LitStr) -> TokenStream {
    // 使用稳定内嵌来源标签生成 App builder。
    expand_app_source(&input.value(), "<inline>", input)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 App .uix 文件。
fn expand_app_file(input: &LitStr) -> TokenStream {
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
                format!("无法确定 uix_app! 调用 crate 的清单目录：{error}"),
                // 给出 Cargo 构建要求。
                "通过 Cargo 编译调用 crate，并确认 CARGO_MANIFEST_DIR 可用",
                // 把错误锚定到宏字面量。
                input,
            );
        }
    };
    // 委托可测试的显式清单目录入口。
    expand_app_file_at(input, Path::new(&manifest_dir))
}

// 相对指定清单目录解析文件图并生成 App builder。
fn expand_app_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    // 解析根文件、递归导入与依赖追踪路径。
    let resolved = match resolve_requested(input, manifest_dir) {
        // 保存完整合并文档。
        Ok(resolved) => resolved,
        // 文件图诊断已经转换为编译错误。
        Err(error) => return error,
    };
    // 使用用户请求路径生成 App 诊断。
    let source_name = input.value();
    // 生成合并文档对应的 App builder。
    let generated = match with_source_markers(|| generate_document_app(&resolved.document)) {
        // 保存成功生成的 App builder。
        Ok(generated) => generated,
        // 转换结构化 codegen 诊断。
        Err(error) => return diagnostic_error(&source_name, &error, input),
    };
    // 注入全部文件的 rustc 编译依赖。
    let tracked = tracked_expression(generated, &resolved.tracked_files, input);
    // 写入可查看的 App 生成文件并返回 include!。
    generated_expression(tracked, &source_name, "app", input)
}

// 解析 UIX 源码并生成公开 App builder 令牌。
fn expand_app_source(
    // 接收完整 UIX 源码。
    source: &str,
    // 接收诊断来源名称。
    source_name: &str,
    // 接收宏输入跨度。
    input: &LitStr,
) -> TokenStream {
    // 执行纯编译期解析、内嵌导入 Gate 与 App 代码生成。
    let generated = parse_inline_document(source)
        // 解析成功后生成现有 App builder。
        .and_then(|document| with_source_markers(|| generate_document_app(&document)));
    // 失败时生成包含来源、位置、原因与建议的编译错误。
    let app = match generated {
        // 保存成功生成的 App builder。
        Ok(app) => app,
        // 转换结构化 UIX 诊断。
        Err(error) => return diagnostic_error(source_name, &error, input),
    };
    // 内嵌入口同样写入可查看的 App 生成文件。
    generated_expression(app, source_name, "app", input)
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
    // 解析根文件、递归导入与依赖追踪路径。
    let resolved = match resolve_requested(input, manifest_dir) {
        // 保存完整合并文档。
        Ok(resolved) => resolved,
        // 文件图诊断已经转换为编译错误。
        Err(error) => return error,
    };
    // 使用用户请求路径生成 View 诊断。
    let source_name = input.value();
    // 生成合并文档对应的 View。
    let generated = match with_source_markers(|| generate_document_view(&resolved.document)) {
        // 保存成功生成的 View。
        Ok(generated) => generated,
        // 转换结构化 codegen 诊断。
        Err(error) => return diagnostic_error(&source_name, &error, input),
    };
    // 注入全部文件的 rustc 编译依赖。
    let tracked = tracked_expression(generated, &resolved.tracked_files, input);
    // 写入可查看的 View 生成文件并返回 include!。
    generated_expression(tracked, &source_name, "view", input)
}

// 公开 uix_items! 自动区分 .uix 路径与内嵌源码。
pub(crate) fn expand_items_public(input: &LitStr) -> TokenStream {
    // 读取宏字符串字面量值。
    let value = input.value();
    // .uix 后缀明确表示编译期文件入口。
    if Path::new(&value)
        .extension()
        .is_some_and(|extension| extension == "uix")
    {
        // 从调用 crate 清单目录解析文件。
        return expand_items_file(input);
    }
    // 其他字符串按内嵌 UIX 源码处理。
    expand_items_inline(input)
}

// 内部测试宏始终把字符串视为内嵌源码。
pub(crate) fn expand_items_inline(input: &LitStr) -> TokenStream {
    // 使用稳定内嵌来源标签生成 record 结构体。
    expand_items_source(&input.value(), "<inline>", input)
}

// 从调用 crate 的 CARGO_MANIFEST_DIR 读取 .uix 文件。
fn expand_items_file(input: &LitStr) -> TokenStream {
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
                format!("无法确定 uix_items! 调用 crate 的清单目录：{error}"),
                // 给出 Cargo 构建要求。
                "通过 Cargo 编译调用 crate，并确认 CARGO_MANIFEST_DIR 可用",
                // 把错误锚定到宏字面量。
                input,
            );
        }
    };
    // 委托可测试的显式清单目录入口。
    expand_items_file_at(input, Path::new(&manifest_dir))
}

// 相对指定清单目录读取文件并生成 record 结构体令牌。
fn expand_items_file_at(input: &LitStr, manifest_dir: &Path) -> TokenStream {
    // 解析根文件、递归导入与依赖追踪路径。
    let resolved = match resolve_requested(input, manifest_dir) {
        // 保存完整合并文档。
        Ok(resolved) => resolved,
        // 文件图诊断已经转换为编译错误。
        Err(error) => return error,
    };
    // 使用用户请求路径生成 record 诊断。
    let source_name = input.value();
    // 生成合并文档中的全部 record。
    let items = match generate_record_items(&resolved.document) {
        // 保存成功生成的结构体序列。
        Ok(items) => items,
        // 转换结构化 codegen 诊断。
        Err(error) => return diagnostic_error(&source_name, &error, input),
    };
    // 把全部文件追踪常量与 items 拼接为模块级令牌。
    tracked_items(items, &resolved.tracked_files, input)
}

// 解析 UIX 源码并生成全部 record 结构体令牌。
fn expand_items_source(
    // 接收完整 UIX 源码。
    source: &str,
    // 接收诊断来源名称。
    source_name: &str,
    // 接收宏输入跨度。
    input: &LitStr,
) -> TokenStream {
    // 执行纯编译期解析、内嵌导入 Gate 与 record 结构体生成。
    let generated =
        parse_inline_document(source).and_then(|document| generate_record_items(&document));
    // 失败时生成包含来源、位置、原因与建议的编译错误。
    let items = match generated {
        // 保存成功生成的结构体。
        Ok(items) => items,
        // 转换结构化 UIX 诊断。
        Err(error) => return diagnostic_error(source_name, &error, input),
    };
    // 内嵌入口直接返回生成结构体。
    items
}

// 解析 UIX 源码并生成公开 Rust View 令牌。
fn expand_source(
    // 接收完整 UIX 源码。
    source: &str,
    // 接收诊断来源名称。
    source_name: &str,
    // 接收宏输入跨度。
    input: &LitStr,
) -> TokenStream {
    // 执行纯编译期解析、内嵌导入 Gate 与代码生成。
    let generated = parse_inline_document(source)
        // 解析成功后生成组件感知 View。
        .and_then(|document| with_source_markers(|| generate_document_view(&document)));
    // 失败时生成包含来源、位置、原因与建议的编译错误。
    let view = match generated {
        // 保存成功生成的 Rust View。
        Ok(view) => view,
        // 转换结构化 UIX 诊断。
        Err(error) => return diagnostic_error(source_name, &error, input),
    };
    // 内嵌入口同样写入可查看的 View 生成文件。
    generated_expression(view, source_name, "view", input)
}

// 把预生成表达式写盘并把文件系统错误转换为入口诊断。
fn generated_expression(
    // 接收预生成 View 或 App 表达式。
    generated: TokenStream,
    // 接收 UIX 来源显示名称。
    source_name: &str,
    // 接收稳定入口类别。
    entry_kind: &str,
    // 接收宏调用字面量。
    input: &LitStr,
) -> TokenStream {
    // 委托唯一生成文件写入器。
    match emit_generated_expression(generated, source_name, entry_kind, input) {
        // 成功时返回 include! 表达式。
        Ok(included) => included,
        // 文件系统失败时生成结构化编译错误。
        Err(error) => entry_error(
            // 保留 UIX 来源名称。
            source_name,
            // 写盘错误固定在入口一行。
            1,
            // 写盘错误固定在入口一列。
            1,
            // 写入具体文件系统错误。
            error,
            // 给出 OUT_DIR 权限与空间修复方向。
            "确认 Cargo OUT_DIR 可写且磁盘空间充足",
            // 锚定宏调用字面量。
            input,
        ),
    }
}

// 解析内嵌文档并拒绝无法确定基准目录的导入。
fn parse_inline_document(source: &str) -> Result<Document, Diagnostic> {
    // 先执行纯语法解析。
    let document = parse_document(source)?;
    // 对内嵌入口执行专用导入 Gate。
    reject_inline_imports(&document)?;
    // 返回不含导入的有效文档。
    Ok(document)
}

// 相对调用 crate 目录解析根文件与递归依赖。
fn resolve_requested(
    // 接收宏路径字面量。
    input: &LitStr,
    // 接收调用 crate 清单目录。
    manifest_dir: &Path,
) -> Result<ResolvedDocument, TokenStream> {
    // 读取用户传入的相对或绝对路径。
    let requested = input.value();
    // 绝对路径保持原样，相对路径基于调用 crate。
    let resolved = if Path::new(&requested).is_absolute() {
        // 复制绝对路径。
        PathBuf::from(&requested)
    } else {
        // 拼接调用 crate 清单目录。
        manifest_dir.join(&requested)
    };
    // 委托唯一文件图 resolver。
    resolve_file(&resolved).map_err(|error| import_diagnostic_error(error, input))
}

// 把真实来源文件的导入诊断转换为编译错误。
fn import_diagnostic_error(error: ImportDiagnostic, input: &LitStr) -> TokenStream {
    // 使用 resolver 保存的真实来源与结构化诊断。
    diagnostic_error(&error.source_name, &error.diagnostic, input)
}

// 生成每个依赖文件对应的 include_str! 字面量。
fn tracked_literals(paths: &[PathBuf], input: &LitStr) -> Vec<LitStr> {
    // 保持 resolver 的首次读取顺序。
    paths
        // 遍历全部规范依赖路径。
        .iter()
        // 把路径锚定到当前宏调用跨度。
        .map(|path| LitStr::new(&path.to_string_lossy(), input.span()))
        // 收集供 quote 重复展开。
        .collect()
}

// 把依赖追踪包裹到表达式生成物之外。
fn tracked_expression(
    // 接收已经生成的表达式令牌。
    generated: TokenStream,
    // 接收根文件与递归导入路径。
    paths: &[PathBuf],
    // 接收宏调用跨度。
    input: &LitStr,
) -> TokenStream {
    // 创建稳定绝对路径字面量。
    let tracked = tracked_literals(paths, input);
    // 返回只含编译期依赖与生成表达式的块。
    quote! {{
        // 让任一文件修改触发宏调用 crate 重新编译。
        #(const _: &str = ::std::include_str!(#tracked);)*
        // 运行期只执行预生成结果。
        #generated
    }}
}

// 把依赖追踪与模块级 items 顺序拼接。
fn tracked_items(
    // 接收已经生成的模块级 items。
    generated: TokenStream,
    // 接收根文件与递归导入路径。
    paths: &[PathBuf],
    // 接收宏调用跨度。
    input: &LitStr,
) -> TokenStream {
    // 创建稳定绝对路径字面量。
    let tracked = tracked_literals(paths, input);
    // 返回模块级依赖常量与结构体序列。
    quote! {
        // 让任一文件修改触发宏调用 crate 重新编译。
        #(const _: &str = ::std::include_str!(#tracked);)*
        // 插入预生成模块级 items。
        #generated
    }
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

    // 验证内嵌源码通过生成文件展开为 View 且不携带 parser。
    #[test]
    fn inline_entry_generates_view_without_runtime_parser() {
        // 创建合法内嵌 UIX 字面量。
        let input = LitStr::new("<Text>Hello</Text>", Span::call_site());
        // 生成并规范化令牌文本。
        let tokens = expand_public(&input).to_string();
        // 宏结果应只包含稳定生成文件入口。
        assert!(tokens.contains("include !"));
        // 生成物不能包含编译期解析函数。
        assert!(!tokens.contains("parse_document"));
        // 内嵌入口的生成文件不需要源文件依赖追踪。
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
        // 宏结果应只包含稳定生成文件入口。
        assert!(tokens.contains("include !"));
        // 生成物不能包含编译期 parser 调用。
        assert!(!tokens.contains("parse_document"));
    }

    // 验证三个文件宏入口都递归解析并追踪完整导入图。
    #[test]
    fn file_entries_track_every_nested_import() {
        // 创建包含两层相对导入的共享 fixture 路径。
        let input = LitStr::new(
            // 使用公开消费者共享的嵌套导入根文件。
            "tests/fixtures/uix_lang/imports/root.uix",
            // 使用调用点跨度。
            Span::call_site(),
        );
        // 创建使用相同依赖图的 App 根路径。
        let app_input = LitStr::new(
            // 使用包含 App 根的嵌套导入 fixture。
            "tests/fixtures/uix_lang/imports/app-root.uix",
            // 使用调用点跨度。
            Span::call_site(),
        );
        // 从 uix-derive 清单目录上移到仓库根。
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        // 分别展开 View、App 与 record 三个文件入口。
        let outputs = [
            // 展开公开 uix! 文件路径。
            expand_file_at(&input, &repository_root),
            // 展开公开 uix_app! 文件路径。
            expand_app_file_at(&app_input, &repository_root),
            // 展开公开 uix_items! 文件路径。
            expand_items_file_at(&input, &repository_root),
        ];
        // 每个入口都必须保留其生成形态与依赖边界。
        for (index, output) in outputs.into_iter().enumerate() {
            // 规范化令牌便于计数。
            let tokens = output.to_string();
            // View 与 App 使用 include!，items 保持模块级直接令牌。
            if index < 2 {
                // 表达式入口必须引用生成文件。
                assert!(tokens.contains("include !"), "{tokens}");
            } else {
                // items 仍直接携带三个文件依赖追踪常量。
                assert_eq!(tokens.matches("include_str").count(), 3, "{tokens}");
            }
            // 展开结果不能包含运行时 parser 调用。
            assert!(!tokens.contains("parse_document"));
        }
    }
}
