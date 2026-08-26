//! ViewAdapter - expands a View tree into a WidgetTree.
//! `App::run()` uses this module to recursively expand user-authored `View`
//! trees into framework `WidgetTree` nodes, keeping `WidgetNode` and
//! `BoxedWidget` internal.
//! # Responsibilities
//! 1. `ViewAdapter::build(root)` captures view context and builds a `WidgetTree`.
//! 2. `expand(node)` iteratively converts `ViewNode` into `WidgetNode`.
//! 3. `apply_style(widget, style)` applies declarative style to concrete widgets.
//! # State Binding
//! - During view build, `begin_state_capture` records `State::new` instances.
//! - After layout, `bind_reactive_widget_states` detects dynamic label closure
//!   dependencies and binds them to narrow Paint invalidation.
//! - 根捕获输出由 `replace_root_captured_state_binds` 显式绑定到所属树的 reconcile
//!   请求端口（结构性 View 更新）。
//! - render 期读取的 State / Computed 绑定窄 Paint；DynamicLabel 还会在 layout 后
//!   主动探测闭包依赖。
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
use crate::ui::event::system_event_handler::SystemEventHandlerRegistration;
use crate::ui::event::{HandlerRegistration, HandlerSignature, SemanticKind};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::theme::style::Style;
use crate::ui::view::ViewNode;
use crate::ui::widget_patch::{
    builtin_widget_config_changed, builtin_widget_layout_changed, builtin_widget_runtime_changed,
    patch_builtin_widget,
};
use crate::ui::widget_runtime::focus_handle::FocusHandle;
use crate::ui::widget_runtime::traits::Widget;
use crate::ui::widget_runtime::widget::{WidgetCore, WidgetNode};
use crate::ui::widget_snapshot::SnapshotFields;
// 引入动态子树组件以识别各自的私有协调边界。
use crate::ui::widgets::{Calendar, Carousel, Image, Transfer};
// 导航 capability 启用时才识别 Anchor 的专属动态容器协调边界。
#[cfg(feature = "navigation")]
use crate::ui::widgets::navigation::Anchor;
use crate::ui::{WidgetId, WidgetTree};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
// 复用独立生命周期模块，保持适配器主体低于文件规模上限。
#[path = "capture_guards.rs"]
// 编译捕获守卫与回执转移的私有实现模块。
mod capture_guards;
// 仅向 UI 内部 renderer 边界重导出不可伪造的动态捕获能力。
pub(crate) use capture_guards::DynamicViewCaptureContext;
// 拆分交错入场计算，保持适配器主体在文件规模约束内。
#[path = "stagger.rs"]
// 编译交错入场配置的私有实现模块。
mod stagger;
// 拆分动态子树协调的事务边界，保持适配器主体在文件规模约束内。
#[path = "dynamic_reconcile.rs"]
// 编译动态子树协调事务的私有实现模块。
mod dynamic_reconcile;
// 拆分根协调的发布边界，避免适配器主体超过文件规模上限。
#[path = "coordination.rs"]
// 编译根构建与协调的私有事务入口。
mod coordination;
// 拆分具体组件样式适配，保持协调主体低于文件规模上限。
#[path = "style.rs"]
// 编译统一 Style 到 widgets 私有配置的单向适配入口。
mod style;
/// 声明期 View 子节点能力端口（System 私有边界）。
///
/// `widget → view` 依赖环消除（SMC-04）：`build_view_children` 从
/// `Widget` 移出，由本边界 trait 承载；树构建经 `as_view_children`
/// 上转型消费，widgets 经 `widget!` 宏实现。
pub trait ViewChildrenProvider: Widget {
    /// 构建由组件声明并交给运行时树展开的 View 子节点。
    fn build_view_children(&self) -> Vec<ViewNode>;
}
/// 读取组件的声明期 View 子节点（无端口时为空）。
pub(crate) fn view_children(widget: &dyn Widget) -> Vec<ViewNode> {
    widget
        .as_view_children()
        .map(|provider| provider.build_view_children())
        .unwrap_or_default()
}
/// View tree adapter.
pub(crate) struct ViewAdapter;
/// 组件原位 patch 对后续流水线的精细失效影响。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WidgetPatchImpact {
    /// 组件绘制输出是否变化。
    paint_changed: bool,
    /// 组件测量或布局输出是否变化。
    layout_changed: bool,
}

// 为 keyed 协调生成稳定摘要；索引与无申请唯一性快路共享一次实现。
fn reconcile_key_hash(key: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

// 为摘要索引应用测试期位宽；命中后仍精确比较原字符串。
fn reconcile_key_fingerprint(key: &str) -> u64 {
    let fingerprint = reconcile_key_hash(key);
    // 测试宿主可收窄摘要以确定性覆盖碰撞回退，生产构建保留完整 64 位。
    #[cfg(feature = "test-harness")]
    {
        return fingerprint
            & RECONCILE_KEY_FINGERPRINT_MASK.load(std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(not(feature = "test-harness"))]
    fingerprint
}

// 用栈上摘要位图过滤常见唯一 key；位冲突时精确回看，绝不把重复 key 误判为唯一。
// 独立栈帧保证 4 KiB 位图在递归协调子树前释放，不随声明深度叠加。
#[inline(never)]
fn reconcile_keys_are_unique(children: &[ViewNode]) -> bool {
    // 32768 位只占 4 KiB 栈空间，512 个随机摘要通常仅需少量精确回看。
    let mut seen = [0_u64; 512];
    for (index, child) in children.iter().enumerate() {
        let Some(key) = child.key.as_deref() else {
            return false;
        };
        let fingerprint = reconcile_key_hash(key);
        let word = fingerprint as usize & (seen.len() - 1);
        let bit = 1_u64 << ((fingerprint >> 9) & 63);
        if seen[word] & bit != 0
            && children[..index]
                .iter()
                .any(|previous| previous.key.as_deref() == Some(key))
        {
            return false;
        }
        seen[word] |= bit;
    }
    true
}

// 摘要已经完成带密钥内容哈希，表索引只需原样接纳 u64，避免二次 SipHash。
#[derive(Default)]
struct ReconcileFingerprintHasher(u64);

impl Hasher for ReconcileFingerprintHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // HashMap 当前只写入 u64；保留通用回退以满足 Hasher 完整契约。
        let mut folded = 0_u64;
        for (index, byte) in bytes.iter().copied().enumerate() {
            folded ^= u64::from(byte) << ((index & 7) * 8);
        }
        self.0 = folded;
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

type ReconcileFingerprintMap =
    HashMap<u64, WidgetId, std::hash::BuildHasherDefault<ReconcileFingerprintHasher>>;

// 只影响 test-harness 进程内的摘要宽度，不进入默认生产构建。
#[cfg(feature = "test-harness")]
static RECONCILE_KEY_FINGERPRINT_MASK: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(u64::MAX);

// 返回旧掩码，供测试守卫在碰撞场景结束后恢复进程状态。
#[cfg(feature = "test-harness")]
pub(crate) fn set_reconcile_key_fingerprint_mask_for_test(mask: u64) -> u64 {
    RECONCILE_KEY_FINGERPRINT_MASK.swap(mask, std::sync::atomic::Ordering::SeqCst)
}

impl ViewAdapter {
    /// Expands a ViewNode tree into a WidgetNode tree using explicit stack
    /// traversal to avoid stack overflow on deep trees in debug builds.
    pub(crate) fn expand(root: ViewNode) -> WidgetNode {
        // Decompose a ViewNode to keep traversal state on the heap.
        struct Frame {
            widget: Box<dyn Widget>,
            style: Style,
            // 保存非根声明节点交接给运行时树的 State 绑定。
            captured_state_binds:
                Vec<std::sync::Arc<dyn crate::ui::reactive::state::StatePaintBind>>,
            // 保存非根声明节点交接给运行时节点的 Effect。
            captured_effects: Vec<crate::ui::reactive::state::Effect>,
            // 保存非根或动态声明节点交接给所属节点的动画源。
            animated_sources: Vec<std::sync::Arc<dyn crate::ui::animation::AnimatedSource>>,
            visual_transform: crate::ui::widget_runtime::view_transform::ViewTransform,
            // 保存声明节点的完整定位元数据。
            position: crate::ui::position::PositionedLayout,
            // 保存声明节点的文字选择策略。
            user_select: crate::ui::UserSelect,
            // 保存声明节点可继承的指针光标覆盖。
            cursor: Option<crate::platform::windowing::CursorType>,
            enter_animation: Option<crate::ui::animation::AnimationConfig>,
            enter_deadline: Option<std::time::Instant>,
            leave_animation: Option<crate::ui::animation::AnimationConfig>,
            flex_grow_override: Option<f32>,
            flex_shrink_override: Option<f32>,
            provider_context: crate::ui::widget_runtime::provider_context::ProviderContext,
            visible: bool,
            z_index: i32,
            key: Option<String>,
            automation_id: Option<String>,
            tab_index: Option<i32>,
            focus_handle: Option<FocusHandle>,
            // 展开栈只移动稀疏覆盖的指针，不复制其大对象。
            accessibility_override: Option<Box<AccessibilityOverride>>,
            handlers: Vec<HandlerRegistration>,
            system_event_handlers: Vec<SystemEventHandlerRegistration>,
            render_handlers: Vec<RenderHandlerRegistration>,
            uix_widget_scopes: Vec<crate::ui::widget_state::UixWidgetScopeMarker>,
            remaining_children: std::vec::IntoIter<ViewNode>,
            processed_children: Vec<WidgetNode>,
        }
        fn decompose(node: ViewNode) -> Frame {
            let ViewNode {
                widget,
                mut children,
                // 把非根与动态子树的 State 输出继续传递到运行时节点。
                captured_state_binds,
                // 把非根与动态子树的 Effect 输出继续传递到运行时节点。
                captured_effects,
                // 把非根与动态子树的动画源继续传递到运行时节点。
                animated_sources,
                provider_context,
                style,
                visual_transform,
                position,
                user_select,
                cursor,
                enter_animation,
                enter_deadline,
                leave_animation,
                stagger_enter,
                flex_grow_override,
                flex_shrink_override,
                z_index,
                key,
                automation_id,
                tab_index,
                focus_handle,
                accessibility_override,
                handlers,
                system_event_handlers,
                render_handlers,
                uix_widget_scopes,
                // 回执已经在建树或协调入口转移，展开不能提前提交或回滚它们。
                widget_state_receipts: _,
                // 捕获根的状态存储已经在建树入口接管，子树展开不再传播。
                widget_state_store: _,
            } = node;
            let anchor = std::time::Instant::now();
            for (rank, child) in children.iter_mut().enumerate() {
                ViewAdapter::configure_staggered_child(child, stagger_enter, anchor, rank);
            }
            let visible = style.visible;
            Frame {
                widget,
                style,
                // 保留本声明节点的结构性 State 输出。
                captured_state_binds,
                // 保留本声明节点的 Effect 输出。
                captured_effects,
                // 保留本声明节点拥有的动画源输出。
                animated_sources,
                visual_transform,
                position,
                user_select,
                cursor,
                enter_animation,
                enter_deadline,
                leave_animation,
                flex_grow_override,
                flex_shrink_override,
                provider_context,
                visible,
                z_index,
                key,
                automation_id,
                tab_index,
                focus_handle,
                accessibility_override,
                handlers,
                system_event_handlers,
                render_handlers,
                // 把声明根的完整组件作用域链交给 WidgetNode 构建阶段。
                uix_widget_scopes,
                remaining_children: children.into_iter(),
                processed_children: Vec::new(),
            }
        }
        fn build_widget(frame: Frame) -> WidgetNode {
            let widget = ViewAdapter::apply_style(
                frame.widget,
                &frame.style,
                frame.flex_grow_override,
                frame.flex_shrink_override,
            );
            let mut wnode = if frame.processed_children.is_empty() {
                WidgetNode::leaf(widget)
            } else {
                WidgetNode::new(widget, frame.processed_children)
            };
            if let Some(key) = frame.key {
                wnode = wnode.key(&key);
            }
            if let Some(automation_id) = frame.automation_id {
                wnode = wnode.automation_id(&automation_id);
            }
            if let Some(tab_index) = frame.tab_index {
                wnode = wnode.tab_index(tab_index);
            }
            if let Some(focus_handle) = frame.focus_handle {
                wnode = wnode.with_focus_handle(focus_handle);
            }
            if let Some(accessibility_override) = frame.accessibility_override {
                wnode = wnode.with_accessibility_override(accessibility_override);
            }
            if frame.z_index != 0 {
                wnode = wnode.z_index(frame.z_index);
            }
            if !frame.visible {
                wnode = wnode.with_visibility(false);
            }
            if frame.visual_transform
                != crate::ui::widget_runtime::view_transform::ViewTransform::default()
            {
                wnode = wnode.with_visual_transform(frame.visual_transform);
            }
            // 所有定位值交给实际树统一求解正常流、包含块和视口语义。
            wnode = wnode.with_position(frame.position);
            // 所有值都保留给实际树执行父子 used-value 解析。
            wnode = wnode.with_user_select(frame.user_select);
            // 显式光标覆盖需要随声明节点进入运行时树。
            if let Some(cursor) = frame.cursor {
                // 保留 Arrow 覆盖父节点的语义，不能按默认值吞掉。
                wnode = wnode.with_cursor(cursor);
            }
            if let Some(animation) = frame.enter_animation {
                wnode = wnode.with_enter_animation(animation, frame.enter_deadline);
            }
            if let Some(animation) = frame.leave_animation {
                wnode = wnode.with_leave_animation(animation);
            }
            if !frame.handlers.is_empty() {
                wnode = wnode.with_handlers(frame.handlers);
            }
            if !frame.system_event_handlers.is_empty() {
                wnode = wnode.with_system_event_handlers(frame.system_event_handlers);
            }
            if !frame.render_handlers.is_empty() {
                wnode = wnode.with_render_handlers(frame.render_handlers);
            }
            // 把动态子树捕获的 State 绑定交接给将来拥有该节点的树。
            wnode = wnode.with_captured_state_binds(frame.captured_state_binds);
            // 把动态子树捕获的 Effect 交接给将来拥有该节点的节点生命周期。
            wnode = wnode.with_captured_effects(frame.captured_effects);
            // 把动态子树捕获的动画源交接给将来拥有该节点的树级注册表。
            wnode = wnode.with_animated_sources(frame.animated_sources);

            // 将声明节点的嵌套组件作用域保留到运行时树。
            wnode = wnode.with_uix_widget_scopes(frame.uix_widget_scopes);

            wnode.with_provider_context(frame.provider_context)
        }

        let mut stack: Vec<Frame> = Vec::new();
        let mut current = decompose(root);

        loop {
            if let Some(child) = current.remaining_children.next() {
                stack.push(current);
                current = decompose(child);
                continue;
            }

            let wnode = build_widget(current);
            match stack.pop() {
                Some(mut parent) => {
                    parent.processed_children.push(wnode);
                    current = parent;
                }
                None => return wnode,
            }
        }
    }

    fn reconcile_existing(tree: &mut WidgetTree, id: WidgetId, node: ViewNode) {
        let ViewNode {
            widget,
            children,
            // 接收本节点本轮捕获的结构性 State 输出。
            captured_state_binds,
            // 接收本节点本轮捕获的 Effect 输出。
            captured_effects,
            // 接收本节点本轮捕获的动画源输出。
            animated_sources,
            provider_context,
            style,
            visual_transform,
            position,
            user_select,
            cursor,
            enter_animation: _,
            enter_deadline: _,
            leave_animation,
            stagger_enter,
            flex_grow_override,
            flex_shrink_override,
            z_index,
            key,
            automation_id,
            tab_index,
            focus_handle,
            accessibility_override,
            handlers,
            system_event_handlers,
            render_handlers,
            uix_widget_scopes,
            // 回执已经在建树或协调入口转移，原位协调不能提前提交或回滚它们。
            widget_state_receipts: _,
            widget_state_store: _,
        } = node;
        let context_changed = tree
            .get(id)
            .is_none_or(|current| current.provider_context() != &provider_context);
        if let Some(current) = tree.get_mut(id) {
            current.set_provider_context(provider_context);
            current.set_leave_animation(leave_animation);
            // 原位协调必须同时更新可继承的光标声明。
            current.set_cursor(cursor);
            // 更新非视觉元数据，使现有节点身份与声明根严格一致。
            current.set_uix_widget_scopes(uix_widget_scopes);
            // 用本轮捕获的 Effect 完整替换此节点的旧声明实例。
            current.replace_captured_effects(captured_effects);
        }
        // 将本节点本轮结构性 State 绑定到所属树的 reconcile 请求端口。
        // 根绑定由根所有者整体替换，非根节点才持有节点生命周期租约。
        if tree.root_id() != Some(id) {
            // 把非根节点本轮结构依赖交给实际节点生命周期。
            tree.replace_node_captured_state_binds(id, captured_state_binds);
        }
        if !style.visible {
            tree.set_node_visibility(id, false);
        }
        let widget = Self::apply_style(widget, &style, flex_grow_override, flex_shrink_override);
        // Calendar cells depend on preserved runtime month/selection. Building them from
        // the freshly declared widget here would invoke the factory with stale defaults;
        // reconcile them after `sync_from` has patched the live Calendar instead.
        let calendar_cells = widget
            .as_any()
            .downcast_ref::<Calendar>()
            .is_some_and(Calendar::owns_custom_cell_children)
            || tree.is_calendar_cell_widget(id);
        // Anchor 容器必须在 live owner 完成原位同步后动态捕获，不能调用新声明组件的工厂。
        let anchor_container = {
            // 导航 capability 启用时识别新声明或 live 节点中的 Anchor owner。
            #[cfg(feature = "navigation")]
            {
                // 新声明与原位复用均需进入 Anchor 专属协调边界。
                widget.as_any().is::<Anchor>() || tree.is_anchor_container_widget(id)
            }
            // 导航 capability 关闭时没有 Anchor owner，保留普通子树协调语义。
            #[cfg(not(feature = "navigation"))]
            {
                // 固定为假以避免 feature 关闭时引用导航组件类型。
                false
            }
        };
        // Transfer 条目必须在 live owner 完成原位同步后由所属树动态捕获。
        let transfer_items = widget.as_any().is::<Transfer>() || tree.is_transfer_item_widget(id);
        // Carousel 自定义箭头必须在 live owner patch 后与 authored slides 一次性协调。
        let carousel_custom_arrows = widget.as_any().is::<Carousel>()
            // 原位复用时也识别当前 live Carousel owner。
            || tree.is_carousel_custom_arrows_widget(id);
        // Image 的占位与错误 View 共同属于同一专属动态子树协调边界。
        let image_children = widget.as_any().is::<Image>();
        // 专属 owner 的延迟子树不能再由无树 store 的通用 ViewChildren 入口执行。
        let widget_view_children = if calendar_cells || anchor_container || carousel_custom_arrows {
            Vec::new()
        } else {
            view_children(widget.as_ref())
        };
        let next_accessibility = widget.snapshot_fields().accessibility();
        let next_disabled = accessibility_override
            .as_ref()
            .map(|override_state| override_state.apply(next_accessibility.clone()))
            .unwrap_or(next_accessibility)
            .state
            .disabled;
        if next_disabled {
            // PointerLeave / DragEnd 必须在旧组件仍启用时交付，随后再 patch disabled。
            tree.cancel_pointer_hover_in_subtree(id);
            tree.cancel_pointer_gesture_in_subtree(id);
        }
        if next_disabled
            && tree
                .managers()
                .focus
                .focused_widget()
                .is_some_and(|focused| tree.is_descendant_of(focused, id))
        {
            // 旧组件仍启用时先交付 FocusOut，清理键盘按压和控件视觉；
            // 随后的 patch 才写入 disabled，避免 disabled 早退吞掉清理事件。
            tree.set_focus(None);
        }
        let widget_impact = Self::patch_widget(tree, id, widget);
        // 定位变化需要重排父槽位并重建绘制与命中投影。
        let position_changed = tree.set_node_position(id, position);
        // patch 可能替换具体组件，因此在其后重算子树并同步最终选择策略。
        tree.set_node_user_select(id, user_select);
        if style.visible {
            tree.set_node_visibility(id, true);
        }
        tree.set_tab_index_override(id, tab_index);
        tree.set_focus_handle(id, focus_handle);
        if let Some(current) = tree.get_mut(id) {
            current.set_accessibility_override(accessibility_override);
        }

        let mut paint_changed = widget_impact.paint_changed || context_changed || position_changed;
        let mut layout_changed =
            widget_impact.layout_changed || context_changed || position_changed;
        if tree.set_visual_transform(id, visual_transform) {
            paint_changed = true;
        }
        if let Some(current) = tree.get_mut(id) {
            // 先借用 String 精确比较；只有 key 真变化时才转换为 Box<str>。
            if current.key() != key.as_deref() {
                current.set_key(key.map(Into::into));
            }
            let next_automation_id = automation_id.map(Into::into);
            if current.automation_id() != next_automation_id.as_deref() {
                current.set_automation_id(next_automation_id);
            }
            if current.z_index() != z_index {
                current.set_z_index(z_index);
                paint_changed = true;
            }
        }

        tree.register_app_state_snapshot(id);

        let _handlers_changed = Self::reconcile_handlers(tree, id, handlers);
        tree.replace_system_event_handlers(id, system_event_handlers);
        tree.replace_render_handlers(id, render_handlers);
        // 表格 capability 启用时才协调泛型单元格或扩展行动态子树。
        #[cfg(feature = "table")]
        let table_children_changed = if tree.has_table_cell_renderer(id) {
            Some(tree.refresh_table_cell_widget(id))
        } else if tree.has_table_expand_renderer(id) {
            // 交给动态刷新入口，确保扩展行 receipt 进入独立事务协调。
            Some(tree.refresh_table_expand_widget(id))
        } else {
            None
        };
        // 表格 capability 关闭时不保留专属动态子树协调结果。
        #[cfg(not(feature = "table"))]
        let table_children_changed = None::<bool>;
        let select_options = tree.has_select_option_renderer(id);
        // VirtualScroll 的物化行由专用 keyed 动态协调器拥有。
        let virtual_scroll_items = tree.has_virtual_scroll_renderer(id);
        let collapse_content = tree.is_collapse_content_widget(id);
        // 在 live Image 完成 patch 后捕获错误 View，避免用新声明的默认加载状态覆盖运行时失败。
        let image_error_view = if image_children {
            // 只为当前活跃 Image owner 请求完整的动态捕获输出。
            tree.image_error_view_for_reconcile(id)
        } else {
            // 非 Image 保持既有普通子节点协调语义。
            None
        };
        if select_options {
            tree.invalidate_select_option_widget(id);
        }
        let mut children_changed = if let Some(changed) = table_children_changed {
            changed
        } else if select_options {
            tree.refresh_select_option_widget(id)
        } else if collapse_content {
            tree.refresh_collapse_content_widget(id)
        } else if calendar_cells {
            tree.refresh_calendar_cell_widget(id)
        } else if anchor_container {
            // 导航 capability 启用时由 live Anchor 统一协调 authored 与动态容器。
            #[cfg(feature = "navigation")]
            {
                // live Anchor 已完成 patch，此处才捕获最新工厂并合并 authored children。
                tree.reconcile_anchor_container_widget(id, children)
            }
            // 导航 capability 关闭时该分支不可达，保留穷尽表达式类型。
            #[cfg(not(feature = "navigation"))]
            {
                // 不消费 children，避免 feature 边界外产生动态协调。
                false
            }
        } else if transfer_items {
            // live Transfer 已完成 patch，此处以 pane 与业务 key 协调全部自定义条目。
            tree.refresh_transfer_item_widget(id)
        } else if carousel_custom_arrows {
            // live Carousel 已完成 patch，此处合并 authored slides 与固定自定义箭头。
            tree.reconcile_carousel_custom_arrows_widget(id, children)
        } else if image_children {
            // 先保留本轮 authored 与 fresh placeholder，再追加同一失败实例的完整动态错误 View。
            let mut children = children;
            // placeholder 仍由声明 Image 的一次性 build_view_children 语义提供。
            children.extend(widget_view_children);
            // 运行时错误 View 只在 Image 失败且 handler 启用时参与完整子树协调。
            let has_error_view = image_error_view.is_some();
            // 保持当前错误实例的 State、Effect、动画与 receipt 作为同一嵌套事务交接。
            children.extend(image_error_view);
            // 让 keyed reconcile 复用错误子树而不是父级声明更新时将其删除。
            let changed = Self::reconcile_dynamic_children(tree, id, children);
            // 成功协调后才让 live Image 接纳本轮错误子树物化真相。
            if let Some(image) = tree
                // 节点仍可能因错误工厂 panic 前的外部重入而不可寻址。
                .get(id)
                // 只允许当前运行时 Image 接收物化事实。
                .and_then(|node| node.widget().as_any().downcast_ref::<Image>())
            {
                // 有错误声明时标记已物化，避免 layout 重复执行用户工厂。
                if has_error_view {
                    // keyed 协调已经复用或建立当前错误子树。
                    image.mark_error_view_materialized();
                }
            }
            // 只有固定 key 错误子树已实际缺席时才清理物化标记，pending leave 必须保留。
            let error_child_still_exists = tree.get(id).is_some_and(|node| {
                // 遍历 Image 的实际直接子节点，包括仍在 leave 的墓碑节点。
                node.children().iter().copied().any(|child_id| {
                    // 以运行时 key 判断错误子树实际存在性。
                    tree.get(child_id).and_then(|child| child.key()) == Some(Image::ERROR_CHILD_KEY)
                })
            });
            // 无错误声明且固定 key 子树已完成真实移除时才允许下一次重新捕获。
            if !has_error_view && !error_child_still_exists {
                // 只更新仍属于当前 owner 的 live Image 私有派生状态。
                if let Some(image) = tree
                    // 读取协调后仍存活的父组件。
                    .get(id)
                    // 确认类型未因生命周期回调重入发生变化。
                    .and_then(|node| node.widget().as_any().downcast_ref::<Image>())
                {
                    // 与 on_children_changed 的真实缺席清理语义保持一致。
                    image.clear_error_view_materialized();
                }
            }
            // 向父级报告本轮专属动态协调是否改写结构。
            changed
        } else if virtual_scroll_items {
            // 保留旧物化窗口，随后用新版 renderer 按稳定 key 原位协调。
            false
        } else {
            let mut children = children;
            children.extend(widget_view_children);
            Self::reconcile_children(tree, id, children, stagger_enter)
        };
        children_changed |= tree.refresh_virtual_scroll_widget(id, None);
        if children_changed {
            paint_changed = true;
            layout_changed = true;
        }

        if paint_changed {
            tree.invalidate_paint(id);
        }
        if layout_changed {
            tree.push_layout_invalidation(id);
            tree.propagate_layout_invalidation(id);
        }
        // 本节点及其动态子树协调成功后才替换节点动画源所有权。
        tree.replace_node_animated_sources(id, animated_sources);
    }

    fn reconcile_handlers(
        tree: &mut WidgetTree,
        id: WidgetId,
        handlers: Vec<HandlerRegistration>,
    ) -> bool {
        let next_signatures = tree
            .get(id)
            .map(|current| {
                Self::resolve_handler_signatures(current.handler_signatures(), &handlers)
            })
            .unwrap_or_else(|| {
                handlers
                    .iter()
                    .map(|handler| handler.authored_signature())
                    .collect()
            });
        let changed = tree.get(id).is_none_or(|current| {
            !Self::handler_signatures_are_stable(current.handler_signatures(), &next_signatures)
                || Self::handler_signature_groups(current.handler_signatures())
                    != Self::handler_signature_groups(&next_signatures)
        });
        if !changed {
            return false;
        }

        tree.handler_table().clear_widget(id);
        if let Some(current) = tree.get_mut(id) {
            current.set_handler_signatures(next_signatures);
        }
        for handler in handlers {
            tree.handler_table().register(id, handler);
        }
        true
    }

    fn resolve_handler_signatures(
        current: &[HandlerSignature],
        handlers: &[HandlerRegistration],
    ) -> Vec<HandlerSignature> {
        let mut seen_by_kind = HashMap::new();
        handlers
            .iter()
            .map(|handler| {
                let mut next = handler.signature();
                if next.generation.is_some() || next.capture_fingerprint.is_none() {
                    return next;
                }

                let occurrence = seen_by_kind.entry(next.kind).or_insert(0);
                let current_signature =
                    Self::nth_handler_signature(current, next.kind, *occurrence);
                *occurrence += 1;

                next.generation = Some(match current_signature {
                    Some(current)
                        if current.capture_fingerprint == next.capture_fingerprint
                            && current.generation.is_some() =>
                    {
                        current.generation.unwrap_or(0)
                    }
                    Some(current) => current.generation.unwrap_or(0).saturating_add(1),
                    None => 0,
                });
                next
            })
            .collect()
    }

    fn nth_handler_signature(
        signatures: &[HandlerSignature],
        kind: SemanticKind,
        occurrence: usize,
    ) -> Option<&HandlerSignature> {
        signatures
            .iter()
            .filter(|signature| signature.kind == kind)
            .nth(occurrence)
    }

    fn handler_signature_groups(
        signatures: &[HandlerSignature],
    ) -> HashMap<SemanticKind, Vec<(Option<u32>, crate::ui::event::HandlerOptionsSignature)>> {
        let mut groups = HashMap::new();
        for signature in signatures {
            groups
                .entry(signature.kind)
                .or_insert_with(Vec::new)
                .push((signature.generation, signature.options));
        }
        groups
    }

    fn patch_widget(
        tree: &mut WidgetTree,
        id: WidgetId,
        widget: Box<dyn Widget>,
    ) -> WidgetPatchImpact {
        let Some(current) = tree.get_mut(id) else {
            // 节点已不存在时没有可上报的 patch 影响。
            return WidgetPatchImpact::default();
        };

        // 先判断运行时受控值是否需要同步。
        let runtime_changed = builtin_widget_runtime_changed(current.widget(), widget.as_ref());
        // 在 patch 前只抓取一次当前组件公开快照。
        let current_fields = current.widget().snapshot_fields();
        // 在 patch 前只抓取一次新版组件公开快照。
        let next_fields = widget.snapshot_fields();
        // 排除含运行时字段的特殊快照，再回退到完整快照比较。
        let config_changed = builtin_widget_config_changed(&current_fields, &next_fields)
            .unwrap_or_else(|| current_fields != next_fields)
            || next_fields == SnapshotFields::Unknown
            || runtime_changed;
        // 已审计类型按字段分类，其余类型由显式保守分类请求布局。
        let layout_changed = config_changed
            && builtin_widget_layout_changed(&current_fields, &next_fields).unwrap_or(true);
        // 只有实际完成原位 patch 或替换后才报告失效影响。
        match patch_builtin_widget(current.widget_mut(), widget) {
            // 原位同步成功时返回精细分类结果。
            Ok(true) => WidgetPatchImpact {
                // 任意声明配置变化至少需要重绘。
                paint_changed: config_changed,
                // 仅布局相关配置变化需要重新布局。
                layout_changed,
            },
            // 无类型化 patch 时替换同型组件并沿用相同分类。
            Err(widget) => {
                // 用新版组件替换无法类型化同步的旧实现。
                current.replace_widget(widget);
                // 返回替换后的精细失效分类。
                WidgetPatchImpact {
                    // 任意声明配置变化至少需要重绘。
                    paint_changed: config_changed,
                    // 未审计快照已经在上方保守归入布局变化。
                    layout_changed,
                }
            }
            // 类型分派未完成同步时不产生额外失效。
            Ok(false) => WidgetPatchImpact::default(),
        }
    }

    fn reconcile_children(
        tree: &mut WidgetTree,
        parent_id: WidgetId,
        children: Vec<ViewNode>,
        stagger_enter: Option<(f64, crate::ui::animation::AnimationConfig)>,
    ) -> bool {
        let old_child_count = tree
            .get(parent_id)
            .map(|node| node.children().len())
            .unwrap_or_default();
        // 唯一 keyed 同序声明直接复用现有身份；重复 key、摘要碰撞和替换继续走通用语义。
        let direct_keyed_reuse =
            old_child_count == children.len()
                && tree.get(parent_id).is_some_and(|parent| {
                    parent.children().iter().copied().zip(children.iter()).all(
                        |(child_id, child)| {
                            let Some(key) = child.key.as_deref() else {
                                return false;
                            };
                            tree.get(child_id).and_then(|node| node.key()) == Some(key)
                                && Self::can_reuse(tree, child_id, child)
                        },
                    )
                })
                && reconcile_keys_are_unique(&children);
        if direct_keyed_reuse {
            for (index, child) in children.into_iter().enumerate() {
                // 快路预检已经证明同序且可复用，协调期间父级直接子序列保持稳定。
                let child_id = tree
                    .get(parent_id)
                    .and_then(|parent| parent.children().get(index))
                    .copied()
                    .expect("direct keyed reconciliation child must remain present");
                tree.cancel_pending_removal(child_id);
                Self::reconcile_existing(tree, child_id, child);
            }
            return false;
        }

        // 摘要索引不取得旧 key 字符串所有权，避免每轮复制全部业务 key。
        let keyed_old_count = tree.get(parent_id).map_or(0, |parent| {
            parent
                .children()
                .iter()
                .filter(|&&child_id| tree.get(child_id).is_some_and(|node| node.key().is_some()))
                .count()
        });
        let mut old_by_key = ReconcileFingerprintMap::with_capacity_and_hasher(
            keyed_old_count,
            std::hash::BuildHasherDefault::default(),
        );
        if let Some(parent) = tree.get(parent_id) {
            for &child_id in parent.children() {
                if let Some(key) = tree.get(child_id).and_then(|node| node.key()) {
                    old_by_key.insert(reconcile_key_fingerprint(key), child_id);
                }
            }
        }
        let old_children = tree
            .get(parent_id)
            .map(|node| node.children().to_vec())
            .unwrap_or_default();
        let mut used_old = HashSet::new();
        let mut new_order = Vec::with_capacity(children.len());
        let mut structure_changed = old_children.len() != children.len();
        let stagger_anchor = std::time::Instant::now();
        let mut mounted_rank = 0;

        for (index, mut child) in children.into_iter().enumerate() {
            let candidate =
                child
                    .key
                    .as_ref()
                    .and_then(|key| {
                        let indexed = old_by_key.get(&reconcile_key_fingerprint(key)).copied()?;
                        if tree.get(indexed).and_then(|node| node.key()) == Some(key.as_str()) {
                            return Some(indexed);
                        }
                        // 摘要碰撞时逆序精确查找，保持旧 HashMap 最后写入者语义。
                        old_children.iter().rev().copied().find(|id| {
                            tree.get(*id).and_then(|node| node.key()) == Some(key.as_str())
                        })
                    })
                    .filter(|id| !used_old.contains(id))
                    .or_else(|| {
                        if child.key.is_some() {
                            return None;
                        }
                        old_children
                            .get(index)
                            .copied()
                            .filter(|id| !used_old.contains(id))
                            .filter(|id| tree.get(*id).is_some_and(|node| node.key().is_none()))
                    });

            let child_id = if let Some(child_id) = candidate {
                used_old.insert(child_id);
                if Self::can_reuse(tree, child_id, &child) {
                    tree.cancel_pending_removal(child_id);
                    Self::reconcile_existing(tree, child_id, child);
                    child_id
                } else {
                    tree.remove(child_id);
                    structure_changed = true;
                    Self::configure_staggered_child(
                        &mut child,
                        stagger_enter,
                        stagger_anchor,
                        mounted_rank,
                    );
                    mounted_rank += 1;
                    tree.build_child_node(parent_id, Self::expand(child))
                }
            } else {
                structure_changed = true;
                Self::configure_staggered_child(
                    &mut child,
                    stagger_enter,
                    stagger_anchor,
                    mounted_rank,
                );
                mounted_rank += 1;
                tree.build_child_node(parent_id, Self::expand(child))
            };
            new_order.push(child_id);
        }

        for (old_index, child_id) in old_children.iter().copied().enumerate() {
            if !used_old.contains(&child_id) && tree.get(child_id).is_some() {
                structure_changed = true;
                if tree.start_leave_transition(child_id) {
                    new_order.insert(old_index.min(new_order.len()), child_id);
                } else {
                    tree.remove(child_id);
                }
            }
        }

        let order_changed = tree
            .get(parent_id)
            .is_some_and(|parent| parent.children() != new_order.as_slice());
        if order_changed {
            if let Some(parent) = tree.get_mut(parent_id) {
                // 原子替换为协调后的稳定子节点顺序。
                *parent.children_mut() = new_order;
                // 让父组件同步依赖直接子节点集合的派生运行态。
                parent.notify_children_changed();
            }
            tree.tree_version += 1;
            structure_changed = true;
        }
        structure_changed
    }
}

// 仅在库测试中编译协调失效分类门禁。
