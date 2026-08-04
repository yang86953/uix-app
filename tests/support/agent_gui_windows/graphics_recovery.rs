use super::*;

const RECOVERY_STATUS_ID: &str = "runtime-graphics-recovery-status";
const INJECT_DEVICE_LOST_ID: &str = "runtime-inject-device-lost";
// SurfaceLost 使用独立语义入口，验证 RHI acquire 的 surface 边界。
const INJECT_SURFACE_LOST_ID: &str = "runtime-inject-surface-lost";
const VERIFY_RECOVERED_ID: &str = "runtime-verify-recovered-interaction";

#[test]
#[ignore = "requires an interactive Windows desktop and the test-harness feature"]
fn real_demo_recovers_from_injected_device_loss_and_accepts_followup_interaction() {
    // 设备故障必须在 RHI preflight 处被分类。
    run_graphics_recovery_case(
        DEFAULT_D3D11_GRAPHICS,
        INJECT_DEVICE_LOST_ID,
        "ID3D11Device::GetDeviceRemovedReason",
        "device",
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and the test-harness feature"]
fn real_demo_recovers_from_injected_surface_loss_and_accepts_followup_interaction() {
    // surface 故障必须在 RHI acquire 处被分类。
    run_graphics_recovery_case(
        DEFAULT_D3D11_GRAPHICS,
        INJECT_SURFACE_LOST_ID,
        "D3d11 RHI test surface lost",
        "surface",
    );
}

// OpenGL ES 设备故障必须经过共享 RHI host 的 typed recovery boundary。
#[cfg(feature = "opengles")]
#[test]
#[ignore = "requires an interactive Windows desktop and the test-harness feature"]
fn real_opengles_demo_recovers_from_injected_device_loss_and_accepts_followup_interaction() {
    // 设备故障必须在 OpenGL RHI preflight 处被分类。
    run_graphics_recovery_case(
        FORCED_OPENGLES_GRAPHICS,
        INJECT_DEVICE_LOST_ID,
        "OpenGL RHI test device lost before present",
        "opengles-device",
    );
}

// OpenGL ES 表面故障必须经过共享 RHI host 的 acquire/present boundary。
#[cfg(feature = "opengles")]
#[test]
#[ignore = "requires an interactive Windows desktop and the test-harness feature"]
fn real_opengles_demo_recovers_from_injected_surface_loss_and_accepts_followup_interaction() {
    // 表面故障必须在 OpenGL RHI surface boundary 处被分类。
    run_graphics_recovery_case(
        FORCED_OPENGLES_GRAPHICS,
        INJECT_SURFACE_LOST_ID,
        "OpenGL RHI test surface lost",
        "opengles-surface",
    );
}

// 运行一条指定 lower fault boundary 的真实窗口恢复验收。
fn run_graphics_recovery_case(
    graphics: GraphicsExpectation,
    inject_id: &str,
    fault_marker: &str,
    fault_label: &str,
) {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo = DemoProcess::spawn_with_args(graphics, &["--graphics-recovery-acceptance"]);
    let descriptor = demo.wait_for_descriptor();
    let endpoint = descriptor["endpoint"]
        .as_str()
        .expect("descriptor endpoint");
    let token = descriptor["token"].as_str().expect("descriptor token");
    let stream = connect(endpoint, &mut demo.child);
    let mut connection = BufReader::new(stream);

    let hello = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "graphics-recovery-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-graphics-recovery-acceptance" },
        }),
    );
    assert_success(&hello, "graphics-recovery-hello");

    let listed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "graphics-recovery-list",
            "type": "list_windows",
        }),
    );
    assert_success(&listed, "graphics-recovery-list");
    let windows = listed["windows"].as_array().expect("listed windows");
    assert_eq!(windows.len(), 1);
    let window_id = windows[0]["window_id"].as_u64().expect("window id");
    let generation = windows[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "graphics-recovery-first-present",
        window_id,
        generation,
        1,
    );

    let home = snapshot(&mut connection, "graphics-recovery-home", window_id);
    demo.assert_expected_dpi(&home, "graphics-recovery");
    let (runtime_x, runtime_y) = visible_center(node_by_automation_id(&home, "sidebar-page-1"));
    let navigated = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "graphics-recovery-open-runtime",
        None,
        json!({ "kind": "click_at", "x": runtime_x, "y": runtime_y }),
    );
    wait_for_presented(
        &mut connection,
        "graphics-recovery-runtime-presented",
        window_id,
        generation,
        navigated["revision"].as_u64().expect("runtime revision"),
    );

    let before = snapshot(&mut connection, "graphics-recovery-before", window_id);
    assert_eq!(
        node_by_automation_id(&before, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：等待注入"
    );
    assert_eq!(
        node_by_automation_id(&before, VERIFY_RECOVERED_ID)["state"]["disabled"],
        true
    );

    let injected =
        invoke_until_presentable(&demo, &mut connection, window_id, generation, inject_id);
    let injected_revision = injected["revision"]
        .as_u64()
        .expect("injected status revision");
    wait_for_presented(
        &mut connection,
        "graphics-recovery-injected-presented",
        window_id,
        generation,
        injected_revision,
    );

    let recovered = snapshot(&mut connection, "graphics-recovery-recovered", window_id);
    assert_eq!(
        node_by_automation_id(&recovered, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：已注入，等待恢复后交互"
    );
    assert_eq!(
        node_by_automation_id(&recovered, VERIFY_RECOVERED_ID)["state"]["disabled"],
        false
    );

    let verified = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        VERIFY_RECOVERED_ID,
    );
    wait_for_presented(
        &mut connection,
        "graphics-recovery-followup-presented",
        window_id,
        generation,
        verified["revision"]
            .as_u64()
            .expect("verified status revision"),
    );
    let after = snapshot(&mut connection, "graphics-recovery-after", window_id);
    assert_eq!(
        node_by_automation_id(&after, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：恢复后交互成功"
    );

    // 第一次恢复成功后继续注入同一类故障，验证重建后的 RHI owner 仍可再次进入 lower boundary。
    let reinjected =
        invoke_until_presentable(&demo, &mut connection, window_id, generation, inject_id);
    // 记录第二次注入产生的新 revision，避免把上一轮 present 当成恢复证据。
    let reinjected_revision = reinjected["revision"]
        .as_u64()
        .expect("second injected status revision");
    // 等待第二次 typed fault 经过 teardown/rebuild 后重新完成 present。
    wait_for_presented(
        &mut connection,
        "graphics-recovery-second-injected-presented",
        window_id,
        generation,
        reinjected_revision,
    );
    // 第二次注入后页面必须重新开放恢复后交互动作。
    let second_recovered = snapshot(
        &mut connection,
        "graphics-recovery-second-recovered",
        window_id,
    );
    // 第二轮仍应处于待验证状态，而不是沿用上一轮的成功状态。
    assert_eq!(
        node_by_automation_id(&second_recovered, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：已注入，等待恢复后交互"
    );
    // 执行第二轮恢复后的用户动作，并等待它实际提交。
    let second_verified = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        VERIFY_RECOVERED_ID,
    );
    // 第二轮 present revision 必须继续前进。
    wait_for_presented(
        &mut connection,
        "graphics-recovery-second-followup-presented",
        window_id,
        generation,
        second_verified["revision"]
            .as_u64()
            .expect("second verified status revision"),
    );
    // 第二轮结束后仍保持可观察的成功状态。
    let second_after = snapshot(&mut connection, "graphics-recovery-second-after", window_id);
    // 第二次 teardown/rebuild 后的交互不能退回 disabled 或错误状态。
    assert_eq!(
        node_by_automation_id(&second_after, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：恢复后交互成功"
    );

    // 两轮同类故障后切换另一类 lower fault，验证重建状态不会绑定单一故障类型。
    let (alternating_inject_id, alternating_fault_marker, alternating_fault_label) =
        if inject_id == INJECT_DEVICE_LOST_ID {
            (
                INJECT_SURFACE_LOST_ID,
                if graphics.evidence_label == "forced-opengles" {
                    "OpenGL RHI test surface lost"
                } else {
                    "D3d11 RHI test surface lost"
                },
                "interleaved-surface",
            )
        } else {
            (
                INJECT_DEVICE_LOST_ID,
                if graphics.evidence_label == "forced-opengles" {
                    "OpenGL RHI test device lost before present"
                } else {
                    "ID3D11Device::GetDeviceRemovedReason"
                },
                "interleaved-device",
            )
        };
    // 注入交错故障并等待它经过新一轮 teardown/rebuild 与 present。
    let alternating = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        alternating_inject_id,
    );
    // 记录交错故障的状态 revision，避免复用第二轮 revision。
    let alternating_revision = alternating["revision"]
        .as_u64()
        .expect("interleaved injected status revision");
    // 交错故障必须到达新的 presented revision。
    wait_for_presented(
        &mut connection,
        "graphics-recovery-interleaved-injected-presented",
        window_id,
        generation,
        alternating_revision,
    );
    // 交错故障恢复后仍应开放用户验证动作。
    let interleaved_recovered = snapshot(
        &mut connection,
        "graphics-recovery-interleaved-recovered",
        window_id,
    );
    // 交错故障不得沿用上一轮的成功状态。
    assert_eq!(
        node_by_automation_id(&interleaved_recovered, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：已注入，等待恢复后交互"
    );
    // 执行交错故障后的 follow-up interaction。
    let interleaved_verified = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        VERIFY_RECOVERED_ID,
    );
    // 交错故障后的交互必须真实完成 present。
    wait_for_presented(
        &mut connection,
        "graphics-recovery-interleaved-followup-presented",
        window_id,
        generation,
        interleaved_verified["revision"]
            .as_u64()
            .expect("interleaved verified status revision"),
    );
    // 交错故障结束后保持可观察的成功状态。
    let interleaved_after = snapshot(
        &mut connection,
        "graphics-recovery-interleaved-after",
        window_id,
    );
    // 交错 teardown/rebuild 后的 owner 仍必须接受后续交互。
    assert_eq!(
        node_by_automation_id(&interleaved_after, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：恢复后交互成功"
    );

    drop(connection);
    let output = demo.close_and_wait();
    assert!(
        output.contains(fault_marker),
        "real window run must reach the native RHI {fault_label} boundary; output={output}"
    );
    // 同一进程还必须实际经过另一类交错 lower fault。
    assert!(
        output.contains(alternating_fault_marker),
        "real window run must reach the interleaved native RHI {alternating_fault_label} boundary; output={output}"
    );
    println!(
        "graphics recovery capture: path={}; typed_fault={fault_label}; lower_boundary={fault_marker}; rounds=3; interleaved={alternating_fault_label}; recovered_revision={injected_revision}; followup_revision={}",
        graphics.evidence_label,
        verified["revision"]
            .as_u64()
            .expect("verified status revision")
    );
}

fn snapshot(connection: &mut BufReader<File>, request_id: &str, window_id: u64) -> Value {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&response, request_id);
    response["snapshot"].clone()
}

fn wait_for_presented(
    connection: &mut BufReader<File>,
    request_id: &str,
    window_id: u64,
    generation: u64,
    revision: u64,
) {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&response, request_id);
    assert_eq!(response["outcome"], "presented");
}
