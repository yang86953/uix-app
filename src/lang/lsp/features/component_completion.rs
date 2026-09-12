//! 补全只返回文本编辑；部分 Inspection 不进入成功分析缓存或执行入口。
use super::{document_snapshot, offset_at, request_position, request_uri};
use crate::lang::{
    compiler::component_source::completion::{self, CompletionKind},
    lsp::{
        position::LineIndex,
        session::{Session, same_document_path},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub(super) fn complete(session: &Session, request: &Value) -> Value {
    let snapshot = document_snapshot(session, &request_uri(request));
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    let analysis = if let Some(path) = &snapshot.path {
        completion::inspect_file_with_overlays(
            &session.component_edit_root(path),
            session.overlays(),
        )
    } else {
        completion::inspect_inline(&snapshot.text, &snapshot.name, &BTreeMap::new())
    };
    let result = analysis
        .ok()
        .and_then(|analysis| {
            let file = analysis.source().source_graph.files().iter().find(|file| {
                file.path == snapshot.name
                    || snapshot
                        .path
                        .as_ref()
                        .is_some_and(|path| same_document_path(path, Path::new(&file.path)))
            })?;
            if file.source != snapshot.text {
                return None;
            }
            analysis.complete(file.id, offset).ok()
        })
        .or_else(|| completion::declaration_completion(&snapshot.text, offset));
    let Some(result) = result else {
        return json!({"isIncomplete":true,"items":[]});
    };
    let index = LineIndex::new(&snapshot.text);
    let (sl, sc) = index.position(&snapshot.text, result.replacement.start);
    let (el, ec) = index.position(&snapshot.text, result.replacement.end);
    let range = json!({"start":{"line":sl,"character":sc},"end":{"line":el,"character":ec}});
    json!({"isIncomplete":result.is_incomplete,"items":result.items.into_iter().map(|item| {
        let kind = match item.kind {
            CompletionKind::Variable => 6, CompletionKind::Function => 3, CompletionKind::Component => 7,
            CompletionKind::Type => 25, CompletionKind::Property => 10, CompletionKind::Method => 2, CompletionKind::Keyword => 14,
        };
        json!({"label":item.label,"kind":kind,"detail":item.detail,"textEdit":{"range":range,"newText":item.label}})
    }).collect::<Vec<_>>()})
}
