//! 扩展包：清单文本与 `.scm` 源文件的不可变冻结快照。
//!
//! 构造时完成结构性校验（路径规范、大小与数量上限、入口存在）；语义
//! 校验（清单字段）由 `ExtensionManifest::parse` 承担。包不可变，检查与
//! 执行使用同一快照。

use std::collections::BTreeMap;

use super::manifest::ExtensionManifest;
use super::ExtensionError;

/// 包体上限（与 docs/架构/app/extensions-engine.md 资源上限一致）。
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 256;
const MAX_COMPONENT_CHARS: usize = 255;
const MAX_MANIFEST_BYTES: usize = 16 * 1024;

/// 冻结的扩展包；构造后不可变。
#[derive(Debug, Clone)]
pub struct ExtensionPackage {
    manifest_text: String,
    sources: BTreeMap<String, String>,
}

impl ExtensionPackage {
    /// 由内存快照构造并做结构校验；文件系统装载属宿主应用责任。
    pub fn from_parts(
        manifest_text: String,
        sources: BTreeMap<String, String>,
    ) -> Result<Self, ExtensionError> {
        if manifest_text.len() > MAX_MANIFEST_BYTES {
            return Err(ExtensionError::Package(format!(
                "清单 {0} 字节超过 {MAX_MANIFEST_BYTES} 上限",
                manifest_text.len()
            )));
        }
        if sources.len() > MAX_FILES {
            return Err(ExtensionError::Package(format!(
                "包内文件数 {} 超过 {MAX_FILES} 上限",
                sources.len()
            )));
        }
        let mut total = manifest_text.len();
        for (path, source) in &sources {
            validate_relative_path("source", path)?;
            if source.len() > MAX_FILE_BYTES {
                return Err(ExtensionError::Package(format!(
                    "文件 {path} 为 {} 字节，超过 {MAX_FILE_BYTES} 上限",
                    source.len()
                )));
            }
            total = total.saturating_add(source.len());
            if total > MAX_TOTAL_BYTES {
                return Err(ExtensionError::Package(format!(
                    "包总字节超过 {MAX_TOTAL_BYTES} 上限"
                )));
            }
        }
        let package = Self {
            manifest_text,
            sources,
        };
        let entry = package.manifest()?.entry;
        if !package.sources.contains_key(&entry) {
            return Err(ExtensionError::Package(format!(
                "入口 {entry} 不在包内来源中"
            )));
        }
        Ok(package)
    }

    /// 解析清单（每次调用重新解析，包本身不持有可变状态）。
    pub fn manifest(&self) -> Result<ExtensionManifest, ExtensionError> {
        ExtensionManifest::parse(&self.manifest_text)
    }

    /// 入口源码。
    pub fn entry_source(&self) -> Result<&str, ExtensionError> {
        let entry = self.manifest()?.entry;
        self.sources
            .get(&entry)
            .map(String::as_str)
            .ok_or_else(|| ExtensionError::Package(format!("入口 {entry} 缺失")))
    }

    /// 包内相对路径 → 源码视图（include 解析的数据来源）。
    pub(crate) fn source(&self, path: &str) -> Option<&str> {
        self.sources.get(path).map(String::as_str)
    }

    /// 来源文件列表。
    pub fn source_paths(&self) -> impl Iterator<Item = &str> {
        self.sources.keys().map(String::as_str)
    }
}

/// 相对路径规范：非空、无 `..` / 绝对前缀 / 反斜杠 / 重复分隔符。
pub(crate) fn validate_relative_path(kind: &str, path: &str) -> Result<(), ExtensionError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.starts_with('/') {
        return Err(ExtensionError::Package(format!(
            "{kind} 路径 {path:?} 无效（非空、≤{MAX_PATH_BYTES} 字节、相对路径）"
        )));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.len() > MAX_COMPONENT_CHARS
            || component.contains('\\')
        {
            return Err(ExtensionError::Package(format!(
                "{kind} 路径 {path:?} 含无效组件"
            )));
        }
    }
    Ok(())
}
