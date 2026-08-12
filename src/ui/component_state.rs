// 声明组件私有状态所需的运行时类型依赖。
use crate::ui::reactive::state::{State, StateSlotId};
// 保存一次捕获期间的线程局部上下文。
use std::cell::RefCell;
// 保存状态槽的动态类型值。
use std::any::Any;
// 维护作用域到字段状态的映射。
use std::collections::HashMap;
// 让窗口树和捕获上下文安全共享状态存储。
use std::sync::{Arc, Mutex};

// 标识一次内联组件调用的稳定声明身份。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UixComponentScope {
    // 保存宏调用点文本以隔离相同声明的不同调用位置。
    callsite: &'static str,
    // 保存代码生成器分配的组件声明标识。
    declaration: u64,
    // 保存当前捕获内相同调用点的出现序号。
    occurrence: u64,
}

// 标识实际 View 根节点承载的一个组件作用域。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct UixComponentScopeMarker {
    // 保存私有状态所属的组件作用域。
    scope: UixComponentScope,
    // 保存该组件在其展开根列表中的序号。
    root_ordinal: u64,
}

// 为树节点提供作用域标记的内部读取入口。
impl UixComponentScopeMarker {
    // 构造一个不改变作用域身份的根节点标记。
    pub(crate) fn new(scope: UixComponentScope, root_ordinal: u64) -> Self {
        // 返回完整标记以同时参与节点身份和生命周期清理。
        Self { scope, root_ordinal }
    }

    // 返回状态存储清理所需的作用域身份。
    pub(crate) fn scope(&self) -> &UixComponentScope {
        // 借用标记内的稳定作用域。
        &self.scope
    }
}

// 保存组件状态存储中的动态值及其可诊断类型名。
struct ComponentStateValue {
    // 保存字段首次初始化时的具体 State 类型。
    type_name: &'static str,
    // 保存精确回滚所需的状态槽身份。
    slot_id: StateSlotId,
    // 保存可跨重建复用的响应式状态句柄。
    value: Box<dyn Any + Send + Sync>,
}

// 记录一次捕获真正插入的状态槽。
struct ComponentStateInsert {
    // 保存插入字段所属的组件作用域。
    scope: UixComponentScope,
    // 保存作用域内的稳定字段标识。
    field: u64,
    // 保存实际插入句柄的唯一槽身份。
    slot_id: StateSlotId,
}

// 保存一个窗口树拥有的全部组件私有状态。
#[derive(Default)]
struct ComponentStateStoreInner {
    // 按组件作用域再按字段标识隔离状态。
    fields: HashMap<UixComponentScope, HashMap<u64, ComponentStateValue>>,
}

// 让单个 WidgetTree 拥有并在其所有捕获之间共享私有状态。
#[derive(Clone, Default)]
pub(crate) struct ComponentStateStore {
    // 通过互斥锁支持同一窗口捕获与回调之间的安全访问。
    inner: Arc<Mutex<ComponentStateStoreInner>>,
}

// 保存当前 View 捕获的临时状态上下文，绝不作为长期状态所有者。
struct ComponentStateCapture {
    // 指向当前 WidgetTree 专属状态存储。
    store: ComponentStateStore,
    // 为相同静态调用点分配本次捕获内的出现序号。
    occurrences: HashMap<(&'static str, u64), u64>,
    // 记录本层捕获新插入的字段以供异常回滚。
    insertions: Vec<ComponentStateInsert>,
}

// 代表一次成功捕获尚未被挂载事务接受的状态写入。
pub(crate) struct ComponentStateCaptureReceipt {
    // 保存本次捕获写入的树私有存储。
    store: ComponentStateStore,
    // 保存未提交的精确插入 journal。
    insertions: Vec<ComponentStateInsert>,
    // 标记挂载事务已经接受这些状态槽。
    committed: bool,
}

// 管理组件状态捕获与挂载事务之间的两阶段交接。
impl ComponentStateCaptureReceipt {
    // 从已结束的线程局部捕获创建未提交回执。
    fn new(finished: ComponentStateCapture) -> Self {
        // 返回由调用方明确决定提交的一次性所有权。
        Self {
            // 转移本次捕获使用的存储句柄。
            store: finished.store,
            // 转移本层独立记录的插入集。
            insertions: finished.insertions,
            // 初始状态必须在丢弃时回滚。
            committed: false,
        }
    }

    // 由成功挂载或协调事务接受本次状态写入。
    fn commit(mut self) {
        // 标记 Drop 不再撤销已挂载的状态槽。
        self.committed = true;
    }
}

// 仅允许树事务协调器在同一批次内原子接纳全部成功捕获的 journal。
pub(crate) fn accept_component_state_receipts(
    // 接收已经通过最外层 WidgetTree 事务确认的回执集合。
    receipts: Vec<ComponentStateCaptureReceipt>,
) {
    // 逐个消费回执以关闭其 Drop 回滚路径。
    for receipt in receipts {
        // 标记该 journal 已作为同批树事务的一部分被接纳。
        receipt.commit();
    }
}

// 确保未成功交付给 WidgetTree 的捕获不泄漏私有状态。
impl Drop for ComponentStateCaptureReceipt {
    // 在回执离开作用域时执行提交或回滚的终态处理。
    fn drop(&mut self) {
        // 已提交回执不能再修改树状态。
        if self.committed {
            // 直接返回保留挂载成功的槽。
            return;
        }
        // 取出 journal 使重入 Drop 也不会重复回滚。
        let insertions = std::mem::take(&mut self.insertions);
        // 精确撤销仍指向本次新建槽的字段。
        self.store.rollback_insertions(insertions);
    }
}

// 仅在构建 View 时暴露当前树的状态存储上下文。
thread_local! {
    // 嵌套捕获通过替换和恢复避免跨窗口泄漏。
    static COMPONENT_STATE_CAPTURE: RefCell<Option<ComponentStateCapture>> = const { RefCell::new(None) };
}

// 为给定窗口树创建独立的组件私有状态存储。
impl ComponentStateStore {
    // 创建空存储以供一个 WidgetTree 独占。
    pub(crate) fn new() -> Self {
        // 返回默认的并发安全状态容器。
        Self::default()
    }

    // 复用现有字段状态或首次建立指定字段的 State。
    fn state<T>(&self, scope: &UixComponentScope, field: u64, init: impl FnOnce() -> T) -> State<T>
    where
        // 约束 State 的公开线程安全契约。
        T: Clone + Send + Sync + 'static,
    {
        // 在执行初始化闭包前只读检查，避免闭包重入时持锁死锁。
        if let Some(existing) = self.existing_state::<T>(scope, field) {
            // 已有同类型字段直接复用原句柄。
            return existing;
        }
        // 仅在字段第一次挂载时执行声明的初始值构造。
        let state = State::new(init());
        // 锁定状态映射并处理初始化期间可能出现的重入竞争。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 定位或创建此组件调用实例的字段映射。
        let fields = inner.fields.entry(scope.clone()).or_default();
        // 若另一个重入路径已先建立字段则复用它。
        if let Some(value) = fields.get(&field) {
            // 同类型字段保持第一次建立的状态为准。
            if let Some(existing) = value.value.downcast_ref::<State<T>>() {
                // 丢弃本次未挂载候选并返回既有句柄。
                return existing.clone();
            }
            // 明确报告宏字段标识被不兼容类型复用的契约错误。
            Self::type_mismatch(scope, field, value.type_name, std::any::type_name::<T>());
        }
        // 记录类型名以便未来错误精确诊断。
        let type_name = std::any::type_name::<T>();
        // 记录候选句柄的唯一槽身份。
        let slot_id = state.slot_id();
        // 把新状态归属到本窗口的组件作用域。
        fields.insert(
            field,
            ComponentStateValue {
                type_name,
                // 把精确槽身份与动态值一同保存。
                slot_id,
                value: Box::new(state.clone()),
            },
        );
        // 在登记捕获 journal 前释放存储锁以允许重入。
        drop(inner);
        // 仅记录当前捕获在同一存储中实际插入的槽。
        record_component_state_insert(self, scope, field, slot_id);
        // 返回首次建立的状态句柄。
        state
    }

    // 查询已存在字段，并在类型不匹配时给出稳定诊断。
    fn existing_state<T>(&self, scope: &UixComponentScope, field: u64) -> Option<State<T>>
    where
        // 保持 State 的公开线程安全契约。
        T: Clone + Send + Sync + 'static,
    {
        // 锁定后只读取已建立的字段。
        let inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 未建立作用域或字段时允许首次初始化。
        let value = inner.fields.get(scope)?.get(&field)?;
        // 同类型字段返回原有状态句柄。
        if let Some(state) = value.value.downcast_ref::<State<T>>() {
            // 返回克隆句柄而不是分配新的 State 槽。
            return Some(state.clone());
        }
        // 类型变化是宏稳定字段标识违反契约，必须尽早失败。
        Self::type_mismatch(scope, field, value.type_name, std::any::type_name::<T>());
    }

    // 统一生成字段类型不兼容的可诊断 panic。
    fn type_mismatch(scope: &UixComponentScope, field: u64, existing: &'static str, requested: &'static str) -> ! {
        // 明确列出所有稳定身份部分以便定位代码生成错误。
        panic!(
            "uix 组件私有 state 类型不匹配：callsite={} declaration={} occurrence={} field={}，已有 {}，请求 {}",
            scope.callsite,
            scope.declaration,
            scope.occurrence,
            field,
            existing,
            requested,
        );
    }

    // 删除树上已无任何实际节点承载的组件作用域。
    pub(crate) fn retain_scopes(&self, live_scopes: &std::collections::HashSet<UixComponentScope>) {
        // 锁定后一次性清除未挂载作用域，避免遗留条件分支状态。
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .fields
            .retain(|scope, _| live_scopes.contains(scope));
    }

    // 在窗口树关闭后释放其全部组件私有状态。
    pub(crate) fn clear(&self) {
        // 清空仅属于当前窗口树的状态映射。
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .fields
            .clear();
    }

    // 回滚异常捕获实际新增且未被替换的状态槽。
    fn rollback_insertions(&self, insertions: Vec<ComponentStateInsert>) {
        // 一次锁定完成本层 journal 的精确回滚。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 按插入的逆序撤销以保持重入初始化的栈语义。
        for insertion in insertions.into_iter().rev() {
            // 仅在字段仍指向原插入槽时删除。
            let remove_scope = if let Some(fields) = inner.fields.get_mut(&insertion.scope) {
                // 核对当前值的槽身份以避免误删后续替换。
                let matches_inserted_slot = fields
                    // 读取同一字段的当前值。
                    .get(&insertion.field)
                    // 只有槽身份完全相同才属于本层 journal。
                    .is_some_and(|value| value.slot_id == insertion.slot_id);
                // 删除经过身份核对的字段。
                if matches_inserted_slot {
                    // 移除本层捕获的精确字段。
                    fields.remove(&insertion.field);
                }
                // 仅在实际删除后检查作用域是否已空。
                matches_inserted_slot && fields.is_empty()
            } else {
                // 作用域已被其他生命周期路径清理时无需处理。
                false
            };
            // 清理失去最后一个字段的空作用域。
            if remove_scope {
                // 移除不再包含状态的作用域映射。
                inner.fields.remove(&insertion.scope);
            }
        }
    }
}

// 把真实新增字段登记到当前同存储捕获的 journal。
fn record_component_state_insert(
    // 接收执行实际插入的树私有存储。
    store: &ComponentStateStore,
    // 接收插入字段所属的组件作用域。
    scope: &UixComponentScope,
    // 接收字段的稳定标识。
    field: u64,
    // 接收新建句柄的唯一槽身份。
    slot_id: StateSlotId,
) {
    // 仅线程当前捕获可以拥有本次插入。
    COMPONENT_STATE_CAPTURE.with(|capture| {
        // 取得可变捕获以追加 journal 条目。
        let mut active = capture.borrow_mut();
        // 无捕获上下文时保持一次性语义。
        let Some(active) = active.as_mut() else {
            // 直接返回避免建立全局 journal。
            return;
        };
        // 嵌套捕获只能记录自己存储中的插入。
        if !Arc::ptr_eq(&active.store.inner, &store.inner) {
            // 存储不同时不污染当前捕获的回滚集。
            return;
        }
        // 登记只属于本层捕获的精确插入。
        active.insertions.push(ComponentStateInsert {
            // 复制稳定作用域值以供异常后独立回滚。
            scope: scope.clone(),
            // 保存字段标识。
            field,
            // 保存新建槽身份。
            slot_id,
        });
    });
}

// 在一次 View 捕获内安装指定窗口的临时状态上下文。
pub(crate) fn with_component_state_capture<R>(
    // 接收当前窗口树唯一的状态存储。
    store: ComponentStateStore,
    // 接收本次声明 View 构建闭包。
    build: impl FnOnce() -> R,
) -> (R, ComponentStateCaptureReceipt) {
    // 安装新的空出现序号表并保存外层捕获。
    let outer = COMPONENT_STATE_CAPTURE.with(|capture| {
        // 原子替换使嵌套捕获可在返回后恢复。
        capture.replace(Some(ComponentStateCapture {
            store,
            occurrences: HashMap::new(),
            // 本层捕获从空回滚 journal 开始。
            insertions: Vec::new(),
        }))
    });
    // 捕获 panic 以确保线程局部上下文不会泄漏到后续构建。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(build));
    // 无论构建成功与否都恢复外层并取回本层捕获。
    let finished_capture = COMPONENT_STATE_CAPTURE.with(|capture| {
        // 恢复前一个捕获或清空当前线程的临时状态。
        capture.replace(outer)
    })
    // 成对的安装与恢复必须始终取回本层捕获。
    .expect("组件状态捕获上下文意外缺失");
    // 保持调用者可观察到的正常返回或原始 panic。
    match result {
        // 正常路径把值与独立可撤销回执一同交给调用方。
        Ok(value) => {
            // 构造仅能显式提交或由 Drop 回滚的一次性回执。
            let receipt = ComponentStateCaptureReceipt::new(finished_capture);
            // 返回已成功构建的值与未提交回执。
            (value, receipt)
        }
        // 异常路径不吞掉用户 View 的 panic。
        Err(payload) => {
            // 用未提交回执的 Drop 立即撤销本层精确新增槽。
            drop(ComponentStateCaptureReceipt::new(finished_capture));
            // 恢复存储后继续原始 panic 展开。
            std::panic::resume_unwind(payload)
        }
    }
}

// 由代码生成的组件入口申请当前调用实例的稳定作用域。
pub fn uix_component_scope(callsite: &'static str, declaration: u64) -> UixComponentScope {
    // 从当前捕获读取并递增相同静态调用点的出现序号。
    COMPONENT_STATE_CAPTURE.with(|capture| {
        // 要求私有 state 只能在 Window/View 捕获期间创建。
        let mut capture = capture.borrow_mut();
        // 缺失上下文时保持一次性 View 兼容语义，不建立任何持久缓存。
        let Some(capture) = capture.as_mut() else {
            // 直接构建的 View 没有跨重建所有者，固定出现序号仅供本次标记。
            return UixComponentScope {
                callsite,
                declaration,
                occurrence: 0,
            };
        };
        // 使用调用点和声明标识区分不同的静态组件调用。
        let occurrence = capture.occurrences.entry((callsite, declaration)).or_insert(0);
        // 保存本次调用的当前出现序号。
        let current = *occurrence;
        // 为下一次同静态调用递增序号。
        *occurrence = occurrence.saturating_add(1);
        // 返回不依赖全局状态的值对象。
        UixComponentScope {
            callsite,
            declaration,
            occurrence: current,
        }
    })
}

// 由代码生成的字段初始化复用当前窗口内的组件私有 State。
pub fn uix_component_state<T>(scope: &UixComponentScope, field: u64, init: impl FnOnce() -> T) -> State<T>
where
    // 保持 State 对值类型的公开约束。
    T: Clone + Send + Sync + 'static,
{
    // 复制当前捕获的窗口专属存储句柄后释放线程局部借用。
    let store = COMPONENT_STATE_CAPTURE.with(|capture| {
        // 只复制临时上下文中的窗口存储句柄。
        capture.borrow().as_ref().map(|capture| capture.store.clone())
    });
    // 有捕获上下文时在窗口私有存储中取得或建立字段状态。
    if let Some(store) = store {
        // 保留跨 reconcile 的组件私有 State 身份。
        return store.state(scope, field, init);
    }
    // 无捕获的直接 View 构建只创建本次状态，绝不引入全局缓存。
    State::new(init())
}

// 仅在库单元测试中编译组件状态回滚门禁。
#[cfg(test)]
// 将可访问私有存储边界的测试放在仓库测试目录。
#[path = "../../tests/unit/ui/component_state_tests.rs"]
// 挂载组件状态专用行为测试。
mod tests;
