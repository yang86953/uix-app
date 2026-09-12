//! i18n 资源表与 `t!` 宏（E-08）。
//!
//! 文案集中在进程级资源表（key → 文案），组件按 key 取用，不各自维护字符串：
//!
//!
//! - `t!` 返回 `String`（显示用途），不改变业务值。
//! - 未命中 key 时返回 key 原文（容错：不 panic、不静默吞错）。
//! - `set_translations` 整体替换资源表（语言切换）；`register_translations`
//!   增量合并（同 key 覆盖）。
//!
//! 与 `Locale` 的关系：`Locale` 承载内置组件文案（结构体字段形态），本表承载
//! 应用自定义文案（key 形态）；两者互不依赖，可并存。

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static RESOURCES: OnceLock<RwLock<HashMap<&'static str, &'static str>>> = OnceLock::new();

fn table() -> &'static RwLock<HashMap<&'static str, &'static str>> {
    RESOURCES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 注册翻译条目（增量合并；同 key 覆盖）。
pub fn register_translations(entries: &[(&'static str, &'static str)]) {
    // 毒锁恢复：RwLock 中毒时取回内部值，不把锁竞争转化为 panic。
    let mut table = table()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for (key, value) in entries {
        table.insert(*key, *value);
    }
}

/// 整体替换资源表（语言切换）；覆盖全部既有条目。
pub fn set_translations(entries: &[(&'static str, &'static str)]) {
    // 毒锁恢复：RwLock 中毒时取回内部值，不把锁竞争转化为 panic。
    let mut table = table()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.clear();
    for (key, value) in entries {
        table.insert(*key, *value);
    }
}

/// 按 key 取文案；未命中返回 key 原文。
pub fn t_lookup(key: &str) -> String {
    // 毒锁恢复：RwLock 中毒时取回内部值，不把锁竞争转化为 panic。
    table()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(key)
        .copied()
        .unwrap_or(key)
        .to_string()
}

/// 按 key 取文案并替换 `{0}` 占位；未命中返回 key 原文。
pub fn t_lookup_fmt(key: &str, args: &[String]) -> String {
    let template = t_lookup(key);
    if args.is_empty() {
        return template;
    }
    let mut out = template;
    for (index, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{index}}}"), arg);
    }
    out
}
