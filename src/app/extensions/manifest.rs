//! 扩展包清单：S 表达式格式（与扩展语言同一 reader，零新增解析依赖）。
//!
//! 形状（schema 1，字段冻结见 docs/架构/app/extensions-engine.md）：
//!
//! ```scheme
//! (uix-extension
//!   (schema-version 1)
//!   (id "text-tools")
//!   (version "0.1.0")
//!   (language r7rs-small)
//!   (entry "main.scm")
//!   (capabilities documents-query)
//!   (state-schema-version 0))
//! ```

use std::collections::BTreeSet;

use super::engine::{SchemeEngine, SchemeLimits, Value};
use super::ExtensionError;

/// 清单字段上限。
const MAX_ID_BYTES: usize = 64;
const MAX_VERSION_BYTES: usize = 32;
const MAX_ENTRY_BYTES: usize = 256;
const MAX_CAPABILITIES: usize = 16;
const MAX_CAPABILITY_BYTES: usize = 128;

/// 解析后的清单；字段不可变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub schema_version: i64,
    pub id: String,
    pub version: String,
    pub language: String,
    pub entry: String,
    pub capabilities: Vec<String>,
    pub state_schema_version: i64,
}

impl ExtensionManifest {
    /// 解析并校验清单文本。
    pub fn parse(text: &str) -> Result<Self, ExtensionError> {
        let mut engine =
            SchemeEngine::new(SchemeLimits::default(), BTreeSet::new(), Vec::new(), Default::default())
                .map_err(|error| ExtensionError::internal("清单引擎构造", &error))?;
        let data =
            super::engine::read_manifest_data(&mut engine, text).map_err(|error| {
                ExtensionError::Package(format!("清单解析失败：{error}"))
            })?;
        if data.len() != 1 {
            return Err(ExtensionError::Package(
                "清单必须是恰好一个 (uix-extension ...) 表达式".to_string(),
            ));
        }
        let clauses = manifest_clauses(&data[0])?;
        let mut schema_version: Option<i64> = None;
        let mut id: Option<String> = None;
        let mut version: Option<String> = None;
        let mut language: Option<String> = None;
        let mut entry: Option<String> = None;
        let mut capabilities: Option<Vec<String>> = None;
        let mut state_schema_version: Option<i64> = None;
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (key, values) in clauses {
            if !seen.insert(key.clone()) {
                return Err(ExtensionError::Package(format!("清单字段 {key} 重复")));
            }
            let single_text = |expected: &str| -> Result<String, ExtensionError> {
                let [Value::String(cell)] = values.as_slice() else {
                    return Err(ExtensionError::Package(format!(
                        "清单字段 {key} 需要恰好一个字符串（{expected}）"
                    )));
                };
                Ok(cell.borrow().clone())
            };
            match key.as_str() {
                "schema-version" => {
                    let [Value::Fixnum(value)] = values.as_slice() else {
                        return Err(ExtensionError::Package(
                            "清单字段 schema-version 需要整数 1".to_string(),
                        ));
                    };
                    schema_version = Some(*value);
                }
                "id" => id = Some(single_text("扩展 ID")?),
                "version" => version = Some(single_text("semver 版本")?),
                "language" => {
                    // 语言标识允许符号或字符串（r7rs-small 是标识符形式）。
                    language = Some(match values.as_slice() {
                        [Value::String(cell)] => cell.borrow().clone(),
                        [Value::Symbol(name)] => name.to_string(),
                        _ => {
                            return Err(ExtensionError::Package(
                                "清单字段 language 需要一个语言标识".to_string(),
                            ))
                        }
                    });
                }
                "entry" => entry = Some(single_text("入口路径")?),
                "capabilities" => {
                    let mut listed = Vec::new();
                    for value in &values {
                        let Value::Symbol(name) = value else {
                            return Err(ExtensionError::Package(
                                "清单字段 capabilities 的能力名必须是符号".to_string(),
                            ));
                        };
                        let name = name.to_string();
                        if name.len() > MAX_CAPABILITY_BYTES
                            || !name.bytes().all(|byte| {
                                byte.is_ascii_lowercase()
                                    || byte.is_ascii_digit()
                                    || matches!(byte, b'-' | b'.' | b'_')
                            })
                        {
                            return Err(ExtensionError::Package(format!(
                                "能力名 {name} 无效（小写字母/数字/连字符/点/下划线，≤{MAX_CAPABILITY_BYTES} 字节）"
                            )));
                        }
                        listed.push(name);
                    }
                    if listed.len() > MAX_CAPABILITIES {
                        return Err(ExtensionError::Package(format!(
                            "能力声明超过 {MAX_CAPABILITIES} 项上限"
                        )));
                    }
                    capabilities = Some(listed);
                }
                "state-schema-version" => {
                    let [Value::Fixnum(value)] = values.as_slice() else {
                        return Err(ExtensionError::Package(
                            "清单字段 state-schema-version 需要非负整数".to_string(),
                        ));
                    };
                    state_schema_version = Some(*value);
                }
                other => {
                    return Err(ExtensionError::Package(format!(
                        "未知清单字段 {other}（schema 1 不允许未知字段）"
                    )))
                }
            }
        }
        let manifest = Self {
            schema_version: schema_version
                .ok_or_else(|| ExtensionError::Package("缺少 schema-version".into()))?,
            id: id.ok_or_else(|| ExtensionError::Package("缺少 id".into()))?,
            version: version.ok_or_else(|| ExtensionError::Package("缺少 version".into()))?,
            language: language.ok_or_else(|| ExtensionError::Package("缺少 language".into()))?,
            entry: entry.ok_or_else(|| ExtensionError::Package("缺少 entry".into()))?,
            capabilities: capabilities.unwrap_or_default(),
            state_schema_version: state_schema_version.unwrap_or(0),
        };
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ExtensionError> {
        if self.schema_version != 1 {
            return Err(ExtensionError::Incompatible(format!(
                "schema-version {} 不受支持（当前为 1）",
                self.schema_version
            )));
        }
        if self.id.is_empty()
            || self.id.len() > MAX_ID_BYTES
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
        {
            return Err(ExtensionError::Package(format!(
                "扩展 ID {:?} 无效（非空、≤{MAX_ID_BYTES} 字节、小写字母/数字/连字符/下划线）",
                self.id
            )));
        }
        if !valid_semver(&self.version) || self.version.len() > MAX_VERSION_BYTES {
            return Err(ExtensionError::Package(format!(
                "版本 {:?} 不是受支持的 semver（x.y.z）",
                self.version
            )));
        }
        if self.language != "r7rs-small" {
            return Err(ExtensionError::Incompatible(format!(
                "语言 {:?} 不受支持（当前为 r7rs-small 受限宿主环境）",
                self.language
            )));
        }
        if self.state_schema_version < 0 {
            return Err(ExtensionError::Package(
                "state-schema-version 必须是非负整数".to_string(),
            ));
        }
        if self.entry.len() > MAX_ENTRY_BYTES || !self.entry.ends_with(".scm") || self.entry.is_empty()
        {
            return Err(ExtensionError::Package(format!(
                "入口 {:?} 无效（.scm 后缀、≤{MAX_ENTRY_BYTES} 字节）",
                self.entry
            )));
        }
        super::package::validate_relative_path("entry", &self.entry)?;
        Ok(())
    }
}

/// 简明 semver 校验：`major.minor.patch`，各段非负整数。
fn valid_semver(text: &str) -> bool {
    let Some((major, rest)) = text.split_once('.') else {
        return false;
    };
    let Some((minor, patch)) = rest.split_once('.') else {
        return false;
    };
    [major, minor, patch]
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// 展开清单头与子句；返回 (键, 值列表) 序列。
fn manifest_clauses(form: &Value) -> Result<Vec<(String, Vec<Value>)>, ExtensionError> {
    let items = flatten(form).ok_or_else(|| {
        ExtensionError::Package("清单必须是 (uix-extension ...) 列表".to_string())
    })?;
    let Some(Value::Symbol(head)) = items.first() else {
        return Err(ExtensionError::Package(
            "清单必须以 uix-extension 开头".to_string(),
        ));
    };
    if head.as_ref() != "uix-extension" {
        return Err(ExtensionError::Package(format!(
            "清单头 {head} 无效（期望 uix-extension）"
        )));
    }
    let mut clauses = Vec::new();
    for clause in &items[1..] {
        let elements = flatten(clause).ok_or_else(|| {
            ExtensionError::Package("清单子句必须是 (键 值...) 列表".to_string())
        })?;
        let Some(Value::Symbol(key)) = elements.first() else {
            return Err(ExtensionError::Package("清单子句必须以字段名开头".to_string()));
        };
        clauses.push((key.to_string(), elements[1..].to_vec()));
    }
    Ok(clauses)
}

/// 严格列表展平（分配额内的小型结构，无需引擎记账）。
fn flatten(value: &Value) -> Option<Vec<Value>> {
    let mut elements = Vec::new();
    let mut current = value.clone();
    loop {
        match current {
            Value::Null => return Some(elements),
            Value::Pair(pair) => {
                let borrowed = pair.borrow();
                elements.push(borrowed.car.clone());
                current = borrowed.cdr.clone();
            }
            _ => return None,
        }
    }
}
