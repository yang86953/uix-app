//! CLI、LSP 和其他工具消费同一受检来源；不引入工具专属语法或宿主默认能力。
use super::*;
use crate::lang::compiler::CompilerSystem;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

/// 家族预判只读有界前缀；真正检查仍由各前端读取并冻结完整来源。
/// 预判不验证 UTF-8，避免恰好在多字节字符中间截断产生伪诊断。
pub(crate) fn document_prefix(path: &Path) -> Result<String, CompilerDiagnostic> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(1_048_577).read_to_end(&mut bytes))
        .map_err(|error| crate::lang::compiler::source_io_diagnostic(path, error))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Debug, Clone)]
pub struct ComponentOutput {
    pub checked: CheckedSource,
    /// 配置与接口文件不伪装为 UIX AST，但同样需要跟踪失效。
    pub interface_files: Vec<PathBuf>,
}

impl CompilerSystem {
    pub fn check_component_file(self, path: &Path) -> Result<ComponentOutput, CompilerDiagnostic> {
        self.check_component_file_with_overlays(path, &BTreeMap::new())
    }
    pub fn check_component_file_with_overlays(
        self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
    ) -> Result<ComponentOutput, CompilerDiagnostic> {
        let interfaces = interface::load_project(path)?;
        let linked = link_file_with_overlays(path, overlays)?;
        Ok(ComponentOutput {
            checked: check(linked, &interfaces.libraries)?,
            interface_files: interfaces.tracked_files,
        })
    }
    /// 内嵌来源不搜索磁盘配置；原生签名由调用者明确提供。
    pub fn check_component_inline(
        self,
        source: &str,
        source_name: &str,
        libraries: &NativeLibraries,
    ) -> Result<ComponentOutput, CompilerDiagnostic> {
        Ok(ComponentOutput {
            checked: check(link_inline(source, source_name)?, libraries)?,
            interface_files: Vec::new(),
        })
    }
}
