// 引入环境、文件系统与稳定路径能力。
use std::{
    env, fs,
    path::{Path, PathBuf},
};

// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;
// 引入字符串字面量语法节点。
use syn::LitStr;
// 引入共享 Emitter 生成的稳定源码映射。
use uix_lang_compiler::source_map::SourceMap;

// 保存生成代码目录名称。
const GENERATED_DIRECTORY: &str = "uix-lang-gen";
// 保存表达式来源标记的内部前缀。
const MARKER_PREFIX: &str = "__uix_source_marker_";

// 把表达式令牌写入稳定生成文件并返回 include! 入口。
pub(crate) fn emit_generated_expression(
    // 接收已经完成语义生成的表达式令牌。
    generated: TokenStream,
    // 接收 UIX 文档显示名称。
    source_name: &str,
    // 接收 View 或 App 入口类别。
    entry_kind: &str,
    // 接收与原始编译令牌对应的 UIX SourceMap。
    source_map: &SourceMap,
    // 接收宏调用字面量以保持 include! 调用跨度。
    input: &LitStr,
) -> Result<TokenStream, String> {
    // 读取调用 crate 清单目录以隔离不同消费者的同名文档。
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        // 非 Cargo 单测环境使用编译期清单目录保持确定性。
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string());
    // 提前取得确定性令牌文本供路径隔离与源码渲染共用。
    let generated = generated.to_string();
    // 构造当前文档与入口类别唯一的稳定文件路径。
    let path = generated_file_path(source_name, entry_kind, &manifest_dir, &generated);
    // 把内部位置令牌转换为真实 UIX 来源注释。
    let rendered = render_generated_source(&generated, source_map)?;
    // 仅在内容变化时写盘，保持增量构建时间戳稳定。
    let _written = write_if_changed(&path, rendered.source.as_bytes())?;
    // SourceMap 与生成 Rust 文件使用同一稳定主名并独立落盘。
    let map_path = path.with_extension("uixmap.json");
    let map = render_source_map(source_map, &rendered.mappings)?;
    let _map_written = write_if_changed(&map_path, &map)?;
    // 把绝对生成路径锚定到宏调用跨度。
    let path_literal = LitStr::new(&path.to_string_lossy(), input.span());
    // 返回由 rustc 继续展开的稳定 include! 入口；items 位置必须自带分号。
    if entry_kind == "items" {
        Ok(quote! { include!(#path_literal); })
    } else {
        Ok(quote! { include!(#path_literal) })
    }
}

// 保存生成 Rust 文本及其中已换算到最终文件字节位置的映射。
struct RenderedGenerated {
    source: String,
    mappings: Vec<RenderedMapping>,
}

// 保存最终生成文件中的半开范围及其共享 SourceMap 条目序号。
struct RenderedMapping {
    generated_start: usize,
    generated_end: usize,
    source_index: usize,
}

// 构造位于 uix-derive OUT_DIR 下的确定生成文件路径。
fn generated_file_path(
    // 接收 UIX 来源显示名称。
    source_name: &str,
    // 接收 View 或 App 入口类别。
    entry_kind: &str,
    // 接收调用 crate 清单目录。
    manifest_dir: &str,
    // 接收确定性生成令牌文本。
    generated: &str,
) -> PathBuf {
    // 从文档路径提取便于查看的文件主名。
    let document = Path::new(source_name)
        // 读取文件主名。
        .file_stem()
        // 转换为可容忍非 UTF-8 的文本。
        .map(|value| value.to_string_lossy().into_owned())
        // 内嵌来源使用稳定名称。
        .unwrap_or_else(|| "inline".to_string());
    // 把主名限制为跨平台安全字符。
    let document = sanitize_file_stem(&document);
    // 内嵌来源没有文件名，必须用内容区分同一 crate 的不同调用。
    let inline_identity = if source_name == "<inline>" {
        // 不同内嵌输入生成不同文件，相同输入仍稳定复用。
        generated
    } else {
        // 文件入口保持路径稳定，内容变化只更新同一可查看文件。
        ""
    };
    // 用调用 crate、来源、入口类别与必要内容形成稳定隔离键。
    let identity = format!("{manifest_dir}\0{source_name}\0{entry_kind}\0{inline_identity}");
    // 计算跨进程可复现的固定 FNV-1a 标识。
    let identity = stable_hash(&identity);
    // 拼接可读主名、稳定标识与 Rust 扩展名。
    let file_name = format!("{document}-{identity:016x}.rs");
    // 返回过程宏自身唯一 OUT_DIR 内的生成目录。
    PathBuf::from(env!("OUT_DIR"))
        // 使用独立子目录避免污染其他构建输出。
        .join(GENERATED_DIRECTORY)
        // 追加当前文档文件名。
        .join(file_name)
}

// 把内部表达式标记替换为可读行映射注释。
fn render_generated_source(
    generated: &str,
    source_map: &SourceMap,
) -> Result<RenderedGenerated, String> {
    // 为生成文件写入稳定说明头。
    let mut rendered = String::from("// 此文件由 uix-derive 生成，请勿手工修改。\n");
    let content_start = rendered.len();
    let mut mappings: Vec<RenderedMapping> = Vec::new();
    let mut marker_index = 0usize;
    // 保存尚未复制的令牌文本起点。
    let mut cursor = 0;
    // 按源码顺序替换全部内部位置令牌。
    while let Some(relative) = generated[cursor..].find(MARKER_PREFIX) {
        // 计算当前标记绝对起点。
        let start = cursor + relative;
        // 复制标记前的普通 Rust 令牌。
        rendered.push_str(&generated[cursor..start]);
        // 前一标记的映射在下一条来源注释前结束。
        if let Some(previous) = mappings.last_mut() {
            previous.generated_end = rendered.len();
        }
        // 解析标记中编码的一基行号。
        let numbers = start + MARKER_PREFIX.len();
        // 查找行列分隔下划线。
        let separator = generated[numbers..]
            // 搜索第一个分隔符。
            .find('_')
            // 缺失表示内部标记被意外破坏。
            .ok_or_else(|| "UIX 生成代码包含损坏的来源行标记".to_string())?
            // 转换为绝对位置。
            + numbers;
        // 把行号文本解析为正整数。
        let line = generated[numbers..separator]
            // 解析十进制行号。
            .parse::<usize>()
            // 转换为稳定内部错误。
            .map_err(|_| "UIX 生成代码包含非法来源行号".to_string())?;
        // 列号从分隔符后开始。
        let column_start = separator + 1;
        // 列号只包含连续十进制数字。
        let column_end = generated[column_start..]
            // 查找第一个非数字字符。
            .find(|character: char| !character.is_ascii_digit())
            // 令牌流保证标识符之后仍有赋值结构。
            .map(|offset| column_start + offset)
            // 缺失表示内部标记没有结束。
            .ok_or_else(|| "UIX 生成代码包含未结束的来源列标记".to_string())?;
        // 把列号文本解析为正整数。
        let column = generated[column_start..column_end]
            // 解析十进制列号。
            .parse::<usize>()
            // 转换为稳定内部错误。
            .map_err(|_| "UIX 生成代码包含非法来源列号".to_string())?;
        let source_index = marker_index.min(source_map.entries().len().saturating_sub(1));
        let source_entry = source_map
            .entries()
            .get(source_index)
            .ok_or_else(|| "UIX Emitter 返回了空 SourceMap".to_string())?;
        let mapped_name = source_entry.source_name.replace(['\r', '\n'], "?");
        // 写入位于对应 Rust 表达式前的可读来源注释。
        rendered.push_str(&format!("\n// [uix-lang] {mapped_name}:{line}:{column}\n"));
        mappings.push(RenderedMapping {
            generated_start: rendered.len(),
            generated_end: 0,
            source_index,
        });
        marker_index += 1;
        // 下一轮从已消费标记标识符之后继续。
        cursor = column_end;
    }
    // 复制最后一个标记之后的剩余 Rust 令牌。
    rendered.push_str(&generated[cursor..]);
    if let Some(previous) = mappings.last_mut() {
        previous.generated_end = rendered.len();
    } else if !source_map.entries().is_empty() {
        mappings.push(RenderedMapping {
            generated_start: content_start,
            generated_end: rendered.len(),
            source_index: 0,
        });
    }
    // 保证生成文件以换行结束，便于稳定查看与诊断。
    rendered.push('\n');
    // 返回可直接由 include! 解析的 Rust 源码。
    Ok(RenderedGenerated {
        source: rendered,
        mappings,
    })
}

// 把最终生成文件范围与共享语义来源编码为稳定 JSON sidecar。
fn render_source_map(
    source_map: &SourceMap,
    mappings: &[RenderedMapping],
) -> Result<Vec<u8>, String> {
    let entries = mappings
        .iter()
        .filter_map(|mapping| {
            source_map
                .entries()
                .get(mapping.source_index)
                .map(|source| {
                    serde_json::json!({
                        "generated_start": mapping.generated_start,
                        "generated_end": mapping.generated_end,
                        "source_id": source.source_id.value(),
                        "source_name": &source.source_name,
                        "source_start": source.source_start,
                        "line": source.line,
                        "column": source.column,
                        "semantic_node_id": &source.semantic_node_id,
                    })
                })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec_pretty(&serde_json::json!({
        "version": 1,
        "entries": entries,
    }))
    .map_err(|error| format!("无法序列化 UIX SourceMap：{error}"))
}

// 仅在内容变化时创建目录并写入生成文件。
fn write_if_changed(path: &Path, content: &[u8]) -> Result<bool, String> {
    // 已存在且内容相同时保持文件时间戳不变。
    if fs::read(path).is_ok_and(|existing| existing == content) {
        // 报告无需写盘。
        return Ok(false);
    }
    // 取得确定存在的父目录。
    let parent = path
        // 读取生成文件父目录。
        .parent()
        // 固定路径构造保证父目录存在。
        .ok_or_else(|| format!("UIX 生成路径没有父目录：{}", path.display()))?;
    // 创建缺失的 OUT_DIR 子目录。
    fs::create_dir_all(parent)
        // 保留精确路径与文件系统原因。
        .map_err(|error| format!("无法创建 UIX 生成目录 {}：{error}", parent.display()))?;
    // 写入完整确定性 UTF-8 Rust 源码。
    fs::write(path, content)
        // 保留精确路径与文件系统原因。
        .map_err(|error| format!("无法写入 UIX 生成文件 {}：{error}", path.display()))?;
    // 报告本次确实更新了生成文件。
    Ok(true)
}

// 把文档主名规范为跨平台安全文件名片段。
fn sanitize_file_stem(value: &str) -> String {
    // 逐字符替换不安全内容并保持输入顺序。
    let sanitized = value
        // 遍历 Unicode 字符。
        .chars()
        // 只保留 ASCII 字母数字、连字符与下划线。
        .map(|character| {
            // 安全字符原样保留。
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                // 返回原字符。
                character
            } else {
                // 其他字符统一替换为下划线。
                '_'
            }
        })
        // 收集为文件名片段。
        .collect::<String>();
    // 空主名回退到稳定 document 名称。
    if sanitized.is_empty() {
        // 返回稳定回退值。
        "document".to_string()
    } else {
        // 返回规范化主名。
        sanitized
    }
}

// 使用固定 FNV-1a 算法生成跨进程稳定标识。
fn stable_hash(value: &str) -> u64 {
    // 初始化标准 FNV-1a 64 位偏移基数。
    let mut hash = 0xcbf29ce484222325_u64;
    // 按 UTF-8 字节顺序更新哈希。
    for byte in value.as_bytes() {
        // 先异或当前字节。
        hash ^= u64::from(*byte);
        // 再乘固定 FNV 质数并允许无符号回绕。
        hash = hash.wrapping_mul(0x100000001b3);
    }
    // 返回确定性哈希。
    hash
}

// 集中验证生成路径、来源注释与增量写盘契约。
#[cfg(test)]
mod tests {
    // 引入调用点跨度。
    use proc_macro2::Span;
    // 引入父模块私有辅助。
    use super::*;
    use uix_lang_compiler::{CompileTarget, compile_inline};

    // 验证内部来源令牌会变成紧邻表达式的映射注释。
    #[test]
    fn renders_uix_source_comments_before_expressions() {
        // 通过共享 Compiler System 取得真实令牌与 SourceMap。
        let output = compile_inline(
            "<Text>{missing_name}</Text>",
            "src/main.uix",
            CompileTarget::View,
        )
        .expect("测试 UIX 必须可编译");
        let generated = output.tokens.to_string();
        // 执行确定性源码渲染。
        let rendered = render_generated_source(&generated, &output.source_map)
            // 内部标记必须成功转换。
            .expect("来源标记应成功渲染");
        // 生成文件必须包含规范映射注释。
        assert!(rendered.source.contains("// [uix-lang] src/main.uix:"));
        // 内部标识符不得泄漏到最终 Rust 源码。
        assert!(!rendered.source.contains("__uix_source_marker"));
        assert!(!rendered.mappings.is_empty());
    }

    // 验证同一输入复用稳定路径与完全相同的 include! 令牌。
    #[test]
    fn emits_deterministic_generated_expression_file() {
        // 创建稳定宏输入字面量。
        let input = LitStr::new("src/main.uix", Span::call_site());
        // 通过共享 Compiler System 构造带真实 SourceMap 的最小表达式。
        let output = compile_inline("<Text>{1}</Text>", "src/main.uix", CompileTarget::View)
            .expect("测试 UIX 必须可编译");
        let generated = output.tokens;
        let generated_text = generated.to_string();
        // 第一次写入并取得 include! 令牌。
        let first = emit_generated_expression(
            generated.clone(),
            "src/main.uix",
            "view",
            &output.source_map,
            &input,
        )
        // OUT_DIR 应可写。
        .expect("首次生成应成功");
        // 第二次使用相同输入生成。
        let second = emit_generated_expression(
            generated,
            "src/main.uix",
            "view",
            &output.source_map,
            &input,
        )
        // 相同内容应直接复用。
        .expect("重复生成应成功");
        // include! 路径与令牌必须完全确定。
        assert_eq!(first.to_string(), second.to_string());
        // 返回令牌必须只暴露生成文件入口。
        assert!(first.to_string().contains("include !"));
        // 构造当前测试对应的确定生成路径。
        let path = generated_file_path(
            // 使用与宏输入一致的来源。
            "src/main.uix",
            // 使用与宏输入一致的入口类别。
            "view",
            // 使用写入器读取的清单目录。
            &env::var("CARGO_MANIFEST_DIR")
                // 单测环境缺失时使用编译期目录。
                .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_string()),
            // 使用与两次写入一致的令牌文本。
            &generated_text,
        );
        // 再次写入磁盘上的相同字节必须报告未改动。
        assert!(
            !write_if_changed(&path, &fs::read(&path).expect("生成文件应存在"))
                // 文件读取与比较必须成功。
                .expect("相同内容检查应成功")
        );
        // 同名 JSON sidecar 必须存在且包含语义节点映射。
        let map = fs::read_to_string(path.with_extension("uixmap.json"))
            .expect("SourceMap sidecar 应存在");
        assert!(map.contains("semantic_node_id"));
    }
}
