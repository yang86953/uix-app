use super::*;
use crate::core::{Constraints, Rect};
use crate::draw::renderer::{Invalidation, InvalidationQueueHandle};
use crate::platform::windowing::{KeyCode, KeyMod};
use crate::ui::animation::AnimatedSource;
use crate::ui::event::{HandlerTable, SemanticEvent, WindowAction};
use crate::ui::overlay::{OverlayRebuildScratch, OverlayStack};
use crate::ui::render_handler::RenderHandlerTable;
use crate::ui::theme::Theme;
use crate::ui::theme::traits::ThemeTokens;
use crate::ui::widget_runtime::app_state::{AppState, FocusRequest};
use crate::ui::widget_runtime::focus_handle::FocusHandle;
use crate::ui::widget_runtime::managers::WidgetManagers;
// 保存每个窗口树独占的内联组件私有状态。
use crate::ui::widget_state::{
    UixWidgetScope,
    // 保存待最外层事务接纳的私有状态写入回执。
    WidgetStateCaptureReceipt,
    // 保存窗口私有状态存储和作用域身份。
    WidgetStateStore,
};
// 保存按工作身份去重的来源所有者集合。
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static NEXT_WIDGET_TREE_SCOPE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct BoundAnimatedSource {
    tree_scope: u64,
    source: Arc<dyn AnimatedSource>,
}

// 区分声明根与运行时节点对动画源的独立生命周期声明。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AnimatedSourceOwner {
    // 根协调拥有的声明根动画源。
    Root,
    // 已分配真实 WidgetId 的节点动画源。
    Node(WidgetId),
}

// 保存事务成功后才可提交的一次动画源所有权替换。
struct PendingAnimatedSourceOwnerUpdate {
    // 标识本次替换影响的根或实际节点所有者。
    owner: AnimatedSourceOwner,
    // 保存尚未绑定到树作用域的候选动画源集合。
    sources: Vec<Arc<dyn AnimatedSource>>,
    // 标记已因真实结构销毁而失效、提交时必须跳过的请求。
    cancelled: bool,
}

impl Drop for BoundAnimatedSource {
    fn drop(&mut self) {
        self.source.unbind_owner(self.tree_scope);
    }
}

#[cfg(test)]
thread_local! {
    /// 1=Phase1 2=Phase2 4=Phase4；0=不记录。
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer is already const and the lint fires through thread_local"
    )]
    pub(crate) static LAYOUT_TRACE_PHASE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

// 节点构建：换根、加子节点与构建入口（文件名避开 Cargo 构建脚本约定名 build.rs）。
mod tree_ops;
// 保存 WidgetTree 私有的运行与 fail-stop 执行状态。
mod execution_state;
// 保存事务异常边界、fail-stop 准入与关闭资源释放实现。
mod execution;
#[path = "../tree_layout/mod.rs"]
mod tree_layout;
pub(crate) use tree_layout::LayoutArrangeScratch;

/// 拥有组件节点、布局、交互、渲染处理器及事务状态的运行时树。
pub struct WidgetTree {
    // 由 WidgetTree 唯一拥有的事务发布与 fail-stop 状态机。
    execution_state: execution_state::WidgetTreeExecutionState,
    tree_scope: u64,
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_slots: Vec<usize>,
    pub(crate) generations: Vec<u32>,
    pub(crate) next_slot: usize,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) scroll_region_moves: Vec<(Rect, f32, f32)>,
    pub(crate) pending_window_actions: Vec<WindowAction>,
    /// 每次公开树结构或可见性变化后递增的版本号。
    pub tree_version: u64,
    /// UI 域主题令牌根；ScenePaint::paint 用它构造 UI-owned 绘制上下文。
    theme_tokens: std::sync::Arc<dyn ThemeTokens>,
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,
    // 先序遍历缓存失效时复用的深度优先栈。
    traversal_stack_scratch: std::cell::RefCell<Vec<WidgetId>>,
    /// 二维与三维命中递归复用的 `(子节点, 原始顺序)` 排序工作区。
    pub(crate) hit_test_order_scratch: std::cell::RefCell<Vec<(WidgetId, usize)>>,
    /// 坐标投影、裁剪与命中逆变换复用的根到节点视觉路径工作区。
    pub(crate) visual_path_scratch: std::cell::RefCell<Vec<WidgetId>>,
    // 复用 Wheel 捕获阶段的根到目标祖先快照；分发期间取出以隔离回调重入。
    pub(crate) wheel_capture_path_scratch: Vec<WidgetId>,
    /// 焦点切换复用的旧、新包含路径快照；由本树唯一拥有，不跨窗口共享。
    pub(crate) focus_transition_path_scratch: Vec<WidgetId>,

    pub(crate) handler_table: HandlerTable,
    pub(crate) render_handler_table: RenderHandlerTable,
    pub(crate) overlay_stack: OverlayStack,
    // 组件浮层重建跨帧复用的树级临时集合。
    pub(crate) overlay_rebuild_scratch: OverlayRebuildScratch,

    pub(crate) invalidation: InvalidationQueueHandle,
    pub(crate) pending_invalidations: Vec<Invalidation>,
    pub(crate) invalidation_batch_depth: usize,
    pub(crate) layout_ancestor_scratch: Vec<WidgetId>,
    pub(crate) reconcile_requested: Arc<AtomicBool>,
    pub(crate) reconcile_callback: Arc<dyn Fn() + Send + Sync>,
    // 保存声明根持有的结构性 State 订阅租约。
    pub(crate) root_reconcile_state_binds: Vec<crate::ui::reactive::state::ReconcileBindLease>,
    pub(crate) effects: Vec<crate::ui::reactive::state::Effect>,
    pub(crate) animated_sources: BTreeMap<WidgetId, BoundAnimatedSource>,
    // 记录每个所有者声明的工作身份，以在最后一个所有者离开时才解绑。
    animated_source_owners: BTreeMap<AnimatedSourceOwner, BTreeSet<WidgetId>>,
    // 保存当前嵌套构建事务尚未确认的动画源所有权替换。
    pending_animated_source_owner_updates: Vec<PendingAnimatedSourceOwnerUpdate>,
    pub(crate) active_widget_animations: HashSet<WidgetId>,
    pub(crate) animation_ids_scratch: Vec<WidgetId>,
    pub(crate) lifecycle_states_scratch: Vec<(WidgetId, bool)>,
    pub(crate) layout_scratch: tree_layout::LayoutFrameScratch,
    app_state_semantic_events_scratch: Vec<(WidgetId, SemanticEvent)>,
    app_state_focus_requests_scratch: Vec<(WidgetId, FocusRequest)>,
    pub(crate) managers: WidgetManagers,
    pub(crate) app_state: Option<AppState>,
    focus_handles: HashMap<WidgetId, FocusHandle>,
    timer_routes: BTreeMap<u64, (WidgetId, u32)>,
    focus_trap_restore: Vec<(WidgetId, Option<WidgetId>)>,
    pub(crate) window_focused: bool,
    keyboard_focus_visible: bool,
    pub(crate) keyboard_activation: Option<(WidgetId, KeyCode, KeyMod)>,
    // 由当前树拥有，禁止跨窗口共享内联组件私有状态。
    pub(crate) widget_state_store: WidgetStateStore,
    // 在适配器构建事务内延迟清理，避免同轮替换误删复用状态。
    pub(crate) widget_state_transaction_depth: usize,
    // 保存当前嵌套事务尚未在最外层成功后接纳的状态 journal。
    pub(crate) widget_state_pending_receipts: Vec<WidgetStateCaptureReceipt>,
    #[cfg(feature = "test-harness")]
    pub(crate) automation_recorder: Option<crate::ui::automation::AutomationRecorder>,
    /// layout() 内实际改写 frame 次数（回归：收敛后二次 layout 应为 0）。
    #[cfg(test)]
    pub(crate) layout_frame_writes: std::cell::Cell<u32>,
    /// Phase 4 实际执行的 shrink 次数（回归：Stretch 侧栏不应反复 shrink）。
    #[cfg(test)]
    pub(crate) layout_shrink_ops: std::cell::Cell<u32>,
    /// 单次 layout() 收敛循环实际执行的遍数（含最后稳定遍）。
    #[cfg(test)]
    pub(crate) layout_converge_passes: std::cell::Cell<u32>,
    /// Phase 2 实际扩展 frame 的次数（回归：不得在稳定后反复 120→124）。
    #[cfg(test)]
    pub(crate) layout_expand_ops: std::cell::Cell<u32>,
    /// 测试探针：记录 `(phase, id, before_h, after_h)` 的 frame 写入。
    #[cfg(test)]
    pub(crate) layout_frame_trace: std::cell::RefCell<Vec<(u8, WidgetId, i32, i32)>>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        let reconcile_requested = Arc::new(AtomicBool::new(false));
        let reconcile_callback = {
            let requested = Arc::clone(&reconcile_requested);
            Arc::new(move || requested.store(true, Ordering::Release))
                as Arc<dyn Fn() + Send + Sync>
        };
        Self {
            // 新树从可接收协调与外部工作的私有状态开始。
            execution_state: execution_state::WidgetTreeExecutionState::operational(),
            tree_scope: NEXT_WIDGET_TREE_SCOPE.fetch_add(1, Ordering::Relaxed),
            nodes: Vec::new(),
            free_slots: Vec::new(),
            generations: Vec::new(),
            next_slot: 0,
            root_id: None,
            theme_tokens: Theme::antd_light().tokens_arc(),
            scroll_region_moves: Vec::new(),
            pending_window_actions: Vec::new(),
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
            traversal_stack_scratch: std::cell::RefCell::new(Vec::new()),
            hit_test_order_scratch: std::cell::RefCell::new(Vec::new()),
            visual_path_scratch: std::cell::RefCell::new(Vec::new()),
            wheel_capture_path_scratch: Vec::new(),
            focus_transition_path_scratch: Vec::new(),
            handler_table: HandlerTable::new(),
            render_handler_table: RenderHandlerTable::default(),
            overlay_stack: OverlayStack::new(),
            overlay_rebuild_scratch: OverlayRebuildScratch::default(),
            invalidation: crate::draw::renderer::InvalidationQueue::shared(),
            pending_invalidations: Vec::new(),
            invalidation_batch_depth: 0,
            layout_ancestor_scratch: Vec::new(),
            reconcile_requested,
            reconcile_callback,
            // 初始树尚未接纳任何声明根结构性 State 绑定。
            root_reconcile_state_binds: Vec::new(),
            effects: Vec::new(),
            animated_sources: BTreeMap::new(),
            // 初始树没有任何根或节点动画源所有者。
            animated_source_owners: BTreeMap::new(),
            // 初始树没有等待事务提交的动画源所有权替换。
            pending_animated_source_owner_updates: Vec::new(),
            active_widget_animations: HashSet::new(),
            animation_ids_scratch: Vec::new(),
            lifecycle_states_scratch: Vec::new(),
            layout_scratch: tree_layout::LayoutFrameScratch::default(),
            app_state_semantic_events_scratch: Vec::new(),
            app_state_focus_requests_scratch: Vec::new(),
            managers: WidgetManagers::new(),
            app_state: None,
            focus_handles: HashMap::new(),
            timer_routes: BTreeMap::new(),
            focus_trap_restore: Vec::new(),
            window_focused: true,
            keyboard_focus_visible: true,
            keyboard_activation: None,
            widget_state_store: WidgetStateStore::new(),
            widget_state_transaction_depth: 0,
            // 初始树没有等待事务确认的组件状态 journal。
            widget_state_pending_receipts: Vec::new(),
            #[cfg(feature = "test-harness")]
            automation_recorder: None,
            #[cfg(test)]
            layout_frame_writes: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_shrink_ops: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_converge_passes: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_expand_ops: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_frame_trace: std::cell::RefCell::new(Vec::new()),
        }
    }
}

mod focus;
mod methods;
