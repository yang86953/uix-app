//! 冻结快照到原生 ViewNode 的投影。事件闭包只投递拥有型参数。

use super::*;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::{button, column, label, row};
use crate::ui::widgets::combinators::input;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};

struct Draft { value: State<String>, pending: Arc<AtomicU64> }
#[derive(Default)]
struct Drafts { generation: u64, inputs: BTreeMap<String, Draft> }

/// 一个挂载面的投影状态。应用在根工厂中调用 project，并在销毁挂载面时关闭 worker。
#[derive(Clone)]
pub struct ModuleView {
    handle: ModuleHandle,
    changed: State<()>,
    drafts: Arc<Mutex<Drafts>>,
}
impl ModuleView {
    /// 创建后台业务线程与界面投影器；wake 只负责唤醒所属窗口或后台操作面。
    pub fn spawn(instance: Instance, capacity: usize, wake: impl Fn() + Send + Sync + 'static) -> RuntimeResult<(ModuleWorker, Self)> {
        let changed = State::new(());
        let signal = changed.clone();
        let worker = ModuleWorker::spawn(instance, capacity, move || { signal.set(()); wake(); })?;
        let view = Self { handle: worker.handle(), changed, drafts: Arc::new(Mutex::new(Drafts::default())) };
        Ok((worker, view))
    }
    pub fn handle(&self) -> ModuleHandle { self.handle.clone() }

    /// 只消费已提交快照，不执行模块。错误由返回值交给应用恢复或 diagnostics。
    pub fn project(&self) -> RuntimeResult<ViewNode> {
        self.changed.get();
        let snapshot = self.handle.snapshot();
        if !snapshot.active { return Err(snapshot.error.unwrap_or_else(|| RuntimeError::new(ErrorKind::Closed, "模块界面已关闭"))); }
        let node = snapshot.view.as_ref().ok_or_else(|| RuntimeError::new(ErrorKind::InvalidModule, "模块未声明 View"))?;
        let mut drafts = self.drafts.lock().unwrap_or_else(|error| error.into_inner());
        if drafts.generation != snapshot.generation {
            drafts.inputs.clear();
            drafts.generation = snapshot.generation;
        }
        let view = self.project_node(node, &snapshot, &mut drafts)?;
        // 业务错误保留可操作的旧界面；公开快照同时保留类型化失败。
        Ok(if let Some(error) = &snapshot.error {
            column(vec![view, label(error.to_string()).automation_id(format!("{}/error", snapshot.name))])
        } else { view })
    }

    fn project_node(&self, node: &ViewSnapshot, snapshot: &ModuleSnapshot, drafts: &mut Drafts) -> RuntimeResult<ViewNode> {
        let string = |name: &str| -> RuntimeResult<String> {
            node.properties.get(name).ok_or_else(|| RuntimeError::new(ErrorKind::InvalidModule, "界面属性缺失"))?.as_str().map(str::to_string)
        };
        let mut view = match node.kind {
            ViewKind::Column | ViewKind::Row => {
                let children = node.children.iter().map(|n| self.project_node(n, snapshot, drafts)).collect::<RuntimeResult<Vec<_>>>()?;
                let mut view = if node.kind == ViewKind::Column { column(children) } else { row(children) };
                for (name, value) in &node.properties {
                    if let Value::Float(value) = value {
                        if name == "gap" { view = view.gap(*value as f32); }
                        if name == "padding" { view = view.padding(*value as f32); }
                    }
                }
                view
            }
            ViewKind::Text => label(string("text")?),
            ViewKind::Button => {
                let disabled = node.properties.get("disabled").ok_or_else(|| RuntimeError::new(ErrorKind::InvalidModule, "按钮缺少 disabled"))?.as_bool()?;
                let handle = self.handle.clone();
                let key = node.key.clone();
                let generation = snapshot.generation;
                button(string("text")?).disabled(disabled).on_click_fn(move || {
                    // 投递与执行失败均保存在 ModuleHandle::snapshot，回执可丢弃。
                    drop(handle.event(&key, generation, vec![]));
                }).build()
            }
            ViewKind::Input => {
                let value = string("value")?;
                let draft = drafts.inputs.entry(node.key.clone()).or_insert_with(|| Draft {
                    value: State::new(value.clone()), pending: Arc::new(AtomicU64::new(0)),
                });
                // 已确认序号之前的快照不得覆盖正在输入的更新草稿。
                if snapshot.sequence >= draft.pending.load(Ordering::Acquire) && draft.value.get() != value { draft.value.set(value); }
                let handle = self.handle.clone();
                let key = node.key.clone();
                let generation = snapshot.generation;
                let pending = draft.pending.clone();
                input().value(&draft.value).placeholder(string("placeholder")?).on_change(move |text| {
                    let event = Value::Record(BTreeMap::from([("value".into(), Value::String(text.to_string()))]));
                    let sequence = match handle.event(&key, generation, vec![event]) {
                        Ok(request) => request.sequence,
                        // 队列失败已经可见；保留本地文字，允许下一次输入重新提交。
                        Err(_) => u64::MAX,
                    };
                    pending.store(sequence, Ordering::Release);
                }).build()
            }
        };
        view.key = Some(node.key.clone());
        view.automation_id = Some(format!("{}/{}", snapshot.name, node.key));
        Ok(view)
    }
}
