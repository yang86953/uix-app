//! 动态 UI 声明：扩展提交的有界描述值、解析与校验。
//!
//! 声明是 tagged list 数据（扩展经 `(uix ui)` 库构造过程或 quote 数据
//! 构造），由 Rust 在提交点全量校验：未知字段、重复 key、超深、超量、
//! 超长文本在改树前拒绝，原树保留。generation 属回调授权，不混入 key。

use super::engine::Value;
use super::ExtensionError;

/// 声明规模上限（与 docs/架构/app/extensions-engine.md 资源上限一致）。
const MAX_NODES: usize = 512;
const MAX_DEPTH: u32 = 16;
const MAX_TEXT_BYTES: usize = 4 * 1024;
const MAX_ITEMS: usize = 256;

/// 已校验的 UI 节点树；owned 数据，可跨线程投递到 UI 线程投影。
#[derive(Debug, Clone, PartialEq)]
pub enum UiNode {
    Column {
        key: String,
        padding: Option<f64>,
        gap: Option<f64>,
        background: Option<String>,
        children: Vec<UiNode>,
    },
    Row {
        key: String,
        padding: Option<f64>,
        gap: Option<f64>,
        background: Option<String>,
        children: Vec<UiNode>,
    },
    Text {
        key: String,
        content: String,
        color: Option<String>,
        size: Option<f64>,
    },
    Input {
        key: String,
        value: String,
        placeholder: Option<String>,
        on_change: Option<String>,
        /// 显式重置：true 时以声明值重建草稿状态。
        reset: bool,
    },
    Button {
        key: String,
        label: String,
        on_click: Option<String>,
        disabled: bool,
    },
    List {
        key: String,
        items: Vec<String>,
    },
}

impl UiNode {
    /// 解析并校验声明值（tagged list）。
    pub fn parse(declaration: &Value) -> Result<Self, ExtensionError> {
        let mut keys = std::collections::BTreeSet::new();
        let mut nodes = 0usize;
        Self::parse_inner(declaration, 0, &mut keys, &mut nodes).map_err(|message| {
            ExtensionError::Argument(format!("UI 声明无效：{message}"))
        })
    }

    fn parse_inner(
        declaration: &Value,
        depth: u32,
        keys: &mut std::collections::BTreeSet<String>,
        nodes: &mut usize,
    ) -> Result<Self, String> {
        if depth > MAX_DEPTH {
            return Err(format!("嵌套深度超过 {MAX_DEPTH} 上限"));
        }
        *nodes += 1;
        if *nodes > MAX_NODES {
            return Err(format!("节点数超过 {MAX_NODES} 上限"));
        }
        let items = flatten(declaration).ok_or("节点必须是列表")?;
        let (kind, rest) = items.split_first().ok_or("节点不能为空")?;
        let Value::Symbol(kind) = kind else {
            return Err("节点头必须是符号".to_string());
        };
        // 拆出 (name value) 属性对与其余子项。
        let mut props: Vec<(String, Value)> = Vec::new();
        let mut rest_items: Vec<Value> = Vec::new();
        for item in rest {
            if let Value::Pair(_) = item {
                let elements = flatten(item).ok_or("属性必须是 (名 值) 对")?;
                if let Some(Value::Symbol(name)) = elements.first()
                    && elements.len() == 2
                    && !matches!(name.as_ref(), "key" if false)
                {
                    let known = matches!(
                        name.as_ref(),
                        "key" | "pad" | "gap" | "bg" | "color" | "size" | "placeholder"
                            | "on-change" | "on-click" | "disabled" | "reset"
                    );
                    if known {
                        props.push((name.to_string(), elements[1].clone()));
                        continue;
                    }
                }
            }
            rest_items.push(item.clone());
        }
        let prop = |name: &str| -> Option<&Value> {
            props.iter().find(|(key, _)| key == name).map(|(_, value)| value)
        };
        let take_key = |keys: &mut std::collections::BTreeSet<String>| -> Result<String, String> {
            match prop("key") {
                Some(Value::String(cell)) => {
                    let key = cell.borrow().clone();
                    if key.is_empty() || key.len() > 128 {
                        return Err("key 必须为 1..=128 字节".to_string());
                    }
                    if !keys.insert(key.clone()) {
                        return Err(format!("重复 key {key:?}"));
                    }
                    Ok(key)
                }
                _ => Err("缺少 (key \"...\") 属性".to_string()),
            }
        };
        let take_text = |value: &Value| -> Result<String, String> {
            match value {
                Value::String(cell) => {
                    let text = cell.borrow().clone();
                    if text.len() > MAX_TEXT_BYTES {
                        return Err(format!("文本超过 {MAX_TEXT_BYTES} 字节上限"));
                    }
                    Ok(text)
                }
                _ => Err("应为字符串".to_string()),
            }
        };
        let take_handler = |value: &Value| -> Result<String, String> {
            match value {
                Value::Symbol(name) => Ok(name.to_string()),
                _ => Err("事件处理名必须是符号".to_string()),
            }
        };
        let take_dimension = |value: &Value| -> Result<f64, String> {
            match value {
                Value::Fixnum(number) => Ok(*number as f64),
                Value::Flonum(number) => Ok(*number),
                _ => Err("尺寸应为数值".to_string()),
            }
        };
        match kind.as_ref() {
            "column" | "row" => {
                let children = rest_items
                    .iter()
                    .map(|child| Self::parse_inner(child, depth + 1, keys, nodes))
                    .collect::<Result<Vec<_>, _>>()?;
                if children.is_empty() {
                    return Err("容器至少需要一个子节点".to_string());
                }
                let padding = match prop("pad") {
                    Some(value) => Some(take_dimension(value)?),
                    None => None,
                };
                let gap = match prop("gap") {
                    Some(value) => Some(take_dimension(value)?),
                    None => None,
                };
                let background = match prop("bg") {
                    Some(value) => Some(take_text(value)?),
                    None => None,
                };
                let key = take_key(keys)?;
                Ok(if kind.as_ref() == "column" {
                    UiNode::Column { key, padding, gap, background, children }
                } else {
                    UiNode::Row { key, padding, gap, background, children }
                })
            }
            "text" => {
                let key = take_key(keys)?;
                let content = match rest_items.first() {
                    Some(value) => take_text(value)?,
                    None => return Err("text 需要字符串内容".to_string()),
                };
                let color = match prop("color") {
                    Some(value) => Some(take_text(value)?),
                    None => None,
                };
                let size = match prop("size") {
                    Some(value) => Some(take_dimension(value)?),
                    None => None,
                };
                Ok(UiNode::Text { key, content, color, size })
            }
            "input" => {
                let key = take_key(keys)?;
                let value = match rest_items.first() {
                    Some(inner) => take_text(inner)?,
                    None => String::new(),
                };
                let placeholder = match prop("placeholder") {
                    Some(inner) => Some(take_text(inner)?),
                    None => None,
                };
                let on_change = match prop("on-change") {
                    Some(inner) => Some(take_handler(inner)?),
                    None => None,
                };
                let reset = match prop("reset") {
                    Some(Value::Bool(flag)) => *flag,
                    None => false,
                    Some(_) => return Err("reset 应为布尔值".to_string()),
                };
                Ok(UiNode::Input { key, value, placeholder, on_change, reset })
            }
            "button" => {
                let key = take_key(keys)?;
                let label = match rest_items.first() {
                    Some(inner) => take_text(inner)?,
                    None => return Err("button 需要字符串标签".to_string()),
                };
                let on_click = match prop("on-click") {
                    Some(inner) => Some(take_handler(inner)?),
                    None => None,
                };
                let disabled = match prop("disabled") {
                    Some(Value::Bool(flag)) => *flag,
                    None => false,
                    Some(_) => return Err("disabled 应为布尔值".to_string()),
                };
                Ok(UiNode::Button { key, label, on_click, disabled })
            }
            "list" => {
                let key = take_key(keys)?;
                let items_value = rest_items.first().ok_or("list 需要字符串列表")?;
                let flattened = flatten(items_value).ok_or("list 项必须是列表")?;
                let mut items = Vec::with_capacity(flattened.len());
                for item in &flattened {
                    items.push(take_text(item)?);
                }
                if items.len() > MAX_ITEMS {
                    return Err(format!("列表项超过 {MAX_ITEMS} 上限"));
                }
                if items.is_empty() {
                    return Err("list 至少一项".to_string());
                }
                Ok(UiNode::List { key, items })
            }
            other => Err(format!("未知节点类型 {other}")),
        }
    }

    /// 节点的稳定 key（reconcile 复用与草稿保留的身份）。
    pub fn key(&self) -> &str {
        match self {
            UiNode::Column { key, .. }
            | UiNode::Row { key, .. }
            | UiNode::Text { key, .. }
            | UiNode::Input { key, .. }
            | UiNode::Button { key, .. }
            | UiNode::List { key, .. } => key,
        }
    }
}

/// 严格列表展平。
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
