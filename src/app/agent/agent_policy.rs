//! Agent 动作策略 — 授权模型的第二层（动作级授权）。
//!
//! 默认策略是「全放行」：通过连接鉴权（同用户 + 会话 token）的 Agent
//! 可以自由控制应用的全部能力。应用按需收紧：全局只读、受保护目标、
//! 禁止的动作类别。策略检查发生在命令执行器内（UI turn 入口），命中
//! 策略拒绝时命令以 `forbidden` 错误失败，不进入 UI 语义路径。

use crate::ui::semantic_action::SemanticActionKind;

/// 策略检查结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyDecision {
    /// 放行：动作进入正常 UI 语义路径。
    Allow,
    /// 策略拒绝：只读模式、受保护目标或禁止的动作类别。
    Forbidden,
    /// 需要用户确认：目标命中 `require_confirm`，动作暂不执行，
    /// 由 AI 发起确认流程。
    RequiresConfirmation,
}

/// Agent 动作策略：应用声明的 AI 控制授权边界。
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentPolicy {
    /// 全局只读：拒绝所有写动作（语义动作与窗口动作）。
    read_only: bool,
    /// 受保护的 automation_id：命中后拒绝所有语义写动作。
    protected_automation_ids: Vec<String>,
    /// 全局禁止的语义动作类别。
    denied_actions: Vec<SemanticActionKind>,
    /// 需要用户确认的 automation_id：命中后动作须经确认流程。
    require_confirm_ids: Vec<String>,
}

impl AgentPolicy {
    /// 全局只读：Agent 只能读语义快照，不能执行任何动作。
    pub(crate) fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// 保护指定 automation_id 的组件：Agent 不能对其执行语义写动作。
    pub(crate) fn protect(mut self, automation_id: impl Into<String>) -> Self {
        self.protected_automation_ids.push(automation_id.into());
        self
    }

    /// 全局禁止指定语义动作类别（如 `set_value`、`toggle`）。
    pub(crate) fn deny_action(mut self, kind: SemanticActionKind) -> Self {
        if !self.denied_actions.contains(&kind) {
            self.denied_actions.push(kind);
        }
        self
    }

    /// 指定 automation_id 的组件需要用户确认：AI 执行写动作前进入确认流程。
    pub(crate) fn require_confirm(mut self, automation_id: impl Into<String>) -> Self {
        let automation_id = automation_id.into();
        if !self
            .require_confirm_ids
            .iter()
            .any(|id| *id == automation_id)
        {
            self.require_confirm_ids.push(automation_id);
        }
        self
    }

    /// 语义动作检查：目标 automation_id（无自动化标识时为 None）+ 动作类别。
    ///
    /// 优先级：只读 > 禁止动作类别 > 受保护目标 > 需要确认。
    pub(crate) fn check_semantic(
        &self,
        automation_id: Option<&str>,
        kind: SemanticActionKind,
    ) -> PolicyDecision {
        if self.read_only {
            return PolicyDecision::Forbidden;
        }
        // 全局拒绝规则必须先于目标确认生效；确认只能收紧允许动作，不能覆盖拒绝。
        if self.denied_actions.contains(&kind) {
            return PolicyDecision::Forbidden;
        }
        if let Some(id) = automation_id {
            if self
                .protected_automation_ids
                .iter()
                .any(|protected| protected == id)
            {
                return PolicyDecision::Forbidden;
            }
            if self
                .require_confirm_ids
                .iter()
                .any(|confirmed| confirmed == id)
            {
                return PolicyDecision::RequiresConfirmation;
            }
        }
        PolicyDecision::Allow
    }

    /// 窗口动作检查：窗口动作都是写操作，只读模式一律拒绝。
    pub(crate) fn check_window(&self) -> PolicyDecision {
        if self.read_only {
            PolicyDecision::Forbidden
        } else {
            PolicyDecision::Allow
        }
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/app/agent/agent_policy__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
