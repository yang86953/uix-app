//! 语言特性 Module 组：把会话快照与编译器公开事实适配为 LSP 响应。

pub(crate) mod completion;
pub(crate) mod diagnostics;
pub(crate) mod formatting;
pub(crate) mod hover;
mod modules;
pub(crate) mod navigation;
pub(crate) mod symbols;

use crate::lang::lsp::position::LineIndex;
use crate::lang::lsp::protocol::{source_name_to_uri, uri_to_path};
use crate::lang::lsp::session::Session;
use serde_json::Value;
use std::path::PathBuf;
use crate::lang::compiler::CheckOutput;

// 保存一次特性请求所需的文档快照。
pub(crate) struct DocumentSnapshot {
    // 请求中的原始 URI。
    pub uri: String,
    // 传给编译器的规范来源名：文件路径或 URI 本身。
    pub name: String,
    // file URI 对应的本地路径；虚拟文档为 None。
    pub path: Option<PathBuf>,
    // 当前生效源码：overlay 优先，其次磁盘，最后空串兜底。
    pub text: String,
}

// 汇集请求文档的当前源码与来源名。
pub(crate) fn document_snapshot(session: &Session, uri: &str) -> DocumentSnapshot {
    // 打开文档的 overlay 优先；未打开文件回退磁盘内容。
    let text = session
        .document_source(uri)
        .map(str::to_string)
        .or_else(|| uri_to_path(uri).and_then(|path| std::fs::read_to_string(path).ok()))
        .unwrap_or_default();
    let path = uri_to_path(uri);
    let name = path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| uri.to_string());
    DocumentSnapshot {
        uri: uri.to_string(),
        name,
        path,
        text,
    }
}

// 提取参数中的 textDocument/uri；本层函数接收 params，不含信封。
pub(crate) fn request_uri(request: &Value) -> String {
    request
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

// 提取参数中的 position 行列；缺省按文档开头处理。
pub(crate) fn request_position(request: &Value) -> (u64, u64) {
    let position = &request["position"];
    (
        position["line"].as_u64().unwrap_or(0),
        position["character"].as_u64().unwrap_or(0),
    )
}

// 把 LSP 行列位置换算为当前源码中的字节偏移。
pub(crate) fn offset_at(text: &str, line: u64, character: u64) -> usize {
    LineIndex::new(text).offset(text, line, character)
}

// 取得覆盖该文档的最新分析；必要时把文档作为根重新检查并缓存。
pub(crate) fn analysis_for(session: &mut Session, uri: &str) -> Option<CheckOutput> {
    // 已有分析覆盖该文档时直接复用，不重复编译。
    if let Some(path) = uri_to_path(uri) {
        if let Some(analysis) = session.analysis_covering(&path) {
            return Some(analysis.clone());
        }
    } else if let Some(analysis) = session.analysis(uri) {
        return Some(analysis.clone());
    }
    // 否则把当前文档当根重新检查；失败时特性按无 IR 降级。
    session.check_as_root(uri)
}

// 把编译器来源名映射回 URI；无法定位时回退请求 URI。
pub(crate) fn source_uri(source_name: &str, fallback: &str) -> String {
    source_name_to_uri(source_name, fallback)
}

#[cfg(test)]
mod tests {
    use crate::lang::lsp::features::document_snapshot;
    use crate::lang::lsp::session::Session;

    #[test]
    fn snapshot_prefers_open_overlay_over_disk() {
        let path =
            std::env::temp_dir().join(format!("uix-lsp-snapshot-{}.uix", std::process::id()));
        std::fs::write(&path, "<App><Text>磁盘内容</Text></App>").expect("必须写入磁盘 fixture");
        let uri = format!("file://{}", path.display());
        let mut session = Session::default();
        session.store_document(uri.clone(), "<App><Text>内存内容</Text></App>".to_string());

        let snapshot = document_snapshot(&session, &uri);

        assert_eq!(snapshot.text, "<App><Text>内存内容</Text></App>");
        assert_eq!(snapshot.name, path.display().to_string());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn snapshot_falls_back_to_disk_for_unopened_files() {
        let path = std::env::temp_dir().join(format!("uix-lsp-disk-{}.uix", std::process::id()));
        std::fs::write(&path, "<App><Text>磁盘内容</Text></App>").expect("必须写入磁盘 fixture");
        let uri = format!("file://{}", path.display());
        let session = Session::default();

        let snapshot = document_snapshot(&session, &uri);

        assert_eq!(snapshot.text, "<App><Text>磁盘内容</Text></App>");
        let _ = std::fs::remove_file(&path);
    }
}
