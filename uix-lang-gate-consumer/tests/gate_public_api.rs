// 在 capability 全关环境中消费公开宏与 check 入口：
// 以真实子进程构建驱动 feature 门控的失败编译，并验证核心面不受能力裁剪影响。
use std::process::Command;
use uix_lang_compiler::{CompileTarget, DiagnosticPhase, check_inline};

// 打开指定场景 feature 构建本探针 crate，断言构建失败且输出携带全部关键诊断文本。
fn assert_broken_build_fails(feature: &str, expected_parts: &[&str]) {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "uix-lang-gate-consumer",
            "--features",
            feature,
        ])
        // 与工作区共享 target 缓存；按清单目录定位避免依赖外部调用环境。
        .env(
            "CARGO_TARGET_DIR",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../target"),
        )
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("必须能够启动 cargo 子构建");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "打开 {feature} 后构建必须失败\n{combined}"
    );
    for part in expected_parts {
        assert!(
            combined.contains(part),
            "失败输出缺少「{part}」\n{combined}"
        );
    }
}

#[test]
fn mixed_member_forms_fail_the_real_consumer_build() {
    assert_broken_build_fails(
        "gate-broken-mixed-member-forms",
        &[
            "UIX1000",
            "组件成员 state 同时使用了属性形式与块级形式",
            "建议：删除",
        ],
    );
}

#[test]
fn cascader_value_fails_the_real_consumer_build() {
    assert_broken_build_fails(
        "gate-broken-cascader-value",
        &[
            "UIX2000",
            "state region 的类型 CascaderValue 需要 capability tree-widgets",
        ],
    );
}

// 引用 CascaderValue 的最小 Widget 文档；未启用 tree-widgets 时必须在生成前拒绝。
const CASCADER_SOURCE: &str = r#"<Widget name="GateValueProbe" state="region: CascaderValue = { labels: [], values: [] }">
  <Text>gate</Text>
</Widget><GateValueProbe />"#;

// cargo 会跨目标统一 uix-lang-compiler 的 capability feature；本用例只在链接了
// 未启用能力版本的构建（如单独 `cargo test -p uix-lang-gate-consumer`）中生效，
// 其余环境自动让位给上方的真实失败编译证据。
#[test]
fn check_entry_reports_disabled_capability_in_a_closed_feature_graph() {
    let Err(error) = check_inline(CASCADER_SOURCE, "gate_closed.uix", CompileTarget::View) else {
        // 当前解析图把 capability 抬回开启：定向诊断由失败编译案例持有。
        return;
    };
    assert_eq!(error.code, "UIX2000", "实际：{error:?}");
    assert_eq!(error.phase, DiagnosticPhase::Semantic);
    assert!(
        error.message.contains("CascaderValue") && error.message.contains("tree-widgets"),
        "{error:?}"
    );
    assert_eq!(error.line, 1);
    assert_eq!(error.column, 31);
}

#[test]
fn core_components_pass_without_any_optional_capability() {
    let source = "<Widget name=\"Minimal\"><Text>core</Text></Widget><Minimal />";
    check_inline(source, "gate_minimal.uix", CompileTarget::View)
        .expect("只用核心组件的文档在不启用任何能力的消费者环境必须通过共享门禁");
}
