// 声明组件私有状态所需的运行时类型依赖。
use crate::ui::reactive::state::{State, StateSlotId};
// 引入动态捕获宿主的稳定组件身份。
use crate::core::WidgetId;
// 保存一次捕获期间的线程局部上下文。
use std::cell::RefCell;
// 保存状态槽的动态类型值。
use std::any::Any;
// 维护作用域映射与未决捕获 claim 集合。
use std::collections::{HashMap, HashSet};
// 让窗口树和捕获上下文安全共享状态存储。
use std::sync::{Arc, Mutex, OnceLock};

// 标识动态构建在特定宿主内复用私有状态的稳定命名空间。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WidgetStateCaptureNamespace {
    // 保存承载动态构建的实际组件身份。
    owner: WidgetId,
    // 保存代码声明的静态槽位或构建种类。
    slot: &'static str,
    // 保存跨捕获保持不变的业务实例键。
    stable_key: String,
}

// 为动态捕获命名空间提供受限构造入口。
impl WidgetStateCaptureNamespace {
    // 以宿主、静态槽位和业务键构造完整稳定身份。
    pub(crate) fn new(
        // 接收承载动态状态的组件身份。
        owner: WidgetId,
        // 接收调用方固定声明的槽位或种类。
        slot: &'static str,
        // 接收可转换为拥有字符串的稳定业务键。
        stable_key: impl Into<String>,
    ) -> Self {
        // 返回由三个身份维度共同确定的命名空间。
        Self {
            // 保留动态实例所属的实际宿主。
            owner,
            // 保留可审计的静态声明维度。
            slot,
            // 立即拥有业务键以脱离调用方借用生命周期。
            stable_key: stable_key.into(),
        }
    }
}

// 标识一次内联组件调用的稳定声明身份。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UixWidgetScope {
    // 保存宏调用点文本以隔离相同声明的不同调用位置。
    callsite: &'static str,
    // 保存代码生成器分配的组件声明标识。
    declaration: u64,
    // 保存当前捕获内相同调用点的出现序号。
    occurrence: u64,
    // 保存动态宿主实例身份；静态根捕获必须保持为空。
    namespace: Option<WidgetStateCaptureNamespace>,
    // 保存组件内部动态节点按静态声明与实际 key 派生的身份路径。
    dynamic_path: Vec<(u64, String)>,
}

// 标识实际 View 根节点承载的一个组件作用域。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct UixWidgetScopeMarker {
    // 保存私有状态所属的组件作用域。
    scope: UixWidgetScope,
    // 保存该组件在其展开根列表中的序号。
    root_ordinal: u64,
}

// 为树节点提供作用域标记的内部读取入口。
impl UixWidgetScopeMarker {
    // 构造一个不改变作用域身份的根节点标记。
    pub(crate) fn new(scope: UixWidgetScope, root_ordinal: u64) -> Self {
        // 返回完整标记以同时参与节点身份和生命周期清理。
        Self {
            scope,
            root_ordinal,
        }
    }

    // 返回状态存储清理所需的作用域身份。
    pub(crate) fn scope(&self) -> &UixWidgetScope {
        // 借用标记内的稳定作用域。
        &self.scope
    }
}

// 标识一次捕获对未提交状态槽持有的唯一 claim。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// 把 claim token 限定为组件状态模块内部的值类型。
struct WidgetStateClaimToken(u64);

// 保存组件状态存储中的动态值及其事务生命周期。
struct WidgetStateValue {
    // 保存字段首次初始化时的具体 State 类型。
    type_name: &'static str,
    // 保存精确回滚所需的状态槽身份。
    slot_id: StateSlotId,
    // 标记至少一个挂载事务已接受此状态槽。
    committed: bool,
    // 保存仍可能接受或释放此 provisional 槽的捕获 claim。
    pending_claims: HashSet<WidgetStateClaimToken>,
    // 标记最终挂载树缺席后应在最后一个外部 claim 释放时清理。
    prune_when_unclaimed: bool,
    // 保存可跨重建复用的响应式状态句柄。
    value: Box<dyn Any + Send + Sync>,
}

// 记录一次捕获对 provisional 状态槽取得的 claim。
struct WidgetStateClaim {
    // 保存 claim 所属字段的组件作用域。
    scope: UixWidgetScope,
    // 保存 claim 所属的稳定字段标识。
    field: u64,
    // 保存 claim 指向的唯一槽身份。
    slot_id: StateSlotId,
}

// 保存一个窗口树拥有的全部组件私有状态。
struct WidgetStateStoreInner {
    // 按组件作用域再按字段标识隔离状态。
    fields: HashMap<UixWidgetScope, HashMap<u64, WidgetStateValue>>,
    // 为此窗口存储中的捕获分配不重复 claim token。
    next_claim_token: u64,
    // 本树共享的键盘焦点可见响应式事实；初始值与树核心默认
    // （窗口聚焦且键盘可见）一致，按树隔离，不跨窗口共享。
    keyboard_focus_visible_fact: State<bool>,
}

impl Default for WidgetStateStoreInner {
    fn default() -> Self {
        Self {
            fields: HashMap::new(),
            next_claim_token: 0,
            keyboard_focus_visible_fact: State::new(true),
        }
    }
}

// 让单个 WidgetTree 拥有并在其所有捕获之间共享私有状态。
#[derive(Clone, Default)]
pub(crate) struct WidgetStateStore {
    // 通过互斥锁支持同一窗口捕获与回调之间的安全访问。
    inner: Arc<Mutex<WidgetStateStoreInner>>,
}

// 保存当前 View 捕获的临时状态上下文，绝不作为长期状态所有者。
struct WidgetStateCapture {
    // 指向当前 WidgetTree 专属状态存储。
    store: WidgetStateStore,
    // 标识本层捕获拥有的全部 provisional 槽 claim。
    claim_token: WidgetStateClaimToken,
    // 保存本层捕获为动态实例提供的可选状态命名空间。
    namespace: Option<WidgetStateCaptureNamespace>,
    // 为相同静态调用点分配本次捕获内的出现序号。
    occurrences: HashMap<(&'static str, u64), u64>,
    // 记录本层捕获取得的全部 provisional 槽 claim。
    claims: Vec<WidgetStateClaim>,
}

// 代表一次成功捕获尚未被挂载事务接受的状态 claim。
pub(crate) struct WidgetStateCaptureReceipt {
    // 保存本次捕获访问的树私有存储。
    store: WidgetStateStore,
    // 保存本次捕获唯一的 claim token。
    claim_token: WidgetStateClaimToken,
    // 保存尚待接受或释放的精确 claim journal。
    claims: Vec<WidgetStateClaim>,
    // 标记挂载事务已经处理这些 claim。
    accepted: bool,
}

// 管理组件状态捕获与挂载事务之间的两阶段交接。
impl WidgetStateCaptureReceipt {
    // 从已结束的线程局部捕获创建未提交回执。
    fn new(finished: WidgetStateCapture) -> Self {
        // 返回由调用方明确决定提交的一次性所有权。
        Self {
            // 转移本次捕获使用的存储句柄。
            store: finished.store,
            // 转移本次捕获的唯一 claim token。
            claim_token: finished.claim_token,
            // 转移本层独立记录的 claim 集。
            claims: finished.claims,
            // 初始状态必须在丢弃时释放 claim。
            accepted: false,
        }
    }

    // 由成功挂载或协调事务接受本次状态 claim。
    fn commit(mut self) {
        // 把本回执仍指向的 provisional 槽转为已提交状态。
        self.store.commit_claims(self.claim_token, &self.claims);
        // 标记 Drop 不再释放已经处理的 claim。
        self.accepted = true;
    }

    // 判断回执是否由指定 WidgetTree 的唯一状态存储产生。
    pub(crate) fn belongs_to(&self, store: &WidgetStateStore) -> bool {
        // 共享同一个 Arc 内核才代表相同窗口树所有权。
        Arc::ptr_eq(&self.store.inner, &store.inner)
    }
}

// 仅允许树事务协调器在同一批次内原子接纳全部成功捕获的 journal。
pub(crate) fn accept_widget_state_receipts(
    // 接收已经通过最外层 WidgetTree 事务确认的回执集合。
    receipts: Vec<WidgetStateCaptureReceipt>,
) {
    // 逐个消费回执以关闭其 Drop 回滚路径。
    for receipt in receipts {
        // 标记该 journal 已作为同批树事务的一部分被接纳。
        receipt.commit();
    }
}

// 按最外层事务的最终挂载作用域解析本批状态 claim 并执行清理。
pub(crate) fn resolve_widget_state_receipts(
    // 接收当前 WidgetTree 唯一拥有的状态存储。
    store: &WidgetStateStore,
    // 接收最外层事务累计的全部捕获回执。
    receipts: Vec<WidgetStateCaptureReceipt>,
    // 接收事务成功后实际由节点标记承载的作用域集合。
    live_scopes: &HashSet<UixWidgetScope>,
) {
    // 在存储内部原子完成本批接纳、释放与最终树清理。
    store.resolve_receipts_and_retain_scopes(receipts, live_scopes);
}

// 确保未成功交付给 WidgetTree 的捕获不泄漏私有状态。
impl Drop for WidgetStateCaptureReceipt {
    // 在回执离开作用域时执行提交或回滚的终态处理。
    fn drop(&mut self) {
        // 已接纳回执不能再次修改树状态。
        if self.accepted {
            // 直接返回保留已经提交的状态槽。
            return;
        }
        // 取出 journal 使重入 Drop 也不会重复释放。
        let claims = std::mem::take(&mut self.claims);
        // 释放本回执的 claim 并清理无人认领的 provisional 槽。
        self.store.release_claims(self.claim_token, claims);
    }
}

// 仅在构建 View 时暴露当前树的状态存储上下文。
thread_local! {
    // 嵌套捕获通过替换和恢复避免跨窗口泄漏。
    static COMPONENT_STATE_CAPTURE: RefCell<Option<WidgetStateCapture>> = const { RefCell::new(None) };
}

// 为给定窗口树创建独立的组件私有状态存储。
impl WidgetStateStore {
    // 取得本树共享的键盘焦点可见响应式事实。
    //
    // 生成的 `:focus-visible` 状态层在所属树捕获内读取本事实，
    // 事实翻转只重建本树命中的声明式视图。
    pub(crate) fn keyboard_focus_visible_fact(&self) -> State<bool> {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keyboard_focus_visible_fact
            .clone()
    }

    // 由树核心在有效事实变化时同步本树响应式槽。
    pub(crate) fn sync_keyboard_focus_visible_fact(&self, visible: bool) {
        // 值未变化时跳过，避免无谓推进代数触发重建。
        if self.keyboard_focus_visible_fact().get_untracked() != visible {
            self.keyboard_focus_visible_fact().set(visible);
        }
    }

    // 创建空存储以供一个 WidgetTree 独占。
    pub(crate) fn new() -> Self {
        // 返回默认的并发安全状态容器。
        Self::default()
    }

    // 为即将安装的捕获分配此存储内唯一的 claim token。
    fn allocate_claim_token(&self) -> WidgetStateClaimToken {
        // 锁定存储内的单调 token 分配器。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 读取本次捕获独占的 token 数值。
        let token = WidgetStateClaimToken(inner.next_claim_token);
        // 对最大值执行受检递增以禁止 token 重复。
        let Some(next_claim_token) = inner.next_claim_token.checked_add(1) else {
            // token 重复会破坏 claim 所有权，因此不可静默回绕。
            panic!("组件状态捕获 claim token 已耗尽");
        };
        // 推进分配器供下一次捕获使用。
        inner.next_claim_token = next_claim_token;
        // 返回只属于本次捕获的 token。
        token
    }

    // 复用现有字段状态或首次建立指定字段的 State。
    fn state<T>(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收组件声明与动态实例的完整作用域。
        scope: &UixWidgetScope,
        // 接收作用域内的稳定字段标识。
        field: u64,
        // 接收当前捕获唯一的 claim token。
        claim_token: WidgetStateClaimToken,
        // 接收仅在字段缺失时执行的初始化闭包。
        init: impl FnOnce() -> T,
    ) -> State<T>
    where
        // 约束 State 的公开线程安全契约。
        T: Clone + Send + Sync + 'static,
    {
        // 在执行初始化闭包前检查并 claim 已有槽，避免闭包重入时持锁死锁。
        if let Some(existing) = self.existing_state::<T>(scope, field, claim_token) {
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
        if let Some(value) = fields.get_mut(&field) {
            // 同类型字段保持第一次建立的状态为准。
            let existing = value
                // 读取字段保存的动态 State 句柄。
                .value
                // 按本次请求的具体类型执行安全向下转换。
                .downcast_ref::<State<T>>()
                // 克隆共享句柄以结束对动态值的不可变借用。
                .cloned()
                // 类型变化是宏稳定字段标识违反契约。
                .unwrap_or_else(|| {
                    // 明确报告宏字段标识被不兼容类型复用的契约错误。
                    Self::type_mismatch(scope, field, value.type_name, std::any::type_name::<T>())
                });
            // 让本捕获对初始化期间出现的状态槽取得独立 claim。
            self.claim_state_slot(value, scope, field, claim_token);
            // 丢弃本次未挂载候选并返回既有句柄。
            return existing;
        }
        // 记录类型名以便未来错误精确诊断。
        let type_name = std::any::type_name::<T>();
        // 记录候选句柄的唯一槽身份。
        let slot_id = state.slot_id();
        // 构造尚未被任何挂载事务接受的 provisional 槽。
        let mut value = WidgetStateValue {
            // 保存首次初始化时的具体 State 类型。
            type_name,
            // 把精确槽身份与动态值一同保存。
            slot_id,
            // 新建槽必须等到回执接纳后才能提交。
            committed: false,
            // 新建槽从空 claim 集开始。
            pending_claims: HashSet::new(),
            // 尚未经过最终挂载树清理的新槽不预设延迟清理。
            prune_when_unclaimed: false,
            // 保存可跨重建复用的响应式句柄。
            value: Box::new(state.clone()),
        };
        // 让创建本槽的捕获先取得独立 claim。
        self.claim_state_slot(&mut value, scope, field, claim_token);
        // 把带 claim 的新状态归属到本窗口组件作用域。
        fields.insert(field, value);
        // 返回首次建立的状态句柄。
        state
    }

    // 查询并 claim 已存在字段，在类型不匹配时给出稳定诊断。
    fn existing_state<T>(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收组件声明与动态实例的完整作用域。
        scope: &UixWidgetScope,
        // 接收作用域内的稳定字段标识。
        field: u64,
        // 接收当前捕获唯一的 claim token。
        claim_token: WidgetStateClaimToken,
    ) -> Option<State<T>>
    where
        // 保持 State 的公开线程安全契约。
        T: Clone + Send + Sync + 'static,
    {
        // 锁定后读取并更新已建立字段的 claim 集。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 未建立作用域或字段时允许首次初始化。
        let value = inner.fields.get_mut(scope)?.get_mut(&field)?;
        // 同类型字段返回原有状态句柄。
        let state = value
            // 读取字段保存的动态 State 句柄。
            .value
            // 按本次请求的具体类型执行安全向下转换。
            .downcast_ref::<State<T>>()
            // 克隆共享句柄以结束对动态值的不可变借用。
            .cloned()
            // 类型变化是宏稳定字段标识违反契约。
            .unwrap_or_else(|| {
                // 明确报告宏字段标识被不兼容类型复用的契约错误。
                Self::type_mismatch(scope, field, value.type_name, std::any::type_name::<T>())
            });
        // 每次捕获都需要为访问的槽登记独立 claim。
        self.claim_state_slot(value, scope, field, claim_token);
        // 返回克隆句柄而不是分配新的 State 槽。
        Some(state)
    }

    // 在持有存储锁时为本捕获登记一个状态槽 claim。
    fn claim_state_slot(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收本次捕获实际访问的状态值。
        value: &mut WidgetStateValue,
        // 接收 claim 所属的完整组件作用域。
        scope: &UixWidgetScope,
        // 接收 claim 所属的稳定字段标识。
        field: u64,
        // 接收当前捕获唯一的 claim token。
        claim_token: WidgetStateClaimToken,
    ) {
        // 同一捕获重复访问同一槽时只保留一个 claim。
        if value.pending_claims.contains(&claim_token) {
            // 直接返回避免 receipt 重复释放同一 token。
            return;
        }
        // 先登记回执 journal，使后续存储写入异常时仍可安全释放。
        record_widget_state_claim(self, scope, field, value.slot_id, claim_token);
        // journal 已取得所有权后把 token 加入槽的未决 claim 集。
        value.pending_claims.insert(claim_token);
    }

    // 统一生成字段类型不兼容的可诊断 panic。
    fn type_mismatch(
        scope: &UixWidgetScope,
        field: u64,
        existing: &'static str,
        requested: &'static str,
    ) -> ! {
        // 明确列出所有稳定身份部分以便定位代码生成错误。
        panic!(
            "uix 组件私有 state 类型不匹配：callsite={} declaration={} occurrence={} field={}，已有 {}，请求 {}",
            scope.callsite, scope.declaration, scope.occurrence, field, existing, requested,
        );
    }

    // 删除树上已无任何实际节点承载的组件作用域。
    pub(crate) fn retain_scopes(&self, live_scopes: &HashSet<UixWidgetScope>) {
        // 锁定后一次性按最终挂载真相与外部 claim 收敛状态。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 清除未挂载且没有外部捕获保护的作用域与字段。
        Self::retain_live_or_claimed_fields(&mut inner, live_scopes);
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

    // 接纳一次捕获的全部 claim 并提交仍指向原槽的字段。
    fn commit_claims(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收本次回执唯一的 claim token。
        claim_token: WidgetStateClaimToken,
        // 接收本次回执取得的精确 claim journal。
        claims: &[WidgetStateClaim],
    ) {
        // 一次锁定完成本回执全部 claim 的提交。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 逐个提交仍由本回执认领的 provisional 槽。
        for claim in claims {
            // 缺失作用域表示槽已经被树生命周期清理。
            let Some(fields) = inner.fields.get_mut(&claim.scope) else {
                // 已清理槽无需恢复或重新创建。
                continue;
            };
            // 缺失字段表示本 claim 已经失效。
            let Some(value) = fields.get_mut(&claim.field) else {
                // 已失效 claim 不得影响后续同身份字段。
                continue;
            };
            // 槽身份变化表示字段已经被其他生命周期替换。
            if value.slot_id != claim.slot_id {
                // 过期 claim 不得提交替换后的槽。
                continue;
            }
            // 只有本回执确实持有该 token 时才能提交槽。
            if value.pending_claims.remove(&claim_token) {
                // 任一有效挂载事务接受后，该槽即成为稳定已提交状态。
                value.committed = true;
                // 显式接纳表示此槽不再等待缺席作用域的延迟清理。
                value.prune_when_unclaimed = false;
            }
        }
    }

    // 原子解析最外层本批回执，并在同一锁内按最终树清理状态。
    fn resolve_receipts_and_retain_scopes(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收最外层事务累计的全部捕获回执。
        mut receipts: Vec<WidgetStateCaptureReceipt>,
        // 接收事务成功后实际挂载的完整作用域集合。
        live_scopes: &HashSet<UixWidgetScope>,
    ) {
        // 锁定后把回执解析与 prune 合并为单一线性化点。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 逐个解析属于本窗口存储的回执。
        for receipt in &mut receipts {
            // 错误存储回执保留未接纳状态，离开本函数时自行释放。
            if !receipt.belongs_to(self) {
                // 跳过其他 WidgetTree 的状态所有权。
                continue;
            }
            // 取出 claim journal 使回执 Drop 不会重复处理。
            let claims = std::mem::take(&mut receipt.claims);
            // 标记本回执已经由最终挂载事务完整解析。
            receipt.accepted = true;
            // 逐个按 claim 所属作用域的最终挂载状态处理。
            for claim in claims {
                // 最终树仍承载该作用域时接受对应状态槽。
                if live_scopes.contains(&claim.scope) {
                    // 仅提交仍指向原槽且由本回执持有的 claim。
                    if let Some(value) = inner
                        // 定位 claim 所属的组件作用域。
                        .fields
                        // 取得该作用域的字段映射。
                        .get_mut(&claim.scope)
                        // 定位 claim 所属字段。
                        .and_then(|fields| fields.get_mut(&claim.field))
                        // 排除已被生命周期替换的过期槽。
                        .filter(|value| value.slot_id == claim.slot_id)
                    {
                        // 只有本回执确实持有 token 时才能提交槽。
                        if value.pending_claims.remove(&receipt.claim_token) {
                            // 最终挂载真相把 provisional 槽转为 committed。
                            value.committed = true;
                            // 重新挂载取消先前缺席 prune 的延迟清理标记。
                            value.prune_when_unclaimed = false;
                        }
                    }
                    // 当前 live claim 已经完整解析，无需执行缺席释放。
                    continue;
                }
                // 最终树缺席时释放本批 claim，并在无人认领时删除字段。
                Self::release_claim_from_inner(
                    // 传入已经锁定的窗口存储内核。
                    &mut inner,
                    // 传入本回执唯一 token。
                    receipt.claim_token,
                    // 传入待释放的精确 claim。
                    &claim,
                    // 最终树已证明作用域缺席，因此无论旧提交状态都可清理。
                    true,
                );
            }
        }
        // 本批解析后保留最终挂载或仍由树外捕获 claim 保护的字段。
        Self::retain_live_or_claimed_fields(&mut inner, live_scopes);
    }

    // 释放一次失败或未挂载捕获的全部 provisional 槽 claim。
    fn release_claims(
        // 接收拥有状态槽的窗口存储。
        &self,
        // 接收本次回执唯一的 claim token。
        claim_token: WidgetStateClaimToken,
        // 接收本次回执取得的精确 claim journal。
        claims: Vec<WidgetStateClaim>,
    ) {
        // 一次锁定完成本回执全部 claim 的释放。
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        // 按 claim 的逆序释放以保持重入初始化的栈语义。
        for claim in claims.into_iter().rev() {
            // 释放精确 claim，并遵循槽记录的延迟 prune 标记。
            Self::release_claim_from_inner(&mut inner, claim_token, &claim, false);
        }
    }

    // 在已锁定的存储内核中释放单个 claim 并按终态决定清理。
    fn release_claim_from_inner(
        // 接收已经锁定的窗口存储内核。
        inner: &mut WidgetStateStoreInner,
        // 接收待释放回执的唯一 token。
        claim_token: WidgetStateClaimToken,
        // 接收待释放的精确状态槽 claim。
        claim: &WidgetStateClaim,
        // 指示最终树已经明确证明该作用域缺席。
        scope_confirmed_absent: bool,
    ) {
        // 仅在释放最后一个有效 claim 时删除允许清理的字段。
        let remove_scope = if let Some(fields) = inner.fields.get_mut(&claim.scope) {
            // 判断本 claim 释放后字段是否失去全部生命周期持有者。
            let remove_field = if let Some(value) = fields.get_mut(&claim.field) {
                // 过期 claim 不得影响后来替换的同身份字段。
                if value.slot_id != claim.slot_id {
                    // 槽身份不匹配时不执行任何删除。
                    false
                } else {
                    // 只释放属于本回执的唯一 token。
                    let released = value.pending_claims.remove(&claim_token);
                    // 仅最后一个 claim 释放后才允许清理该状态槽。
                    released
                        // 要求槽不再由任何其他捕获保护。
                        && value.pending_claims.is_empty()
                        // 未提交槽、明确缺席槽或延迟 prune 槽均应删除。
                        && (!value.committed
                            || scope_confirmed_absent
                            || value.prune_when_unclaimed)
                }
            } else {
                // 字段已由其他生命周期路径清理时无需处理。
                false
            };
            // 删除经过终态与 claim 数量双重核对的字段。
            if remove_field {
                // 移除不再拥有任何生命周期持有者的字段。
                fields.remove(&claim.field);
            }
            // 仅在实际删除后检查作用域是否已空。
            remove_field && fields.is_empty()
        } else {
            // 作用域已由其他生命周期路径清理时无需处理。
            false
        };
        // 清理失去最后一个字段的空作用域。
        if remove_scope {
            // 移除不再包含状态的作用域映射。
            inner.fields.remove(&claim.scope);
        }
    }

    // 在已锁定内核中保留最终挂载或仍有树外 claim 的字段。
    fn retain_live_or_claimed_fields(
        // 接收已经锁定的窗口存储内核。
        inner: &mut WidgetStateStoreInner,
        // 接收当前实际由节点标记承载的作用域集合。
        live_scopes: &HashSet<UixWidgetScope>,
    ) {
        // 按完整作用域遍历并收敛其全部字段。
        inner.fields.retain(|scope, fields| {
            // 实际挂载作用域保留全部状态并取消延迟清理。
            if live_scopes.contains(scope) {
                // 逐个清除先前缺席 prune 留下的清理标记。
                for value in fields.values_mut() {
                    // 实际节点重新承载后不得在外部 claim 释放时删除。
                    value.prune_when_unclaimed = false;
                }
                // 保留实际挂载作用域的完整字段集合。
                return true;
            }
            // 缺席作用域只保留仍由树外捕获 claim 保护的字段。
            fields.retain(|_, value| {
                // 判断字段是否仍有捕获可能在未来挂载它。
                let claimed = !value.pending_claims.is_empty();
                // 被保护字段等待最后一个 claim 的挂载或释放终态。
                if claimed {
                    // 标记最后一个 claim 未挂载释放时应完成延迟清理。
                    value.prune_when_unclaimed = true;
                }
                // 无节点且无 claim 的字段立即清理。
                claimed
            });
            // 仅在仍有被保护字段时保留缺席作用域容器。
            !fields.is_empty()
        });
    }
}

// 把 provisional 槽 claim 登记到当前同存储捕获的 journal。
fn record_widget_state_claim(
    // 接收保存 provisional 槽的树私有存储。
    store: &WidgetStateStore,
    // 接收 claim 所属字段的组件作用域。
    scope: &UixWidgetScope,
    // 接收 claim 所属字段的稳定标识。
    field: u64,
    // 接收 claim 指向的唯一槽身份。
    slot_id: StateSlotId,
    // 接收当前捕获唯一的 claim token。
    claim_token: WidgetStateClaimToken,
) {
    // 仅线程当前捕获可以拥有本次 claim。
    COMPONENT_STATE_CAPTURE.with(|capture| {
        // 取得可变捕获以追加 journal 条目。
        let mut active = capture.borrow_mut();
        // 状态访问的 store 与 token 都来自当前捕获，因此上下文必须存在。
        let active = active.as_mut();
        // 缺失上下文说明内部状态访问契约已被破坏。
        let Some(active) = active else {
            // 不允许把 claim 静默登记到错误生命周期。
            panic!("组件状态 claim 缺少活动捕获上下文");
        };
        // 嵌套捕获只能记录自己存储中的 claim。
        assert!(
            // 精确比较 Arc 内核以核对窗口树所有权。
            Arc::ptr_eq(&active.store.inner, &store.inner),
            // 存储错配必须立即暴露，不能留下无人管理的 provisional 槽。
            "组件状态 claim 与活动捕获存储不匹配",
        );
        // token 不匹配时禁止把 claim 归属给其他嵌套捕获。
        assert_eq!(
            // 读取活动捕获实际拥有的 token。
            active.claim_token,
            // 对比状态访问携带的预期 token。
            claim_token,
            // token 错配必须立即暴露，不能破坏捕获独立生命周期。
            "组件状态 claim token 与活动捕获不匹配",
        );
        // 登记只属于本层捕获的精确 claim。
        active.claims.push(WidgetStateClaim {
            // 复制稳定作用域值以供异常后独立回滚。
            scope: scope.clone(),
            // 保存字段标识。
            field,
            // 保存 claim 指向的槽身份。
            slot_id,
        });
    });
}

// 在一次 View 捕获内安装指定窗口的临时状态上下文。
pub(crate) fn with_widget_state_capture<R>(
    // 接收当前窗口树唯一的状态存储。
    store: WidgetStateStore,
    // 接收本次声明 View 构建闭包。
    build: impl FnOnce() -> R,
) -> (R, WidgetStateCaptureReceipt) {
    // 静态根捕获不附加动态实例命名空间。
    with_widget_state_capture_with_namespace(store, None, build)
}

// 在动态宿主命名空间内安装一次可回滚的组件状态捕获。
pub(crate) fn with_widget_state_capture_in_namespace<R>(
    // 接收当前窗口树唯一的状态存储。
    store: WidgetStateStore,
    // 接收动态实例的稳定身份命名空间。
    namespace: WidgetStateCaptureNamespace,
    // 接收本次动态 View 构建闭包。
    build: impl FnOnce() -> R,
) -> (R, WidgetStateCaptureReceipt) {
    // 复用静态捕获的同一 TLS 与回执事务实现。
    with_widget_state_capture_with_namespace(store, Some(namespace), build)
}

// 在可选命名空间内执行共享的捕获、恢复和回滚协议。
fn with_widget_state_capture_with_namespace<R>(
    // 接收当前窗口树唯一的状态存储。
    store: WidgetStateStore,
    // 接收静态根或动态实例的可选命名空间。
    namespace: Option<WidgetStateCaptureNamespace>,
    // 接收本次声明 View 构建闭包。
    build: impl FnOnce() -> R,
) -> (R, WidgetStateCaptureReceipt) {
    // 在安装 TLS 前为本层捕获取得唯一 claim token。
    let claim_token = store.allocate_claim_token();
    // 安装新的空出现序号表并保存外层捕获。
    let outer = COMPONENT_STATE_CAPTURE.with(|capture| {
        // 原子替换使嵌套捕获可在返回后恢复。
        capture.replace(Some(WidgetStateCapture {
            store,
            // 保存本层 provisional 槽生命周期所用的唯一 token。
            claim_token,
            // 让同一捕获内生成的作用域继承动态实例身份。
            namespace,
            occurrences: HashMap::new(),
            // 本层捕获从空 claim journal 开始。
            claims: Vec::new(),
        }))
    });
    // 捕获 panic 以确保线程局部上下文不会泄漏到后续构建。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(build));
    // 无论构建成功与否都恢复外层并取回本层捕获。
    // 恢复前一个捕获或清空当前线程的临时状态。
    let finished_capture = COMPONENT_STATE_CAPTURE.with(|capture| capture.replace(outer));
    // 成对的安装与恢复必须始终取回本层捕获。
    let Some(finished_capture) = finished_capture else {
        // 缺失本层捕获表示线程局部生命周期已被破坏。
        panic!("组件状态捕获上下文意外缺失");
    };
    // 保持调用者可观察到的正常返回或原始 panic。
    match result {
        // 正常路径把值与独立可撤销回执一同交给调用方。
        Ok(value) => {
            // 构造仅能显式提交或由 Drop 回滚的一次性回执。
            let receipt = WidgetStateCaptureReceipt::new(finished_capture);
            // 返回已成功构建的值与未提交回执。
            (value, receipt)
        }
        // 异常路径不吞掉用户 View 的 panic。
        Err(payload) => {
            // 用未提交回执的 Drop 立即撤销本层精确新增槽。
            drop(WidgetStateCaptureReceipt::new(finished_capture));
            // 恢复存储后继续原始 panic 展开。
            std::panic::resume_unwind(payload)
        }
    }
}

// 由代码生成的组件入口申请当前调用实例的稳定作用域。
pub fn uix_widget_scope(callsite: &'static str, declaration: u64) -> UixWidgetScope {
    // 从当前捕获读取并递增相同静态调用点的出现序号。
    COMPONENT_STATE_CAPTURE.with(|capture| {
        // 要求私有 state 只能在 Window/View 捕获期间创建。
        let mut capture = capture.borrow_mut();
        // 缺失上下文时保持一次性 View 兼容语义，不建立任何持久缓存。
        let Some(capture) = capture.as_mut() else {
            // 直接构建的 View 没有跨重建所有者，固定出现序号仅供本次标记。
            return UixWidgetScope {
                callsite,
                declaration,
                occurrence: 0,
                // 无捕获上下文的静态根不具备动态实例所有者。
                namespace: None,
                // 组件根尚未进入任何动态节点子作用域。
                dynamic_path: Vec::new(),
            };
        };
        // 使用调用点和声明标识区分不同的静态组件调用。
        let occurrence = capture
            .occurrences
            .entry((callsite, declaration))
            .or_insert(0);
        // 保存本次调用的当前出现序号。
        let current = *occurrence;
        // 为下一次同静态调用递增序号。
        *occurrence = occurrence.saturating_add(1);
        // 返回不依赖全局状态的值对象。
        UixWidgetScope {
            callsite,
            declaration,
            occurrence: current,
            // 复制本层动态命名空间以使其参与状态键的相等性比较。
            namespace: capture.namespace.clone(),
            // 组件根尚未进入任何动态节点子作用域。
            dynamic_path: Vec::new(),
        }
    })
}

// 为组件内部实际节点派生不依赖全局注册表的稳定子作用域。
pub fn uix_widget_child_scope(
    // 接收最近组件或父动态节点作用域。
    parent: &UixWidgetScope,
    // 接收节点类型与静态位置形成的声明标识。
    declaration: u64,
    // 接收当前实际节点的 key 或稳定位置路径。
    stable_key: impl Into<String>,
) -> UixWidgetScope {
    // 克隆父作用域以保留组件实例与动态宿主命名空间。
    let mut child = parent.clone();
    // 追加当前节点身份，使 key 或类型变化自然释放旧状态槽。
    child.dynamic_path.push((declaration, stable_key.into()));
    // 返回只属于当前实际节点的作用域。
    child
}

// 由代码生成的字段初始化复用当前窗口内的组件私有 State。
pub fn uix_widget_state<T>(scope: &UixWidgetScope, field: u64, init: impl FnOnce() -> T) -> State<T>
where
    // 保持 State 对值类型的公开约束。
    T: Clone + Send + Sync + 'static,
{
    // 复制当前捕获的存储句柄与 claim token 后释放线程局部借用。
    let capture_context = COMPONENT_STATE_CAPTURE.with(|capture| {
        // 只复制临时上下文中的窗口存储句柄与值类型 token。
        capture
            // 临时借用当前线程的可选捕获上下文。
            .borrow()
            // 只处理实际安装的捕获上下文。
            .as_ref()
            // 提取状态访问所需的最小所有权信息。
            .map(|capture| (capture.store.clone(), capture.claim_token))
    });
    // 有捕获上下文时在窗口私有存储中取得或建立字段状态。
    // 有捕获上下文时在窗口私有存储中取得或建立字段状态。
    if let Some((store, claim_token)) = capture_context {
        // 保留跨 reconcile 的组件私有 State 身份。
        return store.state(scope, field, claim_token, init);
    }
    // 无捕获的直接 View 构建只创建本次状态，绝不引入全局缓存。
    State::new(init())
}

// 仅在库单元测试中编译组件状态回滚门禁。

// 供生成的 `:focus-visible` 状态层读取键盘焦点可见事实。
//
// 事实按既有 WidgetTree/窗口归属提供：有捕获上下文时返回所属树
// 状态存储共享的树级事实句柄，键盘焦点可见翻转只重建本树命中的
// 视图，多树/多窗口互不污染；在视图构建中调用 `get()` 会登记重建
// 依赖。无捕获的直接 View 构建没有跨重建所有者，仅提供本次事实。
pub fn uix_keyboard_focus_visible_fact() -> State<bool> {
    // 复制当前捕获的存储句柄后释放线程局部借用。
    let capture_context = COMPONENT_STATE_CAPTURE.with(|capture| {
        capture
            .borrow()
            .as_ref()
            .map(|capture| capture.store.clone())
    });
    // 有捕获上下文时返回当前树共享的树级事实。
    if let Some(store) = capture_context {
        return store.keyboard_focus_visible_fact();
    }
    // 无捕获的直接 View 构建只创建本次状态，绝不引入全局缓存。
    State::new(true)
}
