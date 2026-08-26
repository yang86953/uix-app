// 引入生产绘制边界使用的恢复辅助函数与运行时节点类型。
use super::{AccessibilityOverride, BoxedWidget, run_with_unwind_restore};
// 引入声明节点与协调中间节点，覆盖完整节点流水线的尺寸基线。
use crate::ui::view::ViewNode;
use crate::ui::widget_runtime::widget::WidgetNode;
// 引入间距与字号令牌查询契约。
use crate::ui::theme::traits::{ISpacingTokens, ITypographyTokens};
// 引入真实主题作用域与令牌补丁类型。
use crate::ui::theme::{ScopedThemeTokens, Theme, TokenPatch};
// 构造只声明类型级浮层能力的最小测试组件。
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities};
// 引入链式无障碍覆盖与基准快照类型。
use crate::ui::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState};
// 引入补丁共享所有权类型。
use std::sync::Arc;

// 用布尔值模拟组件类型级浮层能力声明。
struct OverlayCapabilityWidget(bool);

impl Widget for OverlayCapabilityWidget {
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

    fn may_produce_overlay(&self) -> bool {
        self.0
    }
}

// 用布尔值模拟手写组件的事件布局请求能力声明。
struct EventLayoutCapabilityWidget(bool);

impl Widget for EventLayoutCapabilityWidget {
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

    fn may_request_event_layout(&self) -> bool {
        self.0
    }
}

// 用布尔值模拟手写组件的扩展事件收尾能力声明。
struct ExtendedEventFinishCapabilityWidget(bool);

impl Widget for ExtendedEventFinishCapabilityWidget {
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

    fn requires_extended_event_finish(&self) -> bool {
        self.0
    }
}

crate::widget! {
    // 用真实 widget! 展开验证 take_layout_request 槽位会生成精确能力。
    MacroLayoutRequestWidget {
        requested: bool,
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
        _tree: &crate::ui::widget_runtime::widget::WidgetTree
    ) {}

    take_layout_request => (&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }
}

crate::widget! {
    // 用真实 widget! 展开验证语义槽位会保留扩展事件收尾。
    MacroSemanticFollowupWidget {
        marker: bool,
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
        _tree: &crate::ui::widget_runtime::widget::WidgetTree
    ) {
        let _ = self.marker;
    }

    semantic_event => (
        &self,
        _id: crate::core::WidgetId,
        _event: &crate::ui::event::SystemEvent
    ) -> Option<crate::ui::event::SemanticEvent> {
        None
    }
}

// 确认组件 panic 不会把临时令牌泄漏给后续兄弟节点。
#[test]
// 执行 TokenScope panic 展开回归。
fn panic_during_widget_scope_restores_sibling_tokens() {
    // 创建兄弟节点正常使用的根主题令牌。
    let root = Theme::antd_light().tokens_arc();
    // 记录根主题字号，作为 panic 后的期望值。
    let root_font_size = root.font_size();
    // 记录根主题背景模糊半径，作为 panic 后的期望值。
    let root_backdrop_blur_radius = root.backdrop_blur_radius();
    // 创建与 PaintContext 相同的作用域令牌容器。
    let mut tokens = ScopedThemeTokens::new(root);
    // 安装仅对当前组件生效的临时字号补丁。
    let previous_scope = tokens.replace_scope(
        // 保持根主题不变。
        None,
        // 设置一个与根主题明显不同的组件字号。
        Some(Arc::new(TokenPatch {
            // 使用独立字号辨认临时作用域。
            font_size: Some(31.0),
            // 使用独立背景模糊半径验证新增补丁字段。
            backdrop_blur_radius: Some(19.0),
            // 其余令牌继续继承根主题。
            ..TokenPatch::default() // 结束临时补丁构造。
        })),
        // 保存进入组件前的作用域快照。
    );
    // 在测试外层捕获生产辅助函数继续传播的 panic。
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 复用 BoxedWidget::render 的同一恢复边界。
        run_with_unwind_restore(
            // 传入临时令牌容器。
            &mut tokens,
            // 模拟组件读取补丁后在 render 中 panic。
            |tokens| -> () {
                // 确认 panic 前组件确实看到临时字号。
                assert_eq!(tokens.font_size(), 31.0);
                // 确认 panic 前组件确实看到临时背景模糊半径。
                assert_eq!(tokens.backdrop_blur_radius(), 19.0);
                // 模拟组件绘制失败。
                panic!("widget render panic");
                // 结束模拟组件操作。
            },
            // 使用进入组件前的快照恢复根作用域。
            |tokens| tokens.restore_scope(previous_scope),
            // 结束生产恢复边界调用。
        );
        // 结束外层 panic 捕获。
    }));
    // 确认组件 panic 仍按原语义向上传播。
    assert!(caught.is_err());
    // 确认后续兄弟节点重新读取根主题字号。
    assert_eq!(tokens.font_size(), root_font_size);
    // 确认后续兄弟节点重新读取根主题背景模糊半径。
    assert_eq!(tokens.backdrop_blur_radius(), root_backdrop_blur_radius);
    // 结束 panic 展开回归。
}

// 锁定稀疏元数据优化后的节点尺寸上限。
#[test]
// 防止后续把大覆盖对象或带容量字段的只读列表重新内联进节点。
fn runtime_node_sparse_metadata_stays_compact() {
    // ProviderContext 只能保存一个共享快照句柄，不能重新内联完整配置与语言表。
    assert_eq!(
        std::mem::size_of::<crate::ui::widget_runtime::provider_context::ProviderContext>(),
        std::mem::size_of::<usize>()
    );
    // 空覆盖只占一个可空指针，而不是完整覆盖对象。
    assert!(
        std::mem::size_of::<Option<Box<AccessibilityOverride>>>()
            < std::mem::size_of::<AccessibilityOverride>()
    );
    // Linux 64 位是当前可实测基线；其他平台保留上面的结构契约。
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    {
        // 共享 ProviderContext 前为 3984 字节。
        assert!(std::mem::size_of::<BoxedWidget>() <= 768);
        // 共享 ProviderContext 前为 4080 字节。
        assert!(std::mem::size_of::<ViewNode>() <= 864);
        // 共享 ProviderContext 前为 3664 字节。
        assert!(std::mem::size_of::<WidgetNode>() <= 448);
    }
}

// 类型级浮层能力必须复用现有位集，并在实际组件替换时同步刷新。
#[test]
fn overlay_capability_cache_tracks_widget_replacement() {
    let mut node = BoxedWidget::new(Box::new(OverlayCapabilityWidget(true)));
    assert!(node.may_produce_overlay());

    node.replace_widget(Box::new(OverlayCapabilityWidget(false)));
    assert!(!node.may_produce_overlay());

    node.replace_widget(Box::new(OverlayCapabilityWidget(true)));
    assert!(node.may_produce_overlay());
}

// 事件布局请求能力必须精确覆盖 widget!，并在手写组件替换时刷新。
#[test]
fn event_layout_request_capability_is_cached_without_expanding_nodes() {
    let mut generated = BoxedWidget::new(Box::new(MacroLayoutRequestWidget { requested: true }));
    assert!(
        generated
            .capabilities()
            .contains(WidgetCapabilities::MAY_REQUEST_EVENT_LAYOUT)
    );
    assert!(generated.take_layout_request());
    assert!(!generated.take_layout_request());
    assert!(!generated.requires_extended_event_finish());

    let semantic = BoxedWidget::new(Box::new(MacroSemanticFollowupWidget { marker: true }));
    assert!(semantic.requires_extended_event_finish());

    let virtual_scroll = BoxedWidget::new(Box::new(crate::ui::VirtualScroll::new()));
    assert!(virtual_scroll.requires_extended_event_finish());

    let container = BoxedWidget::new(Box::new(crate::ui::widgets::containers::Container::new()));
    assert!(
        !container
            .capabilities()
            .contains(WidgetCapabilities::MAY_REQUEST_EVENT_LAYOUT)
    );
    assert!(!container.requires_extended_event_finish());

    let button = BoxedWidget::new(Box::new(crate::ui::widgets::general::Button::new("确认")));
    assert!(
        !button
            .capabilities()
            .contains(WidgetCapabilities::MAY_REQUEST_EVENT_LAYOUT)
    );
    assert!(!button.requires_extended_event_finish());

    let mut node = BoxedWidget::new(Box::new(EventLayoutCapabilityWidget(false)));
    assert!(
        !node
            .capabilities()
            .contains(WidgetCapabilities::MAY_REQUEST_EVENT_LAYOUT)
    );
    node.replace_widget(Box::new(EventLayoutCapabilityWidget(true)));
    assert!(
        node.capabilities()
            .contains(WidgetCapabilities::MAY_REQUEST_EVENT_LAYOUT)
    );

    let mut followup = BoxedWidget::new(Box::new(ExtendedEventFinishCapabilityWidget(false)));
    assert!(!followup.requires_extended_event_finish());
    followup.replace_widget(Box::new(ExtendedEventFinishCapabilityWidget(true)));
    assert!(followup.requires_extended_event_finish());

    let conservative = BoxedWidget::new(Box::new(OverlayCapabilityWidget(false)));
    assert!(conservative.requires_extended_event_finish());
}

// 确认按需装箱没有改变连续覆盖的合并语义。
#[test]
// 覆盖 role、name、state 与扩展属性并复用同一分配。
fn boxed_accessibility_override_preserves_chained_updates() {
    // 构造普通标签并连续声明全部覆盖维度。
    let node = ViewNode::leaf(crate::ui::widgets::general::Label::new("原名称"))
        .role(AccessibilityRole::Button)
        .accessible_name("新名称")
        .accessibility_state(AccessibilityState::disabled(true))
        .aria("aria-description", "说明");
    // 对组件派生的基准语义应用声明覆盖。
    let merged = node
        .accessibility_override
        .as_deref()
        .expect("链式声明应创建无障碍覆盖")
        .apply(AccessibilitySnapshot::named(
            AccessibilityRole::Text,
            "原名称",
        ));
    // 确认四个维度均保持既有覆盖结果。
    assert_eq!(merged.role, AccessibilityRole::Button);
    assert_eq!(merged.name.as_deref(), Some("新名称"));
    assert!(merged.state.disabled);
    assert!(
        merged
            .attributes
            .iter()
            .any(|item| item.name == "aria-description" && item.value == "说明")
    );
}
