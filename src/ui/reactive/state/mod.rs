use std::any::TypeId;
// 使用线程局部可变容器保存嵌套捕获上下文。
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::core::{Rect, WidgetId};
/// A thread-safe destination for state-driven UI invalidation.
/// Implementations receive notifications after the state value lock is released.
pub trait InvalidationTarget: Send + Sync {
    fn invalidate_paint(&self, widget: WidgetId, rect: Option<Rect>);
    fn invalidate_layout(&self, widget: WidgetId);
}
/// Shared destination identity; subscriptions retain it until their leases expire.
pub type InvalidationHandle = Arc<dyn InvalidationTarget>;

type ReconcileCallback = Arc<dyn Fn() + Send + Sync>;
type StateWatcher<T> = Arc<dyn Fn(&T) + Send + Sync>;
type StateBindCapture = (
    WidgetId,
    InvalidationHandle,
    Option<Rect>,
    Vec<Arc<dyn StatePaintBind>>,
);

// 拆分结构性绑定租约，保持响应式状态主体低于规模上限。
#[path = "reconcile_lease.rs"]
// 编译结构性绑定租约的私有实现模块。
mod reconcile_lease;
// 向树与节点生命周期边界暴露租约类型。
pub(crate) use reconcile_lease::ReconcileBindLease;

// 拆分绘制绑定租约，保持响应式状态主体低于规模上限。
#[path = "paint_lease.rs"]
// 编译绘制绑定租约与捕获作用域的私有实现模块。
mod paint_lease;
// 向树与节点生命周期边界暴露绘制租约和捕获作用域。
pub(crate) use paint_lease::{PaintBindLease, StateBindCaptureGuard};

// 将副作用订阅与状态主体拆分，保持各文件规模受控。
#[path = "effect.rs"]
// 编译 State 私有的 Effect 自动订阅实现。
mod effect;
// 向响应式模块公开副作用句柄而不泄漏私有租约。
pub use effect::Effect;
// 向 State 与 Computed 的依赖追踪提供私有读取快照契约。
pub(crate) use effect::EffectDependency;

// 将派生值生命周期拆分到私有模块，保持状态主体低于规模上限。
#[path = "computed.rs"]
// 编译 Computed 的上游租约与下游失效传播实现。
mod computed;
// 保持既有响应式公开派生值入口不变。
pub use computed::Computed;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StateBindInvalidation {
    Paint,
    Layout,
}

// 订阅站点唯一身份：节点、窗口队列端点与失效种类。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PaintSiteKey {
    widget_id: WidgetId,
    // 以队列 Arc 地址区分窗口端点；站点自身持有队列强引用，地址在键存活期间稳定。
    queue: usize,
    invalidation: StateBindInvalidation,
}

impl PaintSiteKey {
    // 由节点身份与队列句柄构造站点键。
    pub(crate) fn new(
        widget_id: WidgetId,
        queue: &InvalidationHandle,
        invalidation: StateBindInvalidation,
    ) -> Self {
        Self {
            widget_id,
            queue: Arc::as_ptr(queue) as *const () as usize,
            invalidation,
        }
    }
}

// 站点表按键索引，替代线性扫描，保证共享 State 的大规模订阅仍是 O(log N)。
pub(crate) type PaintSiteMap = std::collections::BTreeMap<PaintSiteKey, PaintBindSite>;

#[derive(Clone)]
pub(crate) struct PaintBindSite {
    widget_id: WidgetId,
    queue: InvalidationHandle,
    rect: Option<Rect>,
    // 区分只重绘与需要重新测量的动态依赖站点。
    invalidation: StateBindInvalidation,
    // 记录由实际节点生命周期持有的租约数量。
    leases: usize,
    // 标记公开兼容入口是否要求站点持续存活到显式覆盖。
    persistent: bool,
}

#[derive(Clone)]
pub(crate) struct ReconcileBindSite {
    key: usize,
    callback: ReconcileCallback,
    // 记录当前树根和节点持有的窄绑定租约数量。
    leases: usize,
}

// ── 响应式依赖追踪 ────────────────────────────────────────────
//
// 设计：使用 thread_local 追踪当前正在计算的 Computed 所读取的 State。
// State::get() 在追踪启用时自动注册依赖，Computed 在计算完毕后收集
// 这些依赖的 generation 快照，后续 get() 时比对以判断是否需要重新计算。

// 保存单轮 Effect 或 Computed 捕获的唯一依赖，并以无分配槽位摘要加速重复判断。
#[derive(Default)]
struct DependencyCollector {
    // 保持依赖首次读取顺序，使后续失效检查顺序与既有契约一致。
    deps: Vec<EffectDependency>,
    // 记录槽身份低六位是否出现；碰撞时再精确扫描，绝不误删依赖。
    occupied_slots: u64,
}

impl DependencyCollector {
    // 仅在当前槽首次出现时构造拥有型检查器与订阅闭包。
    fn capture<F>(&mut self, slot_id: StateSlotId, register: F)
    where
        F: FnOnce() -> EffectDependency,
    {
        // 进程内槽身份单调分配，低六位为常见小依赖集提供无碰撞摘要。
        let slot_bit = 1_u64 << (slot_id.0 & 63);
        // 摘要命中时精确确认，碰撞槽仍保留完整依赖。
        if self.occupied_slots & slot_bit != 0
            && self
                .deps
                .iter()
                .any(|dependency| dependency.slot_id == slot_id)
        {
            // 后续差集原本也只保留首次观察，本处提前避免临时闭包与向量增长。
            return;
        }
        // 先登记摘要，再按首次读取顺序保存完整依赖。
        self.occupied_slots |= slot_bit;
        self.deps.push(register());
    }
}

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static TRACKING_DEPS: RefCell<Option<DependencyCollector>> =
        const { RefCell::new(None) };
}

// 以栈保存绘制依赖捕获，隔离嵌套组件和 panic 展开路径。
thread_local! {
    static STATE_BIND_CAPTURE_STACK: RefCell<Vec<StateBindCapture>> = const { RefCell::new(Vec::new()) };
}

// 保存一次 View 构建捕获的结构依赖与副作用，并由声明根显式交接给所属树。
#[derive(Default)]
pub(crate) struct StateCaptureOutput {
    // 保存本帧已登记的槽身份以保持同帧订阅去重。
    pub(crate) state_bind_slots: Vec<StateSlotId>,
    // 保存去重后的结构性 State 绑定源。
    pub(crate) state_binds: Vec<Arc<dyn StatePaintBind>>,
    // 保存本次构建创建的 Effect 实例。
    pub(crate) effects: Vec<Effect>,
}

// 使用线程私有的捕获帧栈隔离嵌套 View 构建与不同窗口的同步构建。
thread_local! {
    // 每个 begin 都压入独立帧，finish 或 panic 只弹出栈顶。
    static STATE_CAPTURE_STACK: RefCell<Vec<StateCaptureOutput>> = const { RefCell::new(Vec::new()) };
}
static NEXT_STATE_SLOT: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// 标识响应式状态存储槽的进程内稳定身份。
pub struct StateSlotId(pub(crate) u64);

impl StateSlotId {
    /// 返回槽身份的原始整数值。
    pub fn get(self) -> u64 {
        self.0
    }
}

/// 开始捕获 `State::get` 依赖 / `Effect::new` 实例（View 构建期间调用）。
pub(crate) fn begin_state_capture() {
    // 为当前构建创建独立输出帧，避免嵌套构建清空外层结果。
    STATE_CAPTURE_STACK.with(|stack| stack.borrow_mut().push(StateCaptureOutput::default()));
}

/// 结束 View 构建期的 State 捕获，并返回当前帧专属输出。
pub(crate) fn end_state_capture() -> StateCaptureOutput {
    // 只取走栈顶帧，外层捕获在嵌套完成后继续保持活动。
    STATE_CAPTURE_STACK.with(|stack| stack.borrow_mut().pop().unwrap_or_default())
}

// 判断当前线程是否仍有任意活动 View 捕获帧，供 Computed 强制重新暴露底层依赖。
fn state_capture_active() -> bool {
    // 仅检查栈是否非空，嵌套帧结束后外层帧仍应视为活动。
    STATE_CAPTURE_STACK.with(|stack| !stack.borrow().is_empty())
}

// 开始探测组件测量或绘制时读取的 State / Computed。
fn begin_state_bind_capture(widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>) {
    // 将本层捕获压栈，使嵌套绘制不会覆盖外层上下文。
    STATE_BIND_CAPTURE_STACK.with(|stack| {
        // 新捕获只接收本层后续读取的依赖。
        stack
            .borrow_mut()
            .push((widget_id, queue, rect, Vec::new()));
    });
}

// 结束最内层探测并返回由调用节点接管的绘制租约。
fn end_state_bind_capture(widget_id: WidgetId, layout: bool) -> Vec<PaintBindLease> {
    // 只弹出最内层上下文，恢复仍在执行的外层捕获。
    let capture = STATE_BIND_CAPTURE_STACK.with(|stack| stack.borrow_mut().pop());
    // 没有对应捕获时返回空集合，避免制造无所有者绑定。
    let Some((id, queue, rect, states)) = capture else {
        // 空捕获没有需要交接的资源。
        return Vec::new();
    };
    // 身份失配说明捕获作用域没有按后进先出结束。
    if id != widget_id {
        // 保留诊断但仍按实际捕获身份建立租约，避免跨节点投递。
        #[cfg(feature = "diagnostics")]
        tracing::warn!("State 绑定探测 widget_id 不一致: 期望 {widget_id}, 实际 {id}");
    }
    // 将每个读取源转换为由实际节点拥有的窄绘制租约。
    states
        // 逐一交接捕获源。
        .into_iter()
        // 按组件声明建立精确 Paint 或节点级 Layout 租约，不升级为整树 reconcile。
        .map(|source| {
            if layout {
                PaintBindLease::bind_layout(source, id, queue.clone())
            } else {
                PaintBindLease::bind(source, id, queue.clone(), rect)
            }
        })
        // 返回完整租约集合供节点整体替换。
        .collect()
}

// 丢弃最内层未完成捕获，供 panic 展开时恢复线程上下文。
fn discard_state_bind_capture(widget_id: WidgetId) {
    // 弹出当前作用域，不能把异常读取泄漏到下一次组件绘制。
    let capture = STATE_BIND_CAPTURE_STACK.with(|stack| stack.borrow_mut().pop());
    // 仅在存在失配上下文时留下可诊断证据。
    if let Some((id, _, _, _)) = capture
        && id != widget_id
    {
        // 捕获栈失配属于内部生命周期错误，但析构路径不得再次 panic。
        #[cfg(feature = "diagnostics")]
        tracing::warn!("State 绑定捕获清理 widget_id 不一致: 期望 {widget_id}, 实际 {id}");
    }
}

// 判断当前线程是否存在活动绘制依赖捕获。
fn state_bind_capture_active() -> bool {
    // 只检查捕获栈是否非空，不借出其中任何窗口资源。
    STATE_BIND_CAPTURE_STACK.with(|stack| !stack.borrow().is_empty())
}

fn try_capture_state_bind<T: Clone + Send + Sync + 'static>(state: &State<T>) {
    // 只把依赖登记给当前最内层组件捕获。
    STATE_BIND_CAPTURE_STACK.with(|stack| {
        // 独占访问栈顶依赖集合。
        let mut stack = stack.borrow_mut();
        // 没有活动捕获时保持普通读取无副作用。
        if let Some((_, _, _, captured)) = stack.last_mut() {
            // 保存共享状态源，实际绑定由节点租约接管。
            let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
            // 允许同一闭包重复读取，由站点租约计数保持精确释放。
            captured.push(bind);
        }
    });
}

pub(crate) fn capture_pending_state_bind<T: Clone + Send + Sync + 'static>(state: &State<T>) {
    // 只向当前最内层构建帧登记读取，避免子 View 输出泄漏到父 View。
    STATE_CAPTURE_STACK.with(|stack| {
        // 取得可变栈顶以维护本帧的去重集合。
        let mut stack = stack.borrow_mut();
        // 没有活跃 View 捕获时保持既有无副作用读取语义。
        let Some(output) = stack.last_mut() else {
            // 直接结束以避免为非 View 读取分配绑定。
            return;
        };
        // 读取稳定槽身份用于同帧去重。
        let slot_id = state.slot_id();
        // 已登记的同槽 State 不应重复生成 reconcile 订阅。
        if output.state_bind_slots.contains(&slot_id) {
            // 同槽已经属于本帧输出，避免重复安装相同 reconcile 订阅。
            return;
        }
        // 为当前 State 生成交给根树绑定的窄接口句柄。
        let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
        // 先登记槽身份，使后续同帧读取保持去重。
        output.state_bind_slots.push(slot_id);
        // 记录本帧读取的 State 绑定源。
        output.state_binds.push(bind);
    });
}

fn try_capture_computed_bind<T: Clone + Send + Sync + 'static>(computed: &Computed<T>) {
    // Computed 与 State 一样只登记给最内层节点捕获。
    STATE_BIND_CAPTURE_STACK.with(|stack| {
        // 独占访问栈顶依赖集合。
        let mut stack = stack.borrow_mut();
        // 非绘制读取不建立节点站点。
        if let Some((_, _, _, captured)) = stack.last_mut() {
            // 以共享句柄保存派生源，避免捕获阶段直接强持有窗口队列。
            let bind: Arc<dyn StatePaintBind> = Arc::new(computed.clone());
            // 把实际绑定推迟到捕获作用域正常完成时。
            captured.push(bind);
        }
    });
}

// 注册不会由节点租约自动释放的兼容绘制站点。
fn bind_persistent_paint_site(
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    widget_id: WidgetId,
    queue: InvalidationHandle,
    rect: Option<Rect>,
) {
    if let Ok(mut guard) = sites.lock() {
        // 以精确站点键定位，避免共享 State 大规模订阅时的线性查找。
        let key = PaintSiteKey::new(widget_id, &queue, StateBindInvalidation::Paint);
        if let Some(site) = guard.get_mut(&key) {
            // 更新同一端点的最新绘制范围。
            site.rect = rect;
            // 公开直接绑定要求站点保持到状态源销毁。
            site.persistent = true;
        } else {
            // 创建首个永久兼容站点。
            guard.insert(
                key,
                PaintBindSite {
                    widget_id,
                    queue,
                    rect,
                    invalidation: StateBindInvalidation::Paint,
                    // 永久入口本身不计入节点租约。
                    leases: 0,
                    // 标记该站点不能因租约归零而删除。
                    persistent: true,
                },
            );
        }
    }
}

// 增加一份由实际节点拥有的精确绘制站点租约。
fn retain_paint_site(
    // 接收状态源内部站点集合。
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    // 接收当前节点的代际身份。
    widget_id: WidgetId,
    // 接收所属窗口失效队列。
    queue: InvalidationHandle,
    // 接收当前布局解析出的绘制区域。
    rect: Option<Rect>,
) {
    retain_state_site(sites, widget_id, queue, rect, StateBindInvalidation::Paint);
}

// 增加一份由实际节点拥有的布局站点租约。
fn retain_layout_site(
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    widget_id: WidgetId,
    queue: InvalidationHandle,
) {
    retain_state_site(sites, widget_id, queue, None, StateBindInvalidation::Layout);
}

// 按失效种类增加一份由实际节点拥有的响应式站点租约。
fn retain_state_site(
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    widget_id: WidgetId,
    queue: InvalidationHandle,
    rect: Option<Rect>,
    invalidation: StateBindInvalidation,
) {
    // 只在站点集合可访问时登记。
    if let Ok(mut guard) = sites.lock() {
        // 以精确站点键定位同一节点与队列的既有站点。
        let key = PaintSiteKey::new(widget_id, &queue, invalidation);
        if let Some(site) = guard.get_mut(&key) {
            // 重绑时刷新最新布局范围。
            site.rect = rect;
            // 增加本次节点依赖持有。
            site.leases = site.leases.saturating_add(1);
        } else {
            // 首份节点租约创建可自动清理的站点。
            guard.insert(
                key,
                PaintBindSite {
                    // 保存代际化组件身份。
                    widget_id,
                    // 保存仍由节点租约负责释放的窗口队列。
                    queue,
                    // 保存当前精确绘制范围。
                    rect,
                    // 保存状态变化时需要投递的失效种类。
                    invalidation,
                    // 记录首份节点租约。
                    leases: 1,
                    // 节点捕获站点不具有永久所有权。
                    persistent: false,
                },
            );
        }
    }
}

// 释放一份实际节点持有的精确绘制站点租约。
fn release_paint_site(
    // 接收状态源内部站点集合。
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    // 接收正在离开的组件身份。
    widget_id: WidgetId,
    // 接收用于区分窗口端点的队列句柄。
    queue: &InvalidationHandle,
) {
    release_state_site(sites, widget_id, queue, StateBindInvalidation::Paint);
}

// 释放一份实际节点持有的布局站点租约。
fn release_layout_site(
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    widget_id: WidgetId,
    queue: &InvalidationHandle,
) {
    release_state_site(sites, widget_id, queue, StateBindInvalidation::Layout);
}

// 按失效种类释放一份实际节点持有的响应式站点租约。
fn release_state_site(
    sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    widget_id: WidgetId,
    queue: &InvalidationHandle,
    invalidation: StateBindInvalidation,
) {
    // 只在站点集合可访问时执行计数递减。
    if let Ok(mut guard) = sites.lock() {
        // 以精确站点键定位同一节点和窗口端点。
        let key = PaintSiteKey::new(widget_id, queue, invalidation);
        if let Some(site) = guard.get_mut(&key) {
            // 防御性饱和递减，析构路径不能因异常重复释放而下溢。
            site.leases = site.leases.saturating_sub(1);
            // 与既有 retain 语义等价：非永久站点在租约归零时精确移除，
            // 不再为每次释放重扫整张站点表。
            if !site.persistent && site.leases == 0 {
                guard.remove(&key);
            }
        }
    }
}

fn fire_paint_bindings(sites: &Arc<std::sync::Mutex<PaintSiteMap>>) {
    let sites = sites
        .lock()
        .ok()
        .map(|guard| guard.values().cloned().collect::<Vec<_>>());
    let Some(sites) = sites else {
        return;
    };
    for site in &sites {
        match site.invalidation {
            StateBindInvalidation::Paint => {
                site.queue.invalidate_paint(site.widget_id, site.rect);
            }
            StateBindInvalidation::Layout => {
                site.queue.invalidate_layout(site.widget_id);
                // 文本内容即使尺寸不变也必须重绘，Layout 不替代 Paint。
                site.queue.invalidate_paint(site.widget_id, None);
            }
        }
    }
}

fn bind_reconcile_site(
    sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    key: usize,
    callback: ReconcileCallback,
) {
    if let Ok(mut guard) = sites.lock() {
        if let Some(site) = guard.iter_mut().find(|site| site.key == key) {
            // 刷新同一树请求端口的可调用句柄。
            site.callback = callback;
            // 增加同源同树的生命周期持有计数。
            site.leases = site.leases.saturating_add(1);
        } else {
            // 建立由第一份租约持有的树请求端口。
            guard.push(ReconcileBindSite {
                // 保存树请求端口键。
                key,
                // 保存树请求端口回调。
                callback,
                // 记录第一份生命周期租约。
                leases: 1,
            });
        }
    }
}

// 撤销一份同源同树的结构性 State 绑定租约。
fn unbind_reconcile_site(
    // 接收 State 内部的树请求端口集合。
    sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    // 接收需要释放的 WidgetTree 请求端口键。
    key: usize,
) {
    // 仅在站点集合仍可访问时执行精确计数递减。
    if let Ok(mut guard) = sites.lock() {
        // 找到同一树的站点并减少一份租约。
        if let Some(site) = guard.iter_mut().find(|site| site.key == key) {
            // 防御性饱和减法避免异常重复析构下溢。
            site.leases = site.leases.saturating_sub(1);
        }
        // 移除已经不再由任何树或节点持有的站点。
        guard.retain(|site| site.leases != 0);
    }
}

fn fire_reconcile_bindings(sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>) {
    let callbacks: Vec<ReconcileCallback> = {
        let Ok(guard) = sites.lock() else {
            return;
        };
        // 常见的单树绑定只需克隆一个共享回调；先释放站点锁再调用，既保留重入语义，
        // 又避免每次 State 发布都为一个胖指针分配临时 Vec。
        match guard.as_slice() {
            [] => return,
            [site] => {
                let callback = site.callback.clone();
                drop(guard);
                callback();
                return;
            }
            _ => guard.iter().map(|site| site.callback.clone()).collect(),
        }
    };
    for callback in callbacks {
        callback();
    }
}

/// State 变更时推送精确 Paint 失效的绑定接口。
pub trait StatePaintBind: Send + Sync {
    /// 注册状态变化时调用的结构协调回调。
    fn bind_reconcile(&self, reconcile: ReconcileCallback);
    /// 使用树站点身份注册可精确释放的结构协调回调。
    fn bind_reconcile_site(&self, _key: usize, reconcile: ReconcileCallback) {
        self.bind_reconcile(reconcile);
    }
    // 撤销一份结构性树订阅；不支持订阅的源保持无操作。
    /// 释放指定树站点持有的一份结构协调订阅。
    fn unbind_reconcile_site(&self, _key: usize) {}
    /// 注册状态变化时向指定组件队列推送的精确绘制失效。
    fn bind_paint(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>);
    /// 增加一份由实际节点生命周期持有的绘制订阅。
    fn bind_paint_site(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>);
    /// 释放一份实际节点持有的绘制订阅。
    fn unbind_paint_site(&self, widget_id: WidgetId, queue: &InvalidationHandle);
    /// 增加一份由实际节点生命周期持有的布局订阅。
    fn bind_layout_site(&self, widget_id: WidgetId, queue: InvalidationHandle);
    /// 释放一份实际节点持有的布局订阅。
    fn unbind_layout_site(&self, widget_id: WidgetId, queue: &InvalidationHandle);
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for State<T> {
    fn bind_reconcile(&self, reconcile: ReconcileCallback) {
        self.bind_reconcile_invalidation(0, reconcile);
    }

    fn bind_reconcile_site(&self, key: usize, reconcile: ReconcileCallback) {
        self.bind_reconcile_invalidation(key, reconcile);
    }

    // 释放一份同树结构性订阅租约。
    fn unbind_reconcile_site(&self, key: usize) {
        self.unbind_reconcile_invalidation(key);
    }

    fn bind_paint(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>) {
        self.bind_paint_invalidation(widget_id, queue, rect);
    }

    // 增加 State 的节点绘制订阅计数。
    fn bind_paint_site(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>) {
        // 把租约登记到共享状态槽的绘制站点集合。
        retain_paint_site(&self.paint_sites, widget_id, queue, rect);
    }

    // 释放 State 的节点绘制订阅计数。
    fn unbind_paint_site(&self, widget_id: WidgetId, queue: &InvalidationHandle) {
        // 最后一份租约离开时移除站点和窗口队列强引用。
        release_paint_site(&self.paint_sites, widget_id, queue);
    }

    fn bind_layout_site(&self, widget_id: WidgetId, queue: InvalidationHandle) {
        retain_layout_site(&self.paint_sites, widget_id, queue);
    }

    fn unbind_layout_site(&self, widget_id: WidgetId, queue: &InvalidationHandle) {
        release_layout_site(&self.paint_sites, widget_id, queue);
    }
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for Computed<T> {
    fn bind_reconcile(&self, _reconcile: ReconcileCallback) {}

    fn bind_paint(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>) {
        self.bind_paint_invalidation(widget_id, queue, rect);
    }

    // 增加 Computed 的节点绘制订阅计数。
    fn bind_paint_site(&self, widget_id: WidgetId, queue: InvalidationHandle, rect: Option<Rect>) {
        // 委托 Computed 内部对象登记派生槽的绘制站点。
        self.bind_paint_site_invalidation(widget_id, queue, rect);
    }

    // 释放 Computed 的节点绘制订阅计数。
    fn unbind_paint_site(&self, widget_id: WidgetId, queue: &InvalidationHandle) {
        // 委托 Computed 内部对象释放派生槽的绘制站点。
        self.unbind_paint_site_invalidation(widget_id, queue);
    }

    fn bind_layout_site(&self, widget_id: WidgetId, queue: InvalidationHandle) {
        self.bind_layout_site_invalidation(widget_id, queue);
    }

    fn unbind_layout_site(&self, widget_id: WidgetId, queue: &InvalidationHandle) {
        self.unbind_layout_site_invalidation(widget_id, queue);
    }
}

// 保存一次依赖追踪调用替换掉的外层上下文，并在离开作用域时归还它。
struct DependencyTrackingGuard {
    // 保存进入本层前的外层依赖收集器。
    outer: Option<DependencyCollector>,
    // 标记外层上下文是否已经归还，避免析构时重复覆盖。
    restored: bool,
}

impl DependencyTrackingGuard {
    // 安装一个只属于当前闭包的空依赖收集器。
    fn enter() -> Self {
        // 从线程局部存储中原子地替换当前追踪上下文。
        let outer = TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 暂存可能存在的外层收集器。
            let outer = deps.take();
            // 安装本层独立的空收集器。
            *deps = Some(DependencyCollector::default());
            // 将外层收集器交给守卫保存。
            outer
        });
        // 返回负责恢复外层上下文的守卫。
        Self {
            // 记录进入时摘下的上下文。
            outer,
            // 守卫初始尚未执行恢复。
            restored: false,
        }
    }

    // 取出本层收集结果，并立即归还进入前的上下文。
    fn finish(mut self) -> Vec<EffectDependency> {
        // 从当前线程取出本层独立收集器。
        let collected = TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 取走本层的收集结果；异常重入时退化为空集合。
            deps.take()
                .map(|collector| collector.deps)
                .unwrap_or_default()
        });
        // 在返回结果前归还外层上下文。
        self.restore();
        // 将本层依赖交给调用方建立 generation 快照。
        collected
    }

    // 将进入本层前的上下文原样写回线程局部存储。
    fn restore(&mut self) {
        // 已恢复时不再覆盖可能已安装的新上下文。
        if self.restored {
            // 直接结束幂等恢复。
            return;
        }
        // 取出外层上下文，包含“外层不存在”的 None 情形。
        let outer = self.outer.take();
        // 用进入前的上下文替换当前本层上下文。
        TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 恢复外层追踪器或明确清空追踪状态。
            *deps = outer;
        });
        // 标记析构不应再次恢复。
        self.restored = true;
    }
}

impl Drop for DependencyTrackingGuard {
    // 覆盖闭包 unwind 路径，确保 panic 不会泄漏或丢失外层收集器。
    fn drop(&mut self) {
        // 无论正常路径还是 panic 路径，都幂等地恢复外层上下文。
        self.restore();
    }
}

/// 在当前线程启用依赖追踪，执行闭包后返回收集到的依赖 generation 检查器列表。
/// 支持嵌套：内层 collect_deps 保存并恢复外层追踪上下文，使 `Computed` 在其 get()
/// 内部也能被外层正确追踪。
pub(crate) fn collect_deps<F, R>(f: F) -> (R, Vec<EffectDependency>)
where
    F: FnOnce() -> R,
{
    // 在执行用户闭包前安装可在 unwind 时自动恢复的上下文守卫。
    let guard = DependencyTrackingGuard::enter();
    // 执行实际的 Computed 或 Effect 依赖读取。
    let result = f();
    // 仅在正常返回时交出本层完整的依赖集合。
    let collected = guard.finish();
    // 返回闭包结果及其对应的独立依赖集合。
    (result, collected)
}

/// 将当前 State 注册到追踪上下文中（如果追踪已启用）。
fn track_dep<F>(slot_id: StateSlotId, register: F)
where
    F: FnOnce() -> EffectDependency,
{
    TRACKING_DEPS.with(|deps| {
        let mut deps = deps.borrow_mut();
        if let Some(ref mut collector) = *deps {
            // 收集器在构造拥有型依赖前完成精确去重。
            collector.capture(slot_id, register);
        }
    });
}

/// A reactive state value that notifies watchers on change.
/// Thread-safe: Send + Sync when T is Send + Sync.
///
/// 支持自动 reconcile invalidation：当通过 `set()` / `update()` 修改值时，自动调用注册的 reconcile 回调，
/// 通知 WidgetTree 重新渲染所属 View。reconcile 回调由 ViewAdapter 在 ViewNode 展开时自动绑定，
/// 用户不需要手动请求 reconcile。
pub struct State<T> {
    // 单一源实例同时拥有值槽与独立订阅表，公开句柄只克隆此所有权。
    source: Arc<StateDependencySource<T>>,
    /// reconcile invalidation 回调——值变更时自动调用，通知 WidgetTree 重绘所属节点。
    pub(crate) reconcile_sites: Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    /// Phase 6：精确 Paint 失效绑定（WidgetId + 队列句柄）。
    pub(crate) paint_sites: Arc<std::sync::Mutex<PaintSiteMap>>,
}

struct StateInner<T> {
    slot_id: StateSlotId,
    value: T,
    generation: u64,
    // 以惰性写时复制集合保存有序观察器，空状态不承担额外分配。
    watchers: Option<Arc<Vec<StateWatcher<T>>>>,
}

// 保存 State 的值槽与下游订阅端口，不把生命周期对象泄漏到 UI 协调层。
struct StateDependencySource<T> {
    // 值与 generation 始终在同一读写锁内观察。
    inner: RwLock<StateInner<T>>,
    // 订阅表保持独立锁，通知、注销与用户重入不得持有值锁。
    subscribers: effect::DependencySubscriberRegistry,
}

impl<T: Clone + Send + Sync + 'static> effect::DependencySource for StateDependencySource<T> {
    // 通过值锁读取与业务值同域的当前 generation。
    fn generation(&self) -> u64 {
        self.inner
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .generation
    }

    // 只暴露私有下游注册表的窄借用。
    fn subscribers(&self) -> &effect::DependencySubscriberRegistry {
        &self.subscribers
    }
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    /// 创建代数为零、拥有独立稳定槽身份的响应式状态。
    pub fn new(value: T) -> Self {
        let reconcile_sites = Arc::new(std::sync::Mutex::new(Vec::new()));
        let paint_sites = Arc::new(std::sync::Mutex::new(PaintSiteMap::new()));
        Self {
            source: Arc::new(StateDependencySource {
                inner: RwLock::new(StateInner {
                    slot_id: StateSlotId(NEXT_STATE_SLOT.fetch_add(1, Ordering::Relaxed)),
                    value,
                    generation: 0,
                    watchers: None,
                }),
                subscribers: RwLock::new(effect::DependencySubscribers::new()),
            }),
            reconcile_sites,
            paint_sites,
        }
    }

    /// 绑定精确 Paint 失效：State 变更时向队列推送 `Invalidation::Paint`。
    pub fn bind_paint_invalidation(
        &self,
        widget_id: WidgetId,
        queue: InvalidationHandle,
        rect: Option<Rect>,
    ) {
        bind_persistent_paint_site(&self.paint_sites, widget_id, queue, rect);
    }

    /// 为指定树站点登记结构协调失效回调。
    pub fn bind_reconcile_invalidation(&self, key: usize, reconcile: ReconcileCallback) {
        bind_reconcile_site(&self.reconcile_sites, key, reconcile);
    }

    // 释放一份由树或节点生命周期持有的结构性订阅。
    /// 释放指定树站点持有的一份结构协调订阅。
    pub fn unbind_reconcile_invalidation(&self, key: usize) {
        // 从同树站点扣除当前租约。
        unbind_reconcile_site(&self.reconcile_sites, key);
    }

    /// 设置 reconcile invalidation 回调。此回调在值变更时（`set` / `update`）自动调用。
    /// 由 ViewAdapter 内部使用，用户不需要调用此方法。
    pub fn set_reconcile_invalidation_fn<F: Fn() + Send + Sync + 'static>(&self, f: F) {
        if let Ok(mut guard) = self.reconcile_sites.lock() {
            guard.clear();
            guard.push(ReconcileBindSite {
                key: 0,
                callback: Arc::new(f),
                // 兼容入口直接建立一份永久到下次覆盖的持有。
                leases: 1,
            });
        }
    }

    /// 克隆当前值，并把本次读取登记到活动依赖捕获上下文。
    pub fn get(&self) -> T {
        // 在同一读锁快照中取得值、generation 与稳定槽身份。
        let (value, observed_generation, slot_id) = {
            // 获取状态快照锁以避免值与 generation 分离观察。
            let inner = self
                .source
                .inner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            // 复制可安全离开锁区的值与元数据。
            (inner.value.clone(), inner.generation, inner.slot_id)
        };
        // 将该快照注册到活跃的 Computed 或 Effect 依赖收集器。
        track_dep(slot_id, || {
            // 仅首次读取克隆同一个窄依赖源，不再装箱 generation 与订阅闭包。
            let source: Arc<dyn effect::DependencySource> = self.source.clone();
            EffectDependency {
                slot_id,
                observed_generation,
                source,
            }
        });
        // 继续记录结构性 State 绑定捕获。
        try_capture_state_bind(self);
        // 继续记录当前 View 构建帧的待交接绑定。
        capture_pending_state_bind(self);
        // 返回与登记 generation 同一读锁快照取得的值。
        value
    }

    pub fn get_untracked(&self) -> T {
        self.source
            .inner
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .value
            .clone()
    }

    /// 替换当前值，推进代数并同步通知 Effect、观察器和失效站点。
    pub fn set(&self, value: T) {
        // 只在存在公开观察器时建立值与处理器快照；组件失效端口不消费值。
        let watch_notification: Option<(T, Arc<Vec<StateWatcher<T>>>)>;
        // 保存准备在 State 锁外通知的存活 Effect。
        let effect_subscribers;
        {
            let mut inner = self.source.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.value = value;
            inner.generation += 1;
            // 观察器必须在锁外接收稳定快照；空观察器热段无需深克隆业务状态。
            watch_notification = inner
                .watchers
                .as_ref()
                .map(|watchers| (inner.value.clone(), Arc::clone(watchers)));
        }
        // 在值锁释放后从独立注册表收集需要通知的 Effect。
        effect_subscribers = effect::collect_subscribers(&self.source.subscribers);
        // 先在 State 写锁外通知内部 Effect，公开 watcher panic 也不能吞掉该信号。
        effect::notify_subscribers(effect_subscribers);
        if let Some((snapshot, watchers)) = watch_notification {
            for watcher in watchers.iter() {
                watcher(&snapshot);
            }
        }
        Self::fire_invalidation(&self.reconcile_sites, &self.paint_sites);
    }

    /// 原地修改当前值，推进代数并同步通知 Effect、观察器和失效站点。
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        // 只在存在公开观察器时建立值与处理器快照；组件失效端口不消费值。
        let watch_notification: Option<(T, Arc<Vec<StateWatcher<T>>>)>;
        // 保存准备在 State 锁外通知的存活 Effect。
        let effect_subscribers;
        {
            let mut inner = self.source.inner.write().unwrap_or_else(|e| e.into_inner());
            f(&mut inner.value);
            inner.generation += 1;
            // 观察器必须在锁外接收稳定快照；空观察器热段无需深克隆业务状态。
            watch_notification = inner
                .watchers
                .as_ref()
                .map(|watchers| (inner.value.clone(), Arc::clone(watchers)));
        }
        // 在值锁释放后从独立注册表收集需要通知的 Effect。
        effect_subscribers = effect::collect_subscribers(&self.source.subscribers);
        // 先在 State 写锁外通知内部 Effect，公开 watcher panic 也不能吞掉该信号。
        effect::notify_subscribers(effect_subscribers);
        if let Some((snapshot, watchers)) = watch_notification {
            for watcher in watchers.iter() {
                watcher(&snapshot);
            }
        }
        Self::fire_invalidation(&self.reconcile_sites, &self.paint_sites);
    }

    fn fire_invalidation(
        reconcile_sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
        paint_sites: &Arc<std::sync::Mutex<PaintSiteMap>>,
    ) {
        fire_paint_bindings(paint_sites);
        fire_reconcile_bindings(reconcile_sites);
    }

    /// 注册每次值变化后在状态写锁外同步调用的观察器。
    pub fn watch<F: Fn(&T) + Send + Sync + 'static>(&self, f: F) {
        let mut inner = self.source.inner.write().unwrap_or_else(|e| e.into_inner());
        // 注册属于低频生命周期路径；活动通知快照存在时复制旧列表，保持本轮顺序稳定。
        let watchers = inner.watchers.get_or_insert_with(|| Arc::new(Vec::new()));
        Arc::make_mut(watchers).push(Arc::new(f));
    }

    /// 返回每次 [`Self::set`] 或 [`Self::update`] 后递增的状态代数。
    pub fn generation(&self) -> u64 {
        self.source
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .generation
    }

    /// 返回此状态共享存储槽的稳定身份。
    pub fn slot_id(&self) -> StateSlotId {
        self.source
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .slot_id
    }

    #[allow(dead_code)]
    pub(crate) fn capture_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        TypeId::of::<T>().hash(&mut hasher);
        self.slot_id().hash(&mut hasher);
        hasher.finish()
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            reconcile_sites: self.reconcile_sites.clone(),
            paint_sites: self.paint_sites.clone(),
        }
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("slot_id", &self.slot_id())
            .field("value", &self.get())
            .field("generation", &self.generation())
            .finish()
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/ui/reactive/state/mod_tests.rs"]
mod mod_tests;
