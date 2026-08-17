// Wayland 指针激活注册表只保存协议授权的最小私有状态。
use std::collections::HashMap;
// 共享注册表句柄只在检查式入口的短锁内访问。
use std::sync::{Arc, Mutex};

// 窗口身份用于阻止其他窗口消费当前指针授权。
use crate::core::WindowId;
// typed error 让同步窗口操作观察注册表 owner 损坏。
use crate::core::error::{Errc, Error, Result};
// 不透明激活身份用于关联 native 事件与稍后的同步窗口动作。
use crate::native::windowing::event::PointerActivationId;

// surface 注册代次防止协议对象编号复用旧授权。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 注册项只属于 Wayland windowing 后端。
struct SurfaceRegistration {
    // 记录当前 surface 所属的稳定窗口身份。
    window_id: WindowId,
    // 记录每次 surface 登记的单调代次。
    generation: u64,
    // 结束 surface 注册项定义。
}

// 单个 seat 的主指针键同一时刻最多保留一个未消费授权。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 原始 serial 永远不离开 Wayland 私有组件。
struct PendingPrimaryActivation {
    // 关联 native PointerDown 的不可解释身份。
    activation_id: PointerActivationId,
    // 绑定授权产生时的 pointer 代理代次。
    pointer_generation: u64,
    // 绑定授权产生时的 surface 协议身份。
    surface_id: u32,
    // 绑定授权产生时的 surface 注册代次。
    surface_generation: u64,
    // 绑定授权产生时的应用窗口身份。
    window_id: WindowId,
    // 保存 compositor 为该次 BTN_LEFT press 签发的 serial。
    serial: u32,
    // 结束待消费主键授权定义。
}

// 稳定区分正常竞态下拒绝拖动请求的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 拒绝原因只用于 Wayland 调试诊断，不进入 app 或 UI 契约。
pub(crate) enum PointerActivationRejection {
    // 当前 seat 没有未消费的主键授权。
    MissingOrRevoked,
    // 当前事件身份不属于仍待消费的授权。
    ActivationMismatch,
    // 授权属于另一个应用窗口。
    WindowMismatch,
    // 授权属于同窗的另一个原生 surface。
    SurfaceMismatch,
    // pointer 能力已丢失并以新代次恢复。
    StalePointerGeneration,
    // surface 已注销或以新代次重新登记。
    StaleSurfaceRegistration,
    // 结束正常拒绝原因枚举。
}

// 消费结果明确区分已提交所需 serial 与安全忽略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 该结果不会把 raw serial 暴露到 Wayland 模块之外。
pub(crate) enum PointerActivationOutcome {
    // 返回精确匹配且已从注册表取走的协议 serial。
    Authorized {
        // serial 只由 WaylandWindowOps 立即提交给 xdg_toplevel。
        serial: u32,
        // 结束授权成功负载。
    },
    // 正常竞态或身份不匹配必须保持非致命。
    Ignored(PointerActivationRejection),
    // 结束授权消费结果枚举。
}

// Wayland 后端是 surface 注册、pointer 代次与待消费授权的唯一所有者。
#[derive(Debug)]
// 注册表通过短时互斥访问，锁内绝不调用 Wayland 或 UI。
pub(crate) struct WaylandPointerActivationRegistry {
    // 下一个不可解释激活身份从一开始并跳过零。
    next_activation_id: u64,
    // 下一个 surface 注册代次从一开始并跳过零。
    next_surface_generation: u64,
    // 当前 pointer 代理代次在 capability loss 时递增。
    pointer_generation: u64,
    // 当前活跃 surface 注册按协议对象身份索引。
    surfaces: HashMap<u32, SurfaceRegistration>,
    // 仅保存当前 seat 尚未消费的 BTN_LEFT press 授权。
    pending_primary: Option<PendingPrimaryActivation>,
    // 结束 Wayland 指针激活注册表定义。
}

// 默认构造必须建立非零代次与身份空间。
impl Default for WaylandPointerActivationRegistry {
    // 委托唯一构造器保持计数器不变量。
    fn default() -> Self {
        // 返回初始化后的私有注册表。
        Self::new()
        // 结束默认构造。
    }
    // 结束默认实现。
}

// 集中实现授权签发、撤销与原子消费生命周期。
impl WaylandPointerActivationRegistry {
    // 建立空注册表并保留零作为“未签发”哨兵。
    pub(crate) fn new() -> Self {
        // 返回没有 surface 或待消费授权的初始状态。
        Self {
            // 首个激活身份为一。
            next_activation_id: 1,
            // 首个 surface 注册代次为一。
            next_surface_generation: 1,
            // 首个 pointer 代理使用代次一。
            pointer_generation: 1,
            // 初始化空 surface 注册索引。
            surfaces: HashMap::new(),
            // 初始没有主键授权。
            pending_primary: None,
            // 结束初始状态字面量。
        }
        // 结束注册表构造器。
    }

    // 取得当前非零计数值并安全推进到下一个值。
    fn take_counter(counter: &mut u64) -> u64 {
        // 保存本次要签发的稳定值。
        let current = *counter;
        // 使用环绕递增处理理论上的 u64 上界。
        *counter = counter.wrapping_add(1);
        // 零只表示未签发，环绕后立即跳回一。
        if *counter == 0 {
            // 恢复非零计数空间。
            *counter = 1;
            // 结束零值修正。
        }
        // 返回本次签发值。
        current
        // 结束计数推进辅助方法。
    }

    // 返回创建 pointer 回调时需要捕获的稳定代次。
    pub(crate) const fn pointer_generation(&self) -> u64 {
        // 复制当前代次而不暴露可变所有权。
        self.pointer_generation
        // 结束 pointer 代次读取。
    }

    // 登记一个新 surface，并使同协议编号上的旧授权失效。
    pub(crate) fn register_surface(&mut self, surface_id: u32, window_id: WindowId) {
        // 每次登记都签发新的 surface 代次。
        let generation = Self::take_counter(&mut self.next_surface_generation);
        // 协议编号重新登记前先撤销该编号上的旧待消费授权。
        if self
            // 检查是否存在同 surface 编号的待消费授权。
            .pending_primary
            // 只比较后端私有 surface 身份。
            .is_some_and(|pending| pending.surface_id == surface_id)
        // 命中旧授权时进入撤销分支。
        {
            // 丢弃旧代次授权，避免对象编号复用。
            self.pending_primary = None;
            // 结束旧授权撤销。
        }
        // 以最新窗口与代次替换 surface 注册事实。
        self.surfaces.insert(
            // 使用协议对象编号作为当前注册索引。
            surface_id,
            // 保存新登记事实。
            SurfaceRegistration {
                // 绑定稳定窗口身份。
                window_id,
                // 绑定本次单调代次。
                generation,
                // 结束 surface 注册事实。
            },
            // 结束注册索引更新。
        );
        // 结束 surface 登记。
    }

    // 注销仍属于指定窗口的 surface 及其未消费授权。
    pub(crate) fn unregister_surface(&mut self, surface_id: u32, window_id: WindowId) {
        // 只读取当前注册，防止旧窗口注销已复用的协议编号。
        let Some(registration) = self.surfaces.get(&surface_id).copied() else {
            // 重复注销保持幂等。
            return;
            // 结束缺失注册分支。
        };
        // 旧窗口不得删除后来窗口的新注册。
        if registration.window_id != window_id {
            // 保留当前有效注册与授权。
            return;
            // 结束窗口身份保护。
        }
        // 删除精确匹配的当前 surface 注册。
        self.surfaces.remove(&surface_id);
        // 检查待消费授权是否绑定被注销的完整注册代次。
        if self.pending_primary.is_some_and(|pending| {
            // 同时匹配 surface 编号、代次和窗口身份。
            pending.surface_id == surface_id
                // 保证协议编号复用不会误撤销新授权。
                && pending.surface_generation == registration.generation
                // 保证多窗口隔离。
                && pending.window_id == window_id
            // 结束待消费授权匹配闭包。
        }) {
            // 被注销 surface 的授权立即失效。
            self.pending_primary = None;
            // 结束注销授权撤销。
        }
        // 结束 surface 注销。
    }

    // 为当前 pointer 代次上的 BTN_LEFT press 签发一次性身份。
    pub(crate) fn issue_primary_press(
        // 注册表是签发身份与 raw serial 的唯一所有者。
        &mut self,
        // 回调捕获代次阻止旧 pointer 代理写入新状态。
        pointer_generation: u64,
        // press 当时获得焦点的 surface 编号。
        surface_id: u32,
        // press 当时解析出的窗口身份。
        window_id: WindowId,
        // compositor 为该次 press 提供的原始 serial。
        serial: u32,
        // 只有完整匹配当前注册与 pointer 代次时才签发身份。
    ) -> Option<PointerActivationId> {
        // 迟到的旧 pointer 回调不得进入当前代次。
        if pointer_generation != self.pointer_generation {
            // 拒绝旧代理事件且不改变当前授权。
            return None;
            // 结束 pointer 代次校验。
        }
        // 读取 press 对应的当前 surface 注册事实。
        let registration = self.surfaces.get(&surface_id).copied()?;
        // 焦点解析窗口必须与 surface 当前所有者一致。
        if registration.window_id != window_id {
            // 拒绝不一致路由且不签发身份。
            return None;
            // 结束窗口归属校验。
        }
        // 签发新的不透明激活身份。
        let activation_id = PointerActivationId::new(Self::take_counter(
            // 使用注册表独占的单调计数器。
            &mut self.next_activation_id,
            // 结束计数器借用。
        ));
        // 新的主键 press 保守撤销任何未消费旧 press。
        self.pending_primary = Some(PendingPrimaryActivation {
            // 绑定本次 native 事件身份。
            activation_id,
            // 绑定创建回调时的 pointer 代次。
            pointer_generation,
            // 绑定 press surface 编号。
            surface_id,
            // 绑定当前 surface 注册代次。
            surface_generation: registration.generation,
            // 绑定当前窗口身份。
            window_id,
            // 仅在私有授权内保存真实 serial。
            serial,
            // 结束待消费授权字面量。
        });
        // 将不透明身份附着到对应 UiEvent。
        Some(activation_id)
        // 结束主键授权签发。
    }

    // 在对应 BTN_LEFT release 到达时撤销尚未消费的授权。
    pub(crate) fn revoke_primary_press(&mut self, pointer_generation: u64) {
        // 旧 pointer 回调不得撤销新代理签发的授权。
        if pointer_generation != self.pointer_generation {
            // 忽略过时代次的 release。
            return;
            // 结束旧代次保护。
        }
        // 只撤销同一 pointer 代次的待消费授权。
        if self
            // 读取当前待消费授权。
            .pending_primary
            // 精确匹配创建它的 pointer 代次。
            .is_some_and(|pending| pending.pointer_generation == pointer_generation)
        // 命中当前主键授权时进入撤销分支。
        {
            // release 之后协议授权不再可用。
            self.pending_primary = None;
            // 结束 release 撤销。
        }
        // 结束主键 release 处理。
    }

    // pointer 离开原 surface 时撤销该 surface 上尚未消费的授权。
    pub(crate) fn revoke_pointer_focus(
        // 注册表独占撤销状态变更。
        &mut self,
        // 捕获代次阻止旧代理离开事件影响新状态。
        pointer_generation: u64,
        // 离开的 surface 协议身份。
        surface_id: u32,
        // 离开前焦点解析出的窗口身份。
        window_id: WindowId,
        // 该操作无结果且保持幂等。
    ) {
        // 只允许当前 pointer 代理撤销授权。
        if pointer_generation != self.pointer_generation {
            // 忽略旧代理的迟到 leave。
            return;
            // 结束 pointer 代次校验。
        }
        // 检查待消费授权是否属于离开的精确窗口 surface。
        if self.pending_primary.is_some_and(|pending| {
            // 匹配 pointer 代次。
            pending.pointer_generation == pointer_generation
                // 匹配 surface 编号。
                && pending.surface_id == surface_id
                // 匹配稳定窗口身份。
                && pending.window_id == window_id
            // 结束待消费授权匹配闭包。
        }) {
            // 离开后不允许重放该 press 授权。
            self.pending_primary = None;
            // 结束 leave 撤销。
        }
        // 结束 pointer focus 撤销。
    }

    // pointer capability 丢失时撤销全部授权并推进代理代次。
    pub(crate) fn invalidate_pointer(&mut self) {
        // 所有未消费授权随 pointer 代理失效。
        self.pending_primary = None;
        // 推进代次并允许 u64 环绕时跳过零。
        self.pointer_generation = self.pointer_generation.wrapping_add(1);
        // 零不代表任何有效 pointer 代次。
        if self.pointer_generation == 0 {
            // 环绕后从一重新开始代次空间。
            self.pointer_generation = 1;
            // 结束零代次修正。
        }
        // 结束 pointer capability 失效处理。
    }

    // 检查共享 owner 后原子消费当前动作对应的 raw serial。
    pub(crate) fn consume_checked(
        // 共享注册表是 raw serial 的唯一 owner。
        registry: &Arc<Mutex<Self>>,
        // 当前原生事件转交的不可解释身份。
        activation_id: PointerActivationId,
        // 执行动作的稳定窗口身份。
        window_id: WindowId,
        // 执行动作窗口当前持有的 surface 编号。
        surface_id: u32,
        // 返回授权结果或同步 typed failure。
    ) -> Result<PointerActivationOutcome> {
        // mutex poison 时不得继续读取或修改任何授权状态。
        let mut registry = registry.lock().map_err(|_| {
            // 构造稳定的交互窗口授权错误。
            Error::new(
                // 授权 owner 已无法安全访问。
                Errc::InvalidState,
                // 保留 Wayland 移动或缩放请求与注册表阶段。
                "Wayland pointer activation registry mutex poisoned during interactive window request",
            )
        })?;
        // 健康 guard 内继续复用既有一次性校验与消费规则。
        Ok(registry.consume(activation_id, window_id, surface_id))
    }

    // 原子校验并取走当前动作对应的 raw serial。
    pub(crate) fn consume(
        // 注册表在校验期间持有唯一短时可变访问。
        &mut self,
        // 当前原生事件转交的不可解释身份。
        activation_id: PointerActivationId,
        // 执行动作的稳定窗口身份。
        window_id: WindowId,
        // 执行动作窗口当前持有的 surface 编号。
        surface_id: u32,
        // 返回一次性 serial 或确定性的安全忽略原因。
    ) -> PointerActivationOutcome {
        // 复制待消费授权以便在必要时保留给正确窗口。
        let Some(pending) = self.pending_primary else {
            // release、leave、关闭或已消费均属于正常缺失。
            return PointerActivationOutcome::Ignored(
                // 使用稳定原因支持定向诊断。
                PointerActivationRejection::MissingOrRevoked,
                // 结束安全忽略结果。
            );
            // 结束缺失授权分支。
        };
        // 不同 native 事件不得借用当前待消费授权。
        if pending.activation_id != activation_id {
            // 保留授权给真正携带该身份的事件。
            return PointerActivationOutcome::Ignored(
                // 报告激活身份不匹配。
                PointerActivationRejection::ActivationMismatch,
                // 结束身份不匹配结果。
            );
            // 结束激活身份校验。
        }
        // 其他窗口不能消费当前授权。
        if pending.window_id != window_id {
            // 保留授权给所属窗口。
            return PointerActivationOutcome::Ignored(
                // 报告窗口身份不匹配。
                PointerActivationRejection::WindowMismatch,
                // 结束窗口不匹配结果。
            );
            // 结束窗口身份校验。
        }
        // 同窗的其他 surface 不能消费当前授权。
        if pending.surface_id != surface_id {
            // 保留授权给产生它的 surface。
            return PointerActivationOutcome::Ignored(
                // 报告 surface 身份不匹配。
                PointerActivationRejection::SurfaceMismatch,
                // 结束 surface 不匹配结果。
            );
            // 结束 surface 身份校验。
        }
        // capability loss 后的旧授权必须永久失效。
        if pending.pointer_generation != self.pointer_generation {
            // 丢弃已经过时的授权。
            self.pending_primary = None;
            // 返回非致命旧 pointer 代次诊断。
            return PointerActivationOutcome::Ignored(
                // 指明 pointer 代理代次已经变化。
                PointerActivationRejection::StalePointerGeneration,
                // 结束旧 pointer 代次结果。
            );
            // 结束 pointer 代次校验。
        }
        // 读取当前 surface 注册以防关闭后编号复用。
        let registration = self.surfaces.get(&surface_id).copied();
        // 当前注册必须完整匹配授权签发时的窗口与代次。
        if registration.is_none_or(|registration| {
            // 窗口身份必须保持不变。
            registration.window_id != pending.window_id
                // surface 注册代次也必须保持不变。
                || registration.generation != pending.surface_generation
            // 结束 surface 注册匹配闭包。
        }) {
            // 丢弃已经失效的 surface 授权。
            self.pending_primary = None;
            // 返回非致命旧 surface 注册诊断。
            return PointerActivationOutcome::Ignored(
                // 指明 surface 生命周期已经变化。
                PointerActivationRejection::StaleSurfaceRegistration,
                // 结束旧 surface 注册结果。
            );
            // 结束 surface 注册代次校验。
        }
        // 先从注册表取走授权，锁外才能提交 Wayland 请求。
        self.pending_primary = None;
        // 返回与当前 native 事件精确匹配的 raw serial。
        PointerActivationOutcome::Authorized {
            // serial 只能被调用方立即提交一次。
            serial: pending.serial,
            // 结束授权成功结果。
        }
        // 结束授权消费方法。
    }
    // 结束注册表实现。
}
