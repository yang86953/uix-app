//! 把共享模块符号及来源图适配为编辑器特性，不持有业务实例或新增语言规则。

use super::{
    DocumentSnapshot, document_snapshot, offset_at, request_position, request_uri, source_uri,
};
use crate::lang::lsp::{
    position::LineIndex,
    scanner,
    session::{ModuleAnalysis, Session, same_document_path},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use crate::lang::compiler::{modules, source_graph::SourceId};

fn snapshot(session: &Session, request: &Value) -> Option<(DocumentSnapshot, usize)> {
    let snapshot = document_snapshot(session, &request_uri(request));
    if !modules::recognizes_source(&snapshot.text) {
        return None;
    }
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    Some((snapshot, offset))
}

fn analysis(session: &mut Session, snapshot: &DocumentSnapshot) -> Option<Arc<ModuleAnalysis>> {
    session
        .module_analysis(&snapshot.uri)
        .or_else(|| session.check_module(&snapshot.uri, &snapshot.text))
}

fn scope(analysis: &ModuleAnalysis, snapshot: &DocumentSnapshot) -> SourceId {
    analysis
        .source_graph
        .files()
        .iter()
        .find(|file| {
            file.path == snapshot.name
                || snapshot
                    .path
                    .as_ref()
                    .is_some_and(|path| same_document_path(path, Path::new(&file.path)))
        })
        .map_or(SourceId::from_source_name(&snapshot.name), |file| file.id)
}

// 限定词只用于定位；是否确实可见始终由编译器的词法范围符号表决定。
fn word(text: &str, offset: usize) -> Option<(usize, usize)> {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let allowed = |c: char| scanner::is_word_char(c) || c == '.';
    let start = text[..offset].rfind(|c| !allowed(c)).map_or(0, |index| {
        index + text[index..].chars().next().unwrap().len_utf8()
    });
    let end = text[offset..]
        .find(|c| !allowed(c))
        .map_or(text.len(), |index| offset + index);
    (end > start).then_some((start, end))
}

fn symbol_at<'a>(
    analysis: &'a ModuleAnalysis,
    snapshot: &DocumentSnapshot,
    offset: usize,
) -> Option<&'a modules::ModuleSymbol> {
    let scope = scope(analysis, snapshot);
    let (start, end) = word(&snapshot.text, offset)?;
    analysis
        .symbols
        .iter()
        .find(|symbol| symbol.scope_id == scope && symbol.name == snapshot.text[start..end])
}

fn range(text: &str, start: usize, end: usize) -> Value {
    let index = LineIndex::new(text);
    let (sl, sc) = index.position(text, start);
    let (el, ec) = index.position(text, end);
    json!({"start":{"line":sl,"character":sc},"end":{"line":el,"character":ec}})
}

fn location(analysis: &ModuleAnalysis, symbol: &modules::ModuleSymbol, fallback: &str) -> Value {
    match analysis.source_graph.file(symbol.source_id) {
        Some(file) => {
            json!({"uri":source_uri(&file.path, fallback),"range":range(&file.source, symbol.start, symbol.end)})
        }
        None => Value::Null,
    }
}

pub(super) fn definition(session: &mut Session, request: &Value) -> Option<Value> {
    let (snapshot, offset) = snapshot(session, request)?;
    let Some(analysis) = analysis(session, &snapshot) else {
        return Some(Value::Null);
    };
    let scope = scope(&analysis, &snapshot);
    let imported = analysis.symbols.iter().find(|symbol| {
        symbol.scope_id == scope
            && symbol
                .import_range
                .is_some_and(|(start, end)| start <= offset && offset < end)
    });
    let symbol = imported.or_else(|| symbol_at(&analysis, &snapshot, offset));
    Some(symbol.map_or(Value::Null, |symbol| {
        location(&analysis, symbol, &snapshot.uri)
    }))
}

pub(super) fn hover(session: &mut Session, request: &Value) -> Option<Value> {
    let (snapshot, offset) = snapshot(session, request)?;
    if let Some(analysis) = analysis(session, &snapshot) {
        if let Some(symbol) = symbol_at(&analysis, &snapshot, offset) {
            return Some(
                json!({"contents":{"kind":"markdown","value":format!("**{}**\n\n{}\n\n来源按冻结闭包定位；运行期源码需 `uix-dynamic`，AOT 需 `uix-modules`。", symbol.name, symbol.detail)}}),
            );
        }
    }
    let Some((start, end)) = word(&snapshot.text, offset) else {
        return Some(Value::Null);
    };
    Some(modules::syntax().iter().find(|entry| entry.tag == &snapshot.text[start..end]).map_or(Value::Null, |entry| {
        json!({"contents":{"kind":"markdown","value":format!("**{}**\n\n{}\n\n属性：{}",entry.tag,entry.summary,entry.attributes.join(", "))}})
    }))
}

pub(super) fn symbols(session: &mut Session, request: &Value) -> Option<Value> {
    let (snapshot, _) = snapshot(session, request)?;
    let Some(analysis) = analysis(session, &snapshot) else {
        return Some(json!([]));
    };
    let scope = scope(&analysis, &snapshot);
    Some(json!(analysis.symbols.iter().filter(|symbol| symbol.scope_id == scope && symbol.source_id == scope).map(|symbol| {
        let range = range(&snapshot.text, symbol.start, symbol.end);
        json!({"name":symbol.name,"detail":symbol.detail,"kind": if symbol.detail.starts_with("Data") { 23 } else if symbol.detail.starts_with("State") { 13 } else { 12 },
            "range":range,"selectionRange":range})
    }).collect::<Vec<_>>()))
}

pub(super) fn completion(session: &Session, request: &Value) -> Option<Value> {
    let (snapshot, offset) = snapshot(session, request)?;
    let mut items = vec![];
    match scanner::completion_context(&snapshot.text, offset) {
        scanner::CompletionContext::TagName => {
            for entry in modules::syntax() {
                items.push(json!({"label":entry.tag,"kind":7,"detail":entry.summary}));
            }
        }
        scanner::CompletionContext::AttributeName { component } => {
            if let Some(entry) = modules::syntax()
                .iter()
                .find(|entry| entry.tag == component)
            {
                for attribute in entry.attributes {
                    items.push(json!({"label":attribute,"kind":10,"detail":entry.summary}));
                }
            }
        }
        _ => {
            if let Some(analysis) = session.module_analysis(&snapshot.uri) {
                let scope = scope(&analysis, &snapshot);
                let (start, end) = word(&snapshot.text, offset).unwrap_or((offset, offset));
                for symbol in analysis
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.scope_id == scope)
                {
                    items.push(json!({"label":symbol.name,"kind":3,"detail":symbol.detail,
                    "textEdit":{"range":range(&snapshot.text,start,end),"newText":symbol.name}}));
                }
            }
        }
    }
    Some(json!({"isIncomplete":false,"items":items}))
}

pub(super) fn references(session: &mut Session, request: &Value) -> Option<Value> {
    let (snapshot, offset) = snapshot(session, request)?;
    let Some(analysis) = analysis(session, &snapshot) else {
        return Some(json!([]));
    };
    let Some(target) = symbol_at(&analysis, &snapshot, offset) else {
        return Some(json!([]));
    };
    let include_declaration = request
        .pointer("/context/includeDeclaration")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut results = vec![];
    // 与既有 UI 引用入口一样返回词法候选；不提供语义重命名或修改源码动作。
    for symbol in analysis.symbols.iter().filter(|symbol| {
        symbol.source_id == target.source_id
            && symbol.start == target.start
            && symbol.end == target.end
    }) {
        let Some(file) = analysis.source_graph.file(symbol.scope_id) else {
            continue;
        };
        for (start, end) in scanner::word_occurrences(&file.source, &symbol.name) {
            if word(&file.source, start) != Some((start, end)) {
                continue;
            }
            if !include_declaration
                && file.id == target.source_id
                && target.start <= start
                && end <= target.end
            {
                continue;
            }
            results.push(json!({"uri":source_uri(&file.path, &snapshot.uri),"range":range(&file.source,start,end)}));
        }
    }
    Some(json!(results))
}
