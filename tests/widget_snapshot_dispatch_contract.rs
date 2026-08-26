//! 验证自定义组件快照快路与内置类型化快照保持各自契约。

use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use uix::prelude::{Button, Container, Grid, Input, Label};
use uix::ui::__private::traits::{Widget, WidgetCapabilities};
use uix::ui::SnapshotFields;

// 记录默认快照路径是否为未知组件读取动态类型。
static AS_ANY_CALLS: AtomicUsize = AtomicUsize::new(0);

struct CustomWidget;

impl Widget for CustomWidget {
    fn as_any(&self) -> &dyn Any {
        AS_ANY_CALLS.fetch_add(1, Ordering::Relaxed);
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::new()
    }
}

#[test]
fn unknown_custom_widget_snapshot_skips_builtin_type_dispatch() {
    AS_ANY_CALLS.store(0, Ordering::Relaxed);
    assert_eq!(CustomWidget.snapshot_fields(), SnapshotFields::Unknown);
    assert_eq!(AS_ANY_CALLS.load(Ordering::Relaxed), 0);
}

#[test]
fn builtin_widgets_keep_typed_snapshot_fields() {
    let widgets: [&dyn Widget; 5] = [
        &Button::new("确认"),
        &Label::new("标签"),
        &Input::new("输入"),
        &Container::new(),
        &Grid::new(),
    ];
    for widget in widgets {
        assert_ne!(widget.snapshot_fields(), SnapshotFields::Unknown);
    }
}
