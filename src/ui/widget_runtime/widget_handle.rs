use std::sync::{Mutex, Weak as SyncWeak};

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::rc::{Rc, Weak as RcWeak};

use crate::core::WidgetId;
use crate::ui::event::SemanticEvent;
use crate::ui::widget_runtime::app_state::AppStateInner;
use crate::ui::widget_runtime::widget::EventResult;
#[cfg(test)]
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::widget_snapshot::{
    AccessibilitySnapshot, AriaAttribute, WidgetSnapshotFields, WidgetConfigSnapshot,
};

#[derive(Clone)]
/// 不延长组件树或应用状态生命周期的弱组件访问句柄。
pub struct WidgetHandle {
    id: WidgetId,
    #[cfg(test)]
    tree: Option<RcWeak<RefCell<WidgetTree>>>,
    app_state: Option<SyncWeak<Mutex<AppStateInner>>>,
}

impl WidgetHandle {
    pub(crate) fn from_app_state(id: WidgetId, app_state: SyncWeak<Mutex<AppStateInner>>) -> Self {
        Self {
            id,
            #[cfg(test)]
            tree: None,
            app_state: Some(app_state),
        }
    }

    /// 返回此句柄指向的稳定组件身份。
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// 返回所属运行时仍存在且组件身份仍可解析时是否为真。
    pub fn is_alive(&self) -> bool {
        #[cfg(test)]
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if tree
                .try_borrow()
                .is_ok_and(|tree| tree.get(self.id).is_some())
            {
                return true;
            }
        }
        self.app_state
            .as_ref()
            .and_then(SyncWeak::upgrade)
            .is_some_and(|state| {
                state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains(self.id)
            })
    }

    /// 返回当前完整组件配置快照。
    pub fn snapshot(&self) -> Option<WidgetConfigSnapshot> {
        self.map_snapshot(Clone::clone)
    }

    fn map_snapshot<R>(&self, map: impl Fn(&WidgetConfigSnapshot) -> R) -> Option<R> {
        #[cfg(test)]
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if let Ok(tree) = tree.try_borrow() {
                if let Some(node) = tree.get(self.id) {
                    let snapshot = node.widget_snapshot(self.id);
                    return Some(map(&snapshot));
                }
            }
        }

        self.app_state
            .as_ref()
            .and_then(SyncWeak::upgrade)
            .and_then(|state| {
                state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .map_snapshot(self.id, map)
            })
    }

    /// 返回当前组件类型专属的快照字段。
    pub fn snapshot_fields(&self) -> Option<WidgetSnapshotFields> {
        self.map_snapshot_fields(Clone::clone)
    }

    fn map_snapshot_fields<R>(&self, map: impl Fn(&WidgetSnapshotFields) -> R) -> Option<R> {
        self.map_snapshot(|snapshot| map(&snapshot.fields))
    }

    /// 返回当前组件的无障碍快照。
    pub fn accessibility(&self) -> Option<AccessibilitySnapshot> {
        self.map_snapshot(WidgetConfigSnapshot::accessibility)
    }

    /// 返回当前组件解析后的 ARIA role 名称。
    pub fn aria_role(&self) -> Option<&'static str> {
        self.accessibility()?.aria_role()
    }

    /// 返回当前组件解析后的 ARIA 属性集合。
    pub fn aria_attributes(&self) -> Option<Vec<AriaAttribute>> {
        self.accessibility()
            .map(|accessibility| accessibility.aria_attributes())
    }

    /// 返回按钮或标签组件的显示文本。
    pub fn text(&self) -> Option<String> {
        self.map_snapshot_fields(WidgetSnapshotFields::text).flatten()
    }

    /// 返回按钮或标签组件的显示文本，等价于 [`Self::text`]。
    pub fn label(&self) -> Option<String> {
        self.text()
    }

    /// 返回支持占位文本的输入或选择组件当前占位内容。
    pub fn placeholder(&self) -> Option<String> {
        self.map_snapshot_fields(WidgetSnapshotFields::placeholder).flatten()
    }

    /// 返回支持禁用状态的组件当前是否禁用。
    pub fn disabled(&self) -> Option<bool> {
        self.map_snapshot_fields(WidgetSnapshotFields::disabled).flatten()
    }

    /// 返回复选框或开关组件当前是否选中。
    pub fn checked(&self) -> Option<bool> {
        self.map_snapshot_fields(WidgetSnapshotFields::checked).flatten()
    }

    /// 返回支持数值快照的组件当前数值。
    pub fn numeric_value(&self) -> Option<f64> {
        self.map_snapshot_fields(WidgetSnapshotFields::numeric_value).flatten()
    }

    /// 请求此组件重绘，并在应用运行时中唤醒事件循环。
    pub fn invalidate(&self) {
        #[cfg(test)]
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            if let Ok(mut tree) = tree.try_borrow_mut() {
                tree.invalidate_paint(self.id);
            };
            return;
        }

        if let Some(state) = self.app_state.as_ref().and_then(SyncWeak::upgrade) {
            let waker = state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .invalidate(self.id);
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    /// 以此组件为目标派发语义事件；目标不可用时返回未处理。
    pub fn emit(&self, mut event: SemanticEvent) -> EventResult {
        #[cfg(test)]
        if let Some(tree) = self.tree.as_ref().and_then(RcWeak::upgrade) {
            let Ok(tree_ref) = tree.try_borrow() else {
                return EventResult::NotHandled;
            };
            if tree_ref.get(self.id).is_none() {
                return EventResult::NotHandled;
            }
            drop(tree_ref);

            event.target = self.id;
            event.current_target = self.id;
            return match tree.try_borrow_mut() {
                Ok(mut tree) => tree.dispatch_semantic(event),
                // 树正处于可变借用（布局/动画事务）时丢弃本次语义分发并回退
                // NotHandled：重入传播会破坏树事务，降级是既定安全语义。
                Err(_) => EventResult::NotHandled,
            };
        }

        if let Some(state) = self.app_state.as_ref().and_then(SyncWeak::upgrade) {
            event.target = self.id;
            event.current_target = self.id;
            let waker = state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .emit_semantic_event(self.id, event);
            if let Some(waker) = waker {
                waker.wake();
                return EventResult::Handled;
            }
        }

        EventResult::NotHandled
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/ui/widget_runtime/widget_handle_tests.rs"]
mod widget_handle_tests;