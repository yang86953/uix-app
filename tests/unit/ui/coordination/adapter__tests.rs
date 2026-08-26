use super::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ui::view::ViewNode;
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities};
use crate::ui::widget_snapshot::SnapshotFields;
use crate::ui::widgets::Button;

// 记录协调期间新版组件收到的快照请求次数。
struct SnapshotCountingWidget {
    calls: Arc<AtomicUsize>,
    fields: SnapshotFields,
}

impl SnapshotCountingWidget {
    // 复用真实 Button 快照字段，分别构造 enabled 与 disabled 语义。
    fn new(disabled: bool, calls: Arc<AtomicUsize>) -> Self {
        let button = Button::new("snapshot probe").disabled(disabled);
        Self {
            calls,
            fields: button.snapshot_fields(),
        }
    }
}

impl Widget for SnapshotCountingWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::new()
    }

    fn snapshot_fields(&self) -> SnapshotFields {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.fields.clone()
    }
}

// 构造同类型根并协调一次，返回新版组件的快照计数器。
fn reconcile_probe(disabled: bool) -> Arc<AtomicUsize> {
    let old_calls = Arc::new(AtomicUsize::new(0));
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(SnapshotCountingWidget::new(
        false, old_calls,
    )));
    let next_calls = Arc::new(AtomicUsize::new(0));
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(SnapshotCountingWidget::new(
            disabled,
            Arc::clone(&next_calls),
        )),
    );
    next_calls
}

// enabled 稳态必须复用事件清理前已生成的单份新版快照。
#[test]
fn enabled_reconcile_reuses_precomputed_snapshot() {
    let calls = reconcile_probe(false);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

// disabled 路径必须在取消阶段后由 patch 再取一次新版快照；本测试只验证次数，
// 未额外构造 Pointer/Focus 回调，因此事件回调顺序仍由现有路径测试覆盖。
#[test]
fn disabled_reconcile_refreshes_snapshot_after_cleanup() {
    let calls = reconcile_probe(true);

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
