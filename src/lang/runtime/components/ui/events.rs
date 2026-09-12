//! 原生树尚未协调时，同帧后续事件仍使用该树的捕获；移除的槽立即撤销，不能复活。
use super::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PathStep {
    Field(String),
    Index(usize),
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Slot {
    node: Identity,
    export: ExportKey,
    path: Vec<PathStep>,
}
#[derive(Default)]
pub(super) struct MountedEvents(BTreeMap<EventToken, (Slot, Arc<Callback>)>);
impl MountedEvents {
    pub(super) fn capture(engine: &Engine) -> RuntimeResult<Self> {
        slots(engine.snapshot())
            .into_iter()
            .map(|(token, slot)| Ok((token, (slot, engine.mounted_event(token)?))))
            .collect::<RuntimeResult<BTreeMap<_, _>>>()
            .map(Self)
    }
    pub(super) fn get(&self, token: EventToken) -> RuntimeResult<Arc<Callback>> {
        self.0
            .get(&token)
            .map(|(_, callback)| callback.clone())
            .ok_or_else(|| {
                RuntimeError::new(ErrorKind::Conflict, "原生事件挂载槽已移除或由新树替换")
            })
    }
    pub(super) fn retain_live(&mut self, snapshot: &Snapshot) {
        let live: BTreeSet<_> = slots(snapshot).into_values().collect();
        self.0.retain(|_, (slot, _)| live.contains(slot));
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
}
fn slots(snapshot: &Snapshot) -> BTreeMap<EventToken, Slot> {
    let mut output = BTreeMap::new();
    nodes(&snapshot.roots, &mut output);
    output
}
fn nodes(nodes: &[NativeNode], output: &mut BTreeMap<EventToken, Slot>) {
    for node in nodes {
        let mut slot = Slot {
            node: node.identity.clone(),
            export: node.export.clone(),
            path: Vec::new(),
        };
        for (name, property) in &node.properties {
            slot.path.push(PathStep::Field(name.clone()));
            value(property, &mut slot, output);
            slot.path.pop();
        }
    }
}
fn value(property: &ProjectedValue, slot: &mut Slot, output: &mut BTreeMap<EventToken, Slot>) {
    match property {
        ProjectedValue::Data(_) => {}
        ProjectedValue::Event(token) => {
            output.insert(*token, slot.clone());
        }
        ProjectedValue::View(children) => nodes(children, output),
        ProjectedValue::Array(children) => {
            for (index, child) in children.iter().enumerate() {
                slot.path.push(PathStep::Index(index));
                value(child, slot, output);
                slot.path.pop();
            }
        }
        ProjectedValue::Record(children) => {
            for (name, child) in children {
                slot.path.push(PathStep::Field(name.clone()));
                value(child, slot, output);
                slot.path.pop();
            }
        }
    }
}
