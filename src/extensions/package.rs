//! 扩展包：清单文本与 `.scm` 源文件的不可变冻结快照。
//!
//! 构造时完成结构性校验（路径规范、大小与数量上限、入口存在）；语义
//! 校验（清单字段）由 `ExtensionManifest::parse` 承担。包不可变，检查与
//! 执行使用同一快照。文件系统读取见 [`ExtensionPackage::read_from_directory`]：
//! 只接受应用显式传入的目录，不扫描、不监听、不跟随包内符号链接。

use std::collections::BTreeMap;
use std::path::Path;

use super::ExtensionError;
use super::manifest::ExtensionManifest;

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
    /// 由内存快照构造并做结构校验；文件系统装载见
    /// [`ExtensionPackage::read_from_directory`]（应用显式指定来源）。
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

    /// 从应用显式指定的目录读取包并冻结为不可变快照。
    ///
    /// 目录形态：恰好一个 `manifest.scm` 清单，其余条目必须是 `.scm`
    /// 普通文件（子目录、符号链接与非 `.scm` 条目拒绝，包内不跟随任何
    /// 链接）；单个文件与总字节沿用 `from_parts` 上限，读取前按元数据
    /// 预检单文件大小。本入口不做任何目录扫描、监听或来源发现：允许
    /// 哪些来源由宿主应用策略决定，这里只读取调用方显式给出的路径。
    pub fn read_from_directory(path: &Path) -> Result<Self, ExtensionError> {
        let entries = std::fs::read_dir(path).map_err(|error| {
            ExtensionError::Package(format!("包来源 {:?} 读取失败：{error}", path.display()))
        })?;
        let mut manifest_text: Option<String> = None;
        let mut sources = BTreeMap::new();
        for entry in entries {
            let entry = entry
                .map_err(|error| ExtensionError::Package(format!("包来源枚举失败：{error}")))?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ExtensionError::Package("包内文件名必须是 UTF-8".to_string()))?;
            // DirEntry::file_type 不跟随符号链接：符号链接与目录一律拒绝，
            // 只接受真实普通文件。
            let file_type = entry.file_type().map_err(|error| {
                ExtensionError::Package(format!("条目 {name} 类型读取失败：{error}"))
            })?;
            if !file_type.is_file() {
                return Err(ExtensionError::Package(format!(
                    "条目 {name:?} 不是普通文件（子目录与符号链接不允许出现在包内）"
                )));
            }
            let metadata = entry.metadata().map_err(|error| {
                ExtensionError::Package(format!("条目 {name} 元数据读取失败：{error}"))
            })?;
            if metadata.len() > MAX_FILE_BYTES as u64 {
                return Err(ExtensionError::Package(format!(
                    "文件 {name} 为 {} 字节，超过 {MAX_FILE_BYTES} 上限",
                    metadata.len()
                )));
            }
            let content = std::fs::read_to_string(entry.path()).map_err(|error| {
                ExtensionError::Package(format!("文件 {name} 读取失败：{error}"))
            })?;
            if name == "manifest.scm" {
                manifest_text = Some(content);
            } else if name.ends_with(".scm") {
                validate_relative_path("source", &name)?;
                sources.insert(name, content);
            } else {
                return Err(ExtensionError::Package(format!(
                    "条目 {name:?} 不是 .scm 源文件（包内只允许 manifest.scm 与 .scm 源文件）"
                )));
            }
        }
        let Some(manifest_text) = manifest_text else {
            return Err(ExtensionError::Package(
                "包缺少 manifest.scm 清单".to_string(),
            ));
        };
        Self::from_parts(manifest_text, sources)
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
