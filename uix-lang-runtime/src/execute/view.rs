//! 事务内展开冻结模板，同时生成当前可见实例的事件授权表。

use super::*;
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Default)]
pub(super) struct RenderedView {
    pub snapshot: Option<ViewSnapshot>,
    pub events: BTreeMap<String, EventTarget>,
}

pub(super) struct EventTarget {
    pub handler: String,
    pub captures: Vec<Value>,
    pub disabled: bool,
}

#[derive(Default)]
struct Projection {
    events: BTreeMap<String, EventTarget>,
    keys: BTreeSet<String>,
    bytes: usize,
    items: usize,
}
impl Projection {
    fn charge(&mut self, value: &Value, limits: &Limits) -> RuntimeResult<()> {
        let (bytes, items) = value.budget_usage(
            limits.value_bytes.saturating_sub(self.bytes),
            limits.value_items.saturating_sub(self.items),
        )?;
        self.bytes += bytes;
        self.items += items;
        Ok(())
    }
}

impl Machine<'_> {
    pub(super) fn render(&mut self) -> RuntimeResult<RenderedView> {
        let Some(template) = &self.module.view else {
            return Ok(RenderedView::default());
        };
        if template.control.is_some() {
            return Err(type_error("View 根必须是一个普通控件"));
        }
        let mut projection = Projection::default();
        let mut roots = Vec::new();
        self.render_node(template, &[], "", 0, &mut projection, &mut roots)?;
        self.check()?;
        Ok(RenderedView {
            snapshot: roots.pop(),
            events: projection.events,
        })
    }

    fn render_children(
        &mut self,
        children: &[ViewTemplate],
        captures: &[Value],
        path: &str,
        depth: usize,
        projection: &mut Projection,
        output: &mut Vec<ViewSnapshot>,
    ) -> RuntimeResult<()> {
        for child in children {
            self.render_node(child, captures, path, depth + 1, projection, output)?;
        }
        Ok(())
    }

    fn render_node(
        &mut self,
        template: &ViewTemplate,
        captures: &[Value],
        path: &str,
        depth: usize,
        projection: &mut Projection,
        output: &mut Vec<ViewSnapshot>,
    ) -> RuntimeResult<()> {
        self.check()?;
        if depth > self.limits.depth {
            return Err(quota_error());
        }
        match &template.control {
            Some(ViewControl::If {
                condition,
                otherwise,
            }) => {
                let branch = if self
                    .invoke(condition, captures.to_vec(), Effect::Query)?
                    .as_bool()?
                {
                    &template.children
                } else {
                    otherwise
                };
                return self.render_children(branch, captures, path, depth, projection, output);
            }
            Some(ViewControl::For {
                items,
                key,
                indexed,
            }) => {
                let values = self.invoke(items, captures.to_vec(), Effect::Query)?;
                let mut seen = BTreeSet::new();
                for (index, value) in values.as_array()?.iter().enumerate() {
                    self.check()?;
                    let mut args = captures.to_vec();
                    args.push(value.clone());
                    if *indexed {
                        args.push(Value::Int(i64::try_from(index).map_err(|_| quota_error())?));
                    }
                    let identity = self.invoke(key, args.clone(), Effect::Query)?;
                    let segment = key_segment(&identity)?;
                    if !seen.insert(segment.clone()) {
                        return Err(
                            RuntimeError::new(ErrorKind::Conflict, "For 的稳定 key 重复").at(&self
                                .module
                                .functions
                                .iter()
                                .find(|f| f.signature.name == *key)
                                .map(|f| f.location.clone())
                                .unwrap_or_default()),
                        );
                    }
                    self.render_children(
                        &template.children,
                        &args,
                        &format!("{path}/{segment}"),
                        depth,
                        projection,
                        output,
                    )?;
                }
                return Ok(());
            }
            None => {}
        }
        let key = format!("{}{path}", template.key);
        if !projection.keys.insert(key.clone()) {
            return Err(RuntimeError::new(ErrorKind::Conflict, "View 实例 key 重复"));
        }
        if projection.keys.len() > 4096 {
            return Err(quota_error());
        }
        projection.charge(&Value::String(key.clone()), self.limits)?;
        let mut properties = BTreeMap::new();
        for (property, function) in &template.properties {
            let value = self.invoke(function, captures.to_vec(), Effect::Query)?;
            if matches!(property.as_str(), "gap" | "padding")
                && !matches!(value, Value::Float(v) if (0.0..=4096.0).contains(&v))
            {
                return Err(type_error("布局间距必须在 0 到 4096 之间"));
            }
            projection.charge(&Value::String(property.clone()), self.limits)?;
            projection.charge(&value, self.limits)?;
            properties.insert(property.clone(), value);
        }
        if let Some(handler) = &template.handler {
            projection.charge(&Value::Array(captures.to_vec()), self.limits)?;
            let disabled = properties
                .get("disabled")
                .map(Value::as_bool)
                .transpose()?
                .unwrap_or(false);
            projection.events.insert(
                key.clone(),
                EventTarget {
                    handler: handler.clone(),
                    captures: captures.to_vec(),
                    disabled,
                },
            );
        }
        let mut children = Vec::new();
        self.render_children(
            &template.children,
            captures,
            path,
            depth,
            projection,
            &mut children,
        )?;
        output.push(ViewSnapshot {
            kind: template.kind,
            key,
            properties,
            children,
        });
        Ok(())
    }
}

fn key_segment(value: &Value) -> RuntimeResult<String> {
    match value {
        Value::Int(value) => Ok(format!("i{value}")),
        Value::String(value) => {
            let mut encoded = String::from("s");
            for byte in value.bytes() {
                write!(&mut encoded, "{byte:02x}").expect("写入 String 不失败");
            }
            Ok(encoded)
        }
        _ => Err(type_error("For key 只能是 String 或 Int")),
    }
}
