//! 导航特性：定义跳转与引用查找，基于语义声明与源码图闭包。

use super::{analysis_for, document_snapshot, offset_at, request_position, request_uri};
use crate::position::LineIndex;
use crate::protocol::source_name_to_uri;
use crate::scanner;
use crate::session::Session;
use serde_json::{Value, json};
use std::path::Path;
use uix_lang_compiler::CheckOutput;
use uix_lang_compiler::semantic_ir::IrSpan;

// 生成 textDocument/definition 响应：单一 Location 或 null。
pub(crate) fn definition(session: &mut Session, request: &Value) -> Value {
    if let Some(result) = super::modules::definition(session, request) {
        return result;
    }
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    // @import 路径字面量优先：直接跳转导入目标文件。
    if let Some(location) = import_location(&snapshot, offset) {
        return location;
    }
    let Some(analysis) = analysis_for(session, &uri) else {
        return Value::Null;
    };
    // 词提取失败（含多字节内部）没有可导航对象。
    let Some((start, end)) = scanner::word_at(&snapshot.text, offset) else {
        return Value::Null;
    };
    // 事件引用去掉 @ 前缀后参与名称查找。
    let word = scanner_word(&snapshot.text, start, end);
    // 按名查找导入闭包内的顶层声明。
    let declaration = analysis
        .ir
        .declarations()
        .iter()
        .find(|declaration| declaration.name == word);
    let Some(declaration) = declaration else {
        return Value::Null;
    };
    declaration_location(&analysis, declaration.span, &snapshot.uri, &snapshot.text)
}

// 生成 textDocument/references 响应：闭包内全词边界出现位置。
pub(crate) fn references(session: &mut Session, request: &Value) -> Value {
    if let Some(result) = super::modules::references(session, request) {
        return result;
    }
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    let Some(analysis) = analysis_for(session, &uri) else {
        return json!([]);
    };
    let Some((start, end)) = scanner::word_at(&snapshot.text, offset) else {
        return json!([]);
    };
    let word = scanner_word(&snapshot.text, start, end);
    // includeDeclaration 缺省为 true，与 LSP 语义一致。
    let include_declaration = request["context"]["includeDeclaration"]
        .as_bool()
        .unwrap_or(true);
    let definition_span = analysis
        .ir
        .declarations()
        .iter()
        .find(|declaration| declaration.name == word)
        .map(|declaration| declaration.span);
    let mut locations = Vec::new();
    for file in analysis.source_graph.files() {
        for (start, end) in scanner::word_occurrences(&file.source, word) {
            // 排除声明时跳过定义声明所在区间。
            if !include_declaration
                && definition_span.is_some_and(|span| {
                    span.source_id == file.id && span.start <= start && end <= span.end
                })
            {
                continue;
            }
            // 图内路径是文件系统路径，必须编码回 file URI。
            let uri = crate::protocol::source_name_to_uri(&file.path, &snapshot.uri);
            locations.push(file_location(&uri, &file.source, start, end));
        }
    }
    json!(locations)
}

// 提取区间内的词文本；@ 前缀事件名去掉前缀参与查找。
fn scanner_word(text: &str, start: usize, end: usize) -> &str {
    let word = &text[start..end];
    word.strip_prefix('@').unwrap_or(word)
}

// 生成声明位置的 Location。
fn declaration_location(
    analysis: &CheckOutput,
    span: IrSpan,
    fallback_uri: &str,
    fallback_text: &str,
) -> Value {
    // 声明可能位于导入闭包内的其他文件。
    let file = analysis.source_graph.file(span.source_id);
    let (source_name, source_text) = match file {
        Some(file) => (file.path.as_str(), file.source.as_str()),
        None => (fallback_uri, fallback_text),
    };
    let uri = source_name_to_uri(source_name, fallback_uri);
    file_location(&uri, source_text, span.start, span.end)
}

// 按文件文本把字节区间编码为 Location。
fn file_location(uri: &str, text: &str, start: usize, end: usize) -> Value {
    let index = LineIndex::new(text);
    let (start_line, start_character) = index.position(text, start);
    let (end_line, end_character) = index.position(text, end);
    json!({
        "uri": uri,
        "range": {
            "start": {"line": start_line, "character": start_character},
            "end": {"line": end_line, "character": end_character}
        }
    })
}

// 光标位于 @import 路径字面量内时返回目标文件的 Location。
fn import_location(snapshot: &super::DocumentSnapshot, offset: usize) -> Option<Value> {
    // 虚拟文档没有目录基准，无法解析相对路径。
    let directory = snapshot.path.as_ref()?.parent()?.to_path_buf();
    // 定位包含光标的行。
    let line_start = snapshot.text[..offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_end = snapshot.text[offset..]
        .find('\n')
        .map_or(snapshot.text.len(), |index| offset + index);
    let line = &snapshot.text[line_start..line_end];
    // 只有 @import 指令行才可能携带路径字面量。
    if !line.trim_start().starts_with("@import") {
        return None;
    }
    // 找到行内覆盖光标的引号字面量。
    let quote = line.chars().find(|c| *c == '\'' || *c == '"')?;
    let open = line.find(quote)?;
    let close = line[open + quote.len_utf8()..]
        .find(quote)
        .map(|index| open + quote.len_utf8() + index)?;
    if offset <= line_start + open || offset > line_start + close {
        return None;
    }
    let raw = &line[open + quote.len_utf8()..close];
    // 相对路径按文档目录做纯词法解析，不触碰文件系统。
    let target = resolve_relative(&directory, Path::new(raw));
    let text = std::fs::read_to_string(&target).unwrap_or_default();
    Some(file_location(
        &crate::protocol::path_to_uri(&target),
        &text,
        0,
        0,
    ))
}

// 纯词法解析 `./` 与 `../` 段，不依赖文件系统规范化。
fn resolve_relative(directory: &Path, raw: &Path) -> std::path::PathBuf {
    use std::path::PathBuf;
    let mut resolved = PathBuf::from(directory);
    for segment in raw.components() {
        match segment {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::{definition, references};
    use crate::session::Session;
    use serde_json::{Value, json};
    use std::time::{SystemTime, UNIX_EPOCH};

    // 建立带导入闭包的临时 fixture，返回（根 URI、依赖 URI）。
    fn import_fixture() -> (String, String, std::path::PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时钟必须可用")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("uix-lsp-nav-{unique}"));
        std::fs::create_dir(&directory).expect("必须创建 fixture 目录");
        let helper = directory.join("helper.uix");
        let root = directory.join("main.uix");
        std::fs::write(
            &helper,
            "@export('Helper')\n<Widget name=\"Helper\">\n<Text>共享</Text>\n</Widget>\n<Helper />",
        )
        .expect("必须写入依赖文件");
        std::fs::write(
            &root,
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>",
        )
        .expect("必须写入根文件");
        (
            format!("file://{}", root.display()),
            format!("file://{}", helper.display()),
            directory,
        )
    }

    #[test]
    fn widget_usage_jumps_to_imported_declaration() {
        let (root_uri, helper_uri, directory) = import_fixture();
        let mut session = Session::default();
        // 打开根文件并触发一次成功检查，登记分析。
        session.store_document(
            root_uri.clone(),
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>".to_string(),
        );
        let response = definition(
            &mut session,
            &json!({"textDocument":{"uri":root_uri},"position":{"line":1,"character":7}}),
        );
        assert_eq!(response["uri"], helper_uri);
        // 声明位于依赖文件第 1 行 <Widget name="Helper">。
        assert_eq!(response["range"]["start"]["line"], 1);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn import_path_word_jumps_to_target_file_start() {
        let (root_uri, helper_uri, directory) = import_fixture();
        let mut session = Session::default();
        session.store_document(
            root_uri.clone(),
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>".to_string(),
        );
        let response = definition(
            &mut session,
            &json!({"textDocument":{"uri":root_uri},"position":{"line":0,"character":12}}),
        );
        assert_eq!(response["uri"], helper_uri);
        assert_eq!(response["range"]["start"], json!({"line":0,"character":0}));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn builtin_components_have_no_definition() {
        let (root_uri, _helper_uri, directory) = import_fixture();
        let mut session = Session::default();
        session.store_document(
            root_uri.clone(),
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>".to_string(),
        );
        let response = definition(
            &mut session,
            &json!({"textDocument":{"uri":root_uri},"position":{"line":1,"character":5}}),
        );
        // App 是内置组件，没有可跳转声明。
        assert!(response.is_null());
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn references_scan_all_closure_files_with_word_boundaries() {
        let (root_uri, helper_uri, directory) = import_fixture();
        let mut session = Session::default();
        session.store_document(
            root_uri.clone(),
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>".to_string(),
        );
        let response = references(
            &mut session,
            &json!({"textDocument":{"uri":root_uri},"position":{"line":1,"character":7},"context":{"includeDeclaration":true}}),
        );
        let locations = response.as_array().expect("必须返回位置数组");
        // 根文件的用法 + 依赖文件的标签用法 + <Widget name="Helper"> 声明 + @export。
        assert!(locations.len() >= 3, "闭包内至少有 3 处 Helper 出现");
        assert!(
            locations
                .iter()
                .any(|location| location["uri"] == Value::String(helper_uri.clone())),
            "必须覆盖依赖文件"
        );
        // 排除声明模式必须滤掉声明区间。
        let excluded = references(
            &mut session,
            &json!({"textDocument":{"uri":root_uri},"position":{"line":1,"character":7},"context":{"includeDeclaration":false}}),
        );
        let excluded_locations = excluded.as_array().expect("必须返回位置数组");
        assert!(excluded_locations.len() < locations.len());
        let _ = std::fs::remove_dir_all(directory);
    }
}
