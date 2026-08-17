// 保存树运行时唯一所有者可见的协调执行状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WidgetTreeExecutionState {
    // 正常运行时允许外部事件、帧工作与协调事务进入树。
    Operational,
    // 协调事务期间记录是否已经越过不可回滚的发布点。
    Coordinating {
        // 结构或最终 journal 发布一旦开始就不能恢复旧树。
        publish_started: bool,
    },
    // 已发布后的 panic 使树停止接收任何后续外部工作。
    Poisoned,
    // 所有者关闭树后永久拒绝后续工作。
    Shutdown,
}

// 提供状态转换与工作准入的私有实现。
impl WidgetTreeExecutionState {
    // 创建尚未接收任何外部工作的新树状态。
    pub(super) const fn operational() -> Self {
        // 新树从可协调且可接收外部工作的状态开始。
        Self::Operational
    }

    // 判断外部调度是否仍可调用用户组件。
    pub(super) const fn accepts_external_work(self) -> bool {
        // 仅完整可运行树可以处理外部工作。
        matches!(self, Self::Operational)
    }

    // 判断新的协调事务是否仍可进入树。
    pub(super) const fn accepts_coordination_work(self) -> bool {
        // 嵌套协调共享外层事务，而已停止树不得再次协调。
        matches!(self, Self::Operational | Self::Coordinating { .. })
    }

    // 判断驱动和绘制是否必须停止访问半完成或已关闭树。
    pub(super) const fn is_fail_stopped(self) -> bool {
        // poison 与 shutdown 都不允许继续驱动已有节点。
        matches!(self, Self::Poisoned | Self::Shutdown)
    }

    // 开始最外层协调事务并保留嵌套事务的发布标记。
    pub(super) fn begin_coordination(&mut self) {
        // 仅运行中树可以建立新的协调边界。
        if matches!(self, Self::Operational) {
            // 新事务尚未改写任何不可回滚的运行时结构。
            *self = Self::Coordinating {
                // 发布标记由真实 mutator 单调置位。
                publish_started: false,
            };
        }
    }

    // 记录当前协调事务已越过不可回滚发布点。
    pub(super) fn mark_publish_started(&mut self) {
        // 仅协调中的树需要记录发布事实。
        if let Self::Coordinating { publish_started } = self {
            // 发布事实单调保持，嵌套事务不得把它复位。
            *publish_started = true;
        }
    }

    // 返回当前协调事务是否已经改写真实运行时结构。
    pub(super) const fn publish_started(self) -> bool {
        // 非协调状态没有可恢复事务发布事实。
        matches!(
            self,
            Self::Coordinating {
                publish_started: true
            }
        )
    }

    // 在最外层协调完整成功后重新开放树。
    pub(super) fn finish_coordination(&mut self) {
        // 仅正常完成的协调事务可以回到运行态。
        if matches!(self, Self::Coordinating { .. }) {
            // 成功提交后的树再次接受下一轮独立协调。
            *self = Self::Operational;
        }
    }

    // 把已发布且无法回滚的树切换到 fail-stop 状态。
    pub(super) fn poison(&mut self) {
        // 保留失败终态，避免后续嵌套入口意外重开树。
        if !matches!(self, Self::Shutdown) {
            // 已停止树只能由其所有者 teardown 后替换。
            *self = Self::Poisoned;
        }
    }

    // 进入关闭终态并返回是否仍可调用正常生命周期。
    pub(super) fn begin_shutdown(&mut self) -> bool {
        // 只有从完整运行态关闭时生命周期才拥有可靠结构前提。
        let run_lifecycle = matches!(self, Self::Operational);
        // 关闭先线性化，防止回调重入后继续外部工作。
        *self = Self::Shutdown;
        // 由树所有者决定是否执行受控生命周期回调。
        run_lifecycle
    }
}
