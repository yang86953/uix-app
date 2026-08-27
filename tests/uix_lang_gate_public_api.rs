// 消费 Compiler System 公开 check 入口，为门禁与诊断形态建立使用方视角的稳定快照。
use uix_lang_compiler::{CompileTarget, DiagnosticPhase, check_inline};

// 提交内嵌输入并断言其在期望的阶段、稳定诊断码、一基行列与关键文案上被定向拒绝。
fn expect_gate_diagnostic(
    source: &str,
    code: &str,
    phase: DiagnosticPhase,
    message_parts: &[&str],
    line: usize,
    column: usize,
) {
    let error = check_inline(source, "gate_inline.uix", CompileTarget::View)
        .expect_err("公开 check 入口必须拒绝无效输入");
    assert_eq!(error.code, code, "诊断码漂移：{source}\n实际：{error:?}");
    assert_eq!(error.phase, phase, "阶段漂移：{source}\n实际：{error:?}");
    for part in message_parts {
        assert!(
            error.message.contains(part),
            "诊断文案缺少「{part}」：{source}\n实际：{error:?}"
        );
    }
    assert!(error.suggestion.len() > 0, "定向诊断必须携带修复建议");
    assert_eq!(
        error.line, line,
        "主 SourceSpan 行漂移：{source}\n实际：{error:?}"
    );
    assert_eq!(
        error.column, column,
        "主 SourceSpan 列漂移：{source}\n实际：{error:?}"
    );
}

#[test]
fn unknown_label_is_rejected_in_semantic_phase() {
    expect_gate_diagnostic(
        "<UnknownLabel />",
        "UIX2000",
        DiagnosticPhase::Semantic,
        &["UnknownLabel"],
        1,
        1,
    );
}

#[test]
fn match_expression_is_rejected_in_syntax_phase() {
    expect_gate_diagnostic(
        "<Text>{match}</Text>",
        "UIX1000",
        DiagnosticPhase::Syntax,
        &["不支持 match"],
        1,
        8,
    );
    // 复杂分支的重定向建议同样锁进快照。
    let error = check_inline(
        "<Text>{match}</Text>",
        "gate_inline.uix",
        CompileTarget::View,
    )
    .expect_err("match 必须被拒绝");
    assert!(error.suggestion.contains("三元表达式"), "{error:?}");
}

#[test]
fn dangling_ternary_is_rejected_with_fix_hint() {
    expect_gate_diagnostic(
        "<Text>{flag ? 1}</Text>",
        "UIX1000",
        DiagnosticPhase::Syntax,
        &["三元表达式缺少 :"],
        1,
        16,
    );
}

#[test]
fn reserved_event_prefix_outside_handler_is_rejected() {
    expect_gate_diagnostic(
        "<Text>{$other}</Text>",
        "UIX1000",
        DiagnosticPhase::Syntax,
        &["$event", "$other"],
        1,
        8,
    );
}

#[test]
fn inline_import_is_rejected_in_import_phase() {
    expect_gate_diagnostic(
        "@import('./x.uix', 'X')\n<Text>t</Text>",
        "UIX1100",
        DiagnosticPhase::Import,
        &["@import"],
        1,
        1,
    );
}

#[test]
fn orphan_elseif_chain_is_rejected_in_semantic_phase() {
    expect_gate_diagnostic(
        "<Column><If {a}><Text /></If><Text>b</Text><ElseIf {c}><Text /></ElseIf></Column>",
        "UIX2000",
        DiagnosticPhase::Semantic,
        &["ElseIf"],
        1,
        44,
    );
}

#[test]
fn self_recursive_action_is_rejected_at_parse_time() {
    expect_gate_diagnostic(
        "<Widget name=\"Looper\" actions=\"loopSelf: loopSelf()\"><Button @click=\"loopSelf()\">x</Button></Widget><Looper />",
        "UIX1000",
        DiagnosticPhase::Syntax,
        &["action 递归调用不受支持", "loopSelf -> loopSelf"],
        1,
        23,
    );
}

#[test]
fn mixed_member_forms_are_rejected_at_parse_time() {
    expect_gate_diagnostic(
        "<Widget name=\"Counter\" state=\"count: 0\">\n  @state {\n    draft: '',\n  }\n  <Text>t</Text>\n</Widget><Counter />",
        "UIX1000",
        DiagnosticPhase::Syntax,
        &["state", "属性形式", "块级形式"],
        1,
        24,
    );
}

#[test]
fn valid_widget_document_passes_the_shared_gate() {
    let source = "<Widget name=\"Pager\">\n  @props {\n    total: number,\n  }\n  @state {\n    page: 0,\n  }\n  @actions {\n    next: do { setState(page: page + total); },\n  }\n  <Button @click=\"next()\" /><Slot />\n</Widget>\n<Pager total={1} />";
    let output =
        check_inline(source, "gate_valid.uix", CompileTarget::View).expect("合法文档必须通过");
    // 检查结果携带完整增量身份；内嵌来源没有文件追踪项。
    assert!(output.tracked_files.is_empty());
    assert!(!format!("{:?}", output.compilation_key).is_empty());
}

#[test]
fn valid_app_document_passes_the_shared_gate() {
    let source = "<App title=\"gate\"><Column><Text>hello</Text></Column></App>";
    check_inline(source, "gate_app.uix", CompileTarget::App)
        .expect("合法 App 文档必须通过共享门禁");
}
