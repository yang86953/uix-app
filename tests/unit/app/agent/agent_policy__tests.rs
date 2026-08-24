use super::*;

#[test]
fn default_policy_allows_everything() {
    let policy = AgentPolicy::default();
    assert_eq!(
        policy.check_semantic(None, SemanticActionKind::Invoke),
        PolicyDecision::Allow
    );
    assert_eq!(
        policy.check_semantic(Some("save-button"), SemanticActionKind::SetValue),
        PolicyDecision::Allow
    );
    assert_eq!(policy.check_window(), PolicyDecision::Allow);
}

#[test]
fn read_only_forbids_semantic_and_window_actions() {
    let policy = AgentPolicy::default().read_only();
    assert_eq!(
        policy.check_semantic(Some("save-button"), SemanticActionKind::Invoke),
        PolicyDecision::Forbidden
    );
    assert_eq!(
        policy.check_semantic(None, SemanticActionKind::Scroll),
        PolicyDecision::Forbidden
    );
    assert_eq!(policy.check_window(), PolicyDecision::Forbidden);
}

#[test]
fn protected_automation_id_is_forbidden() {
    let policy = AgentPolicy::default().protect("delete-button");
    assert_eq!(
        policy.check_semantic(Some("delete-button"), SemanticActionKind::Invoke),
        PolicyDecision::Forbidden
    );
    // 其他目标不受影响。
    assert_eq!(
        policy.check_semantic(Some("save-button"), SemanticActionKind::Invoke),
        PolicyDecision::Allow
    );
    // 无 automation_id 的目标不受目标保护影响。
    assert_eq!(
        policy.check_semantic(None, SemanticActionKind::Invoke),
        PolicyDecision::Allow
    );
}

#[test]
fn denied_action_kind_is_forbidden() {
    let policy = AgentPolicy::default().deny_action(SemanticActionKind::Toggle);
    assert_eq!(
        policy.check_semantic(Some("switch"), SemanticActionKind::Toggle),
        PolicyDecision::Forbidden
    );
    assert_eq!(
        policy.check_semantic(Some("switch"), SemanticActionKind::Invoke),
        PolicyDecision::Allow
    );
}

#[test]
fn deny_action_is_idempotent() {
    let policy = AgentPolicy::default()
        .deny_action(SemanticActionKind::SetValue)
        .deny_action(SemanticActionKind::SetValue);
    assert_eq!(
        policy.check_semantic(None, SemanticActionKind::SetValue),
        PolicyDecision::Forbidden
    );
}

#[test]
fn require_confirm_requests_confirmation() {
    let policy = AgentPolicy::default().require_confirm("danger-button");
    assert_eq!(
        policy.check_semantic(Some("danger-button"), SemanticActionKind::Invoke),
        PolicyDecision::RequiresConfirmation
    );
    // 其他目标不受影响。
    assert_eq!(
        policy.check_semantic(Some("save-button"), SemanticActionKind::Invoke),
        PolicyDecision::Allow
    );
}

#[test]
fn protected_target_wins_over_confirm() {
    // 受保护目标优先于确认目标：保护 = 直接拒绝，不进入确认流程。
    let policy = AgentPolicy::default()
        .protect("danger-button")
        .require_confirm("danger-button");
    assert_eq!(
        policy.check_semantic(Some("danger-button"), SemanticActionKind::Invoke),
        PolicyDecision::Forbidden
    );
}

#[test]
fn denied_action_wins_over_confirm() {
    // 全局禁止动作优先于目标确认：确认不能把拒绝规则降级成可执行动作。
    let policy = AgentPolicy::default()
        .deny_action(SemanticActionKind::Invoke)
        .require_confirm("danger-button");
    assert_eq!(
        policy.check_semantic(Some("danger-button"), SemanticActionKind::Invoke),
        PolicyDecision::Forbidden
    );
}
